//! `host`: the native host of si-jam-sessions.
//!
//! ```text
//! cargo run -p host --release -- devices
//! cargo run -p host --release -- play [--output <index|name>] [--mute]
//! cargo run -p host --release -- render <out.wav>
//! cargo run -p host --release -- jam [--output <index|name>] [--midi <index|name> | --keyboard] [--mute]
//! cargo run -p host --release -- jitter [--output <index|name>]
//! cargo run -p host --release -- notices
//! ```
//!
//! Exit status: 0 when the command finished, 1 when it stopped on an error (a
//! refused law call, a device error, a bad argument), with the reason printed.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use cpal::traits::StreamTrait;
use golden::take::Perturbation;
use host::anchor::{Reading, residuals};
use host::bridge::{Law, Refused};
use host::callback::{Callback, Shared};
use host::device::{self, Output, Playing};
use host::event::{Event, Monitor};
use host::live::{Clocks, Held, Passed, Press, close_through, delivery, pass, release_all};
use host::schedule::{Scheduler, steps_for};
use host::score::{Piece, clock, root};
use host::synth::Synth;
use host::{RING_EVENTS, offline};
use rtrb::{Consumer, Producer, RingBuffer};

const USAGE: &str = "usage:
  host devices                  list the audio outputs and the MIDI inputs
  host play [--output X] [--mute]
                                play The Entertainer's constructed take against the score and the click;
                                --mute renders and counts everything and sends the device silence
  host render <out.wav>         render the same mix to a WAV file, with no device
  host jam [--output X] [--midi X | --keyboard] [--mute]
                                play the score and the click, take a live take, print its verdicts
  host jitter [--output X]      measure the audio clock's readings and the console's key path
  host notices                  print the licences of the crates the host is built from
X is an index from `host devices` or part of a name.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((command, rest)) => match command.as_str() {
            "devices" => no_options(rest).and_then(|()| devices()),
            "play" => options(rest, false, true).and_then(|o| play(&o)),
            "render" => match rest {
                [path] => render(path),
                _ => Err(String::from("render takes one argument, the WAV to write")),
            },
            "jam" => options(rest, true, true).and_then(|o| jam(&o)),
            "jitter" => options(rest, false, false).and_then(|o| jitter(&o)),
            "notices" => no_options(rest).map(|()| print!("{}", host::notices::TEXT)),
            "help" | "--help" | "-h" => {
                println!("{USAGE}");
                Ok(())
            }
            other => Err(format!("no command \"{other}\"\n{USAGE}")),
        },
        None => Err(String::from(USAGE)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("host: {e}");
            ExitCode::from(1)
        }
    }
}

#[derive(Default)]
struct Options {
    output: Option<String>,
    midi: Option<String>,
    keyboard: bool,
    mute: bool,
}

fn no_options(rest: &[String]) -> Result<(), String> {
    match rest {
        [] => Ok(()),
        _ => Err(format!("unexpected arguments: {}", rest.join(" "))),
    }
}

/// The options a command takes: `--output` always; `--midi` and `--keyboard`
/// for `jam` (`live`); `--mute` for `play` and `jam` (`mute`).
fn options(rest: &[String], live: bool, mute: bool) -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--output" if o.output.is_none() => o.output = Some(value()?),
            "--midi" if live && o.midi.is_none() => o.midi = Some(value()?),
            "--keyboard" if live && !o.keyboard => o.keyboard = true,
            "--mute" if mute && !o.mute => o.mute = true,
            other => return Err(format!("unexpected argument \"{other}\"\n{USAGE}")),
        }
    }
    if o.keyboard && o.midi.is_some() {
        return Err(String::from(
            "--midi and --keyboard name two inputs; pick one",
        ));
    }
    Ok(o)
}

fn refused(r: Refused) -> String {
    r.to_string()
}

/// A MIDI pitch's name: 60 is C4.
fn name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = i32::from(pitch) / 12 - 1;
    let letter = NAMES.get(usize::from(pitch % 12)).copied().unwrap_or("?");
    format!("{letter}{octave}")
}

fn devices() -> Result<(), String> {
    println!("Audio outputs (WASAPI shared mode on Windows):");
    for (i, o) in device::outputs()?.iter().enumerate() {
        let mut line = format!("  {i}: {}", o.name);
        if o.default {
            line.push_str("  [default]");
        }
        if o.bluetooth {
            line.push_str(
                "  [Bluetooth: fine for hearing late notes against the click, too slow to play \
                 along through]",
            );
        }
        println!("{line}");
    }
    println!();
    list_midi();
    Ok(())
}

#[cfg(windows)]
fn list_midi() {
    let ports = host::winmm::ports();
    println!("MIDI inputs (WinMM):");
    if ports.is_empty() {
        println!("  none: `jam` takes the computer keyboard");
    }
    for (i, name) in ports.iter().enumerate() {
        println!("  {i}: {name}");
    }
    if ports.len() > 1 {
        println!(
            "  `jam` needs --midi <index|name> to choose one; one keyboard can show as more \
             than one port"
        );
    }
}

#[cfg(not(windows))]
fn list_midi() {
    println!("MIDI inputs: live input is built for Windows (WinMM and the console) only");
}

/// Prints where to listen, for `play` and `render`, in time order.
fn guide(piece: &Piece) {
    let mut drawn = piece.drawn;
    drawn.sort_by_key(|d| piece.score.note(d.note).map_or(0, |n| n.onset_sample));
    println!("Where to listen (the take is the reedy voice, the score the pure one):");
    for d in drawn {
        let Some(note) = piece.score.note(d.note) else {
            continue;
        };
        let score = note.onset_sample;
        let take = d.perturbation.onset(score).unwrap_or(score);
        let what = match d.perturbation {
            Perturbation::WrongPitch => format!(
                "the take plays {} ({}) where the score plays {} ({})",
                name(d.perturbation.pitch(note.pitch)),
                d.perturbation.pitch(note.pitch),
                name(note.pitch),
                note.pitch
            ),
            p => {
                let ms = p.delta_samples() as f64 / 48.0;
                let side = if ms < 0.0 { "before" } else { "after" };
                format!(
                    "the take plays {} ({}) {:.1} ms {side} the score",
                    name(note.pitch),
                    note.pitch,
                    ms.abs()
                )
            }
        };
        println!(
            "  {:<11}  at {}  score note {} at sample {}, take at sample {}: {what}",
            d.perturbation.label(),
            clock(score),
            d.note.0,
            score,
            take
        );
    }
}

/// The law's rows for the five drawn notes.
fn drawn_rows(piece: &Piece, rows: &str) {
    println!("The law's verdicts for them:");
    for d in piece.drawn {
        let prefix = format!("note {}: ", d.note.0);
        if let Some(row) = rows.lines().find(|r| r.starts_with(&prefix)) {
            println!("  {row}");
        }
    }
}

fn render(path: &str) -> Result<(), String> {
    let piece = Piece::entertainer(&root())?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    law.admit_take(&piece.take).map_err(refused)?;
    let end = piece.end + 48_000;
    let (samples, counts) = offline::render(&mut law, end, 480).map_err(refused)?;
    let mut bytes = Vec::with_capacity(samples.len() * 4 + 58);
    offline::write_wav(&mut bytes, &samples).map_err(|e| e.to_string())?;
    std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
    let record = law.record().map_err(refused)?;
    println!(
        "Rendered {} frames ({}) of The Entertainer's constructed take, the score and the \
         click to {path}: mono 32-bit float at 48 kHz; sample n of the file is law sample n.",
        samples.len(),
        clock(samples.len() as u64)
    );
    println!(
        "  {} notes and {} beats started, {} late, {} dropped; file SHA-256 {}",
        counts.notes,
        counts.beats,
        counts.late,
        counts.dropped,
        golden::run::hex(&golden::run::sha256(&bytes))
    );
    guide(&piece);
    drawn_rows(&piece, &record.rows);
    Ok(())
}

/// Rings, the scheduler and a started stream, for `play` and `jam`.
struct Session {
    playing: Playing,
    scheduler: Scheduler,
    events: Producer<Event>,
    readings: Consumer<Reading>,
}

fn open(
    output: &Output,
    law: &mut Law,
    monitor: Option<Consumer<Monitor>>,
    mute: bool,
) -> Result<Session, String> {
    // Every ring is made before the stream: rtrb allocates only in
    // RingBuffer::new.
    let (mut events, events_out) = RingBuffer::new(RING_EVENTS);
    let (readings_in, readings) = RingBuffer::new(1_024);
    let shared = Arc::new(Shared::default());
    let mut scheduler = Scheduler::new(0);
    // The first H + 1 quanta are in the ring before the first callback, and
    // the callback knows how far: its pre-roll waits for a callback they
    // cover.
    let pumped = scheduler
        .pump(law, steps_for(0, 0, None), &mut events)
        .map_err(refused)?;
    shared.covered.store(pumped.covered, Ordering::Release);
    let callback = Callback::new(
        Synth::new(0),
        events_out,
        monitor,
        readings_in,
        Arc::clone(&shared),
        mute,
    );
    let playing = device::start(output, callback, shared)?;
    Ok(Session {
        playing,
        scheduler,
        events,
        readings,
    })
}

impl Session {
    /// Steps the law ([`steps_for`]): to the sample heard now when `heard` is
    /// given, to the callback's position otherwise, and always far enough that
    /// the horizon covers the next two callbacks of the largest size seen.
    /// Then fills the ring, and tells the callback how far it is complete.
    fn pump(&mut self, law: &mut Law, heard: Option<i64>) -> Result<(), String> {
        let shared = &self.playing.shared;
        let frame = shared.frames.load(Ordering::Acquire);
        let lookahead = shared
            .largest_buffer
            .load(Ordering::Relaxed)
            .saturating_mul(2);
        let target = steps_for(frame, lookahead, heard);
        let pumped = self
            .scheduler
            .pump(law, target, &mut self.events)
            .map_err(refused)?;
        shared.covered.store(pumped.covered, Ordering::Release);
        Ok(())
    }
}

/// Reports, once, when law time started: how long the silent pre-roll ran,
/// and what the device's first callback asked for.
#[derive(Default)]
struct Startup {
    told: bool,
}

impl Startup {
    fn watch(&mut self, session: &Session) {
        let shared = &session.playing.shared;
        if self.told || !shared.started.load(Ordering::Acquire) {
            return;
        }
        self.told = true;
        let silent = shared.preroll_frames.load(Ordering::Relaxed);
        let first = shared.first_buffer.load(Ordering::Relaxed);
        if silent == 0 {
            println!(
                "  Law time started with the first callback ({first} frames), which the ring \
                 covered."
            );
        } else {
            println!(
                "  The first callback asked for {first} frames, more than the ring covered; \
                 {silent} frames ({:.1} ms) of silence came before law sample 0.",
                silent as f64 / 48.0
            );
        }
    }
}

/// Says what the output is, now; its latency follows once it settles.
fn describe(output: &Output, session: &Session) {
    let buffer = session
        .playing
        .buffer
        .map_or_else(|| String::from("unknown"), |b| b.to_string());
    println!(
        "Output: {} ({} channels, 48 kHz, {buffer} frames a callback)",
        output.name, session.playing.channels
    );
}

/// Reports the output's latency once, when it has settled.
///
/// The latency cpal reports (`playback - callback`) grows while WASAPI fills
/// the device's buffer at the start: on this project's Bluetooth speaker the
/// first callback came after half a second, and the latency then read 43 ms
/// before it settled at 163 ms. So it is read from the law thread's loop,
/// which must not stop to wait for it, and printed when two readings 100 ms
/// apart agree within half a millisecond.
#[derive(Default)]
struct Latency {
    last: Option<(std::time::Instant, u64)>,
    told: bool,
}

impl Latency {
    fn watch(&mut self, output: &Output, session: &Session) {
        let shared = &session.playing.shared;
        if self.told || shared.callbacks.load(Ordering::Relaxed) == 0 {
            return;
        }
        let now = std::time::Instant::now();
        let nanos = shared.latency_nanos.load(Ordering::Relaxed);
        match self.last {
            Some((then, before)) if now.duration_since(then) >= Duration::from_millis(100) => {
                if nanos.abs_diff(before) < 500_000 {
                    self.told = true;
                    tell_latency(output, nanos);
                } else {
                    self.last = Some((now, nanos));
                }
            }
            Some(_) => {}
            None => self.last = Some((now, nanos)),
        }
    }
}

fn tell_latency(output: &Output, nanos: u64) {
    let ms = nanos as f64 / 1e6;
    println!("  The output reports {ms:.1} ms from a callback to the ear.");
    if output.bluetooth || ms > 40.0 {
        println!(
            "  That is fine for hearing a late note against the click, but too slow to play \
             along through: your own notes come back that late. Grading follows what you hear, \
             except any delay the device does not report, which grades your notes that much \
             late. A wired output is the one to play through."
        );
    }
}

/// Stops on Enter, from a thread of its own. An input that ends without a
/// line (none attached, or closed) stops nothing: the command runs to its end.
fn stop_on_enter(stop: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut line = String::new();
        if matches!(std::io::stdin().read_line(&mut line), Ok(n) if n > 0) {
            stop.store(true, Ordering::SeqCst);
        }
    });
}

/// A stop flag the device's error also raises.
fn stop_flag(session: &Session) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let shared = Arc::clone(&session.playing.shared);
    let watched = Arc::clone(&stop);
    thread::spawn(move || {
        while !watched.load(Ordering::Relaxed) {
            if shared.stop.load(Ordering::Relaxed) {
                watched.store(true, Ordering::SeqCst);
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
    stop
}

/// What the device said, if it stopped the stream.
fn device_error(session: &Session) -> Option<String> {
    match session.playing.shared.error.load(Ordering::SeqCst) {
        0 => None,
        code => Some(format!(
            "the audio device stopped the stream: {}",
            device::kind_text(code)
        )),
    }
}

fn counted(session: &Session) -> String {
    let s = &session.playing.shared;
    format!(
        "{} notes and {} beats started, {} live notes heard back, {} late, {} dropped; the \
         first callback asked for {} frames, the largest for {}, and {} frames of silence came \
         before law sample 0",
        s.notes.load(Ordering::Relaxed),
        s.beats.load(Ordering::Relaxed),
        s.monitored.load(Ordering::Relaxed),
        s.late.load(Ordering::Relaxed),
        s.dropped.load(Ordering::Relaxed),
        s.first_buffer.load(Ordering::Relaxed),
        s.largest_buffer.load(Ordering::Relaxed),
        s.preroll_frames.load(Ordering::Relaxed)
    )
}

fn play(o: &Options) -> Result<(), String> {
    let piece = Piece::entertainer(&root())?;
    let output = device::choose_output(o.output.as_deref())?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    law.admit_take(&piece.take).map_err(refused)?;
    let mut session = open(&output, &mut law, None, o.mute)?;
    describe(&output, &session);
    guide(&piece);
    let muted = if o.mute { ", muted" } else { "" };
    println!("Playing {}{muted}; press Enter to stop.", clock(piece.end));
    let stop = stop_flag(&session);
    stop_on_enter(Arc::clone(&stop));
    let end = piece.end + 48_000;
    let mut failed = None;
    let mut latency = Latency::default();
    let mut startup = Startup::default();
    while !stop.load(Ordering::Relaxed) {
        latency.watch(&output, &session);
        startup.watch(&session);
        let frame = session.playing.shared.frames.load(Ordering::Acquire);
        if frame >= end {
            break;
        }
        if let Err(e) = session.pump(&mut law, None) {
            failed = Some(e);
            break;
        }
        while session.readings.pop().is_ok() {}
        thread::sleep(Duration::from_millis(2));
    }
    stop.store(true, Ordering::SeqCst);
    let _ = session.playing.stream.pause();
    println!("{}", counted(&session));
    let report = law
        .record()
        .map_err(refused)
        .map(|record| drawn_rows(&piece, &record.rows));
    host::outcome(report, device_error(&session).or(failed))
}

/// Which input a jam takes.
enum Input {
    #[cfg(windows)]
    Midi(u32, String),
    #[cfg(windows)]
    Keyboard,
    #[cfg(not(windows))]
    None,
}

#[cfg(windows)]
fn choose_input(o: &Options) -> Result<Input, String> {
    if o.keyboard {
        return Ok(Input::Keyboard);
    }
    let ports = host::winmm::ports();
    let listed = || {
        ports
            .iter()
            .enumerate()
            .map(|(i, p)| format!("  {i}: {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let index = match &o.midi {
        Some(text) => match text.parse::<usize>() {
            Ok(i) if i < ports.len() => i,
            _ => {
                let wanted = text.to_lowercase();
                let hits: Vec<usize> = ports
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.to_lowercase().contains(&wanted))
                    .map(|(i, _)| i)
                    .collect();
                match hits.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(format!(
                            "no single MIDI input is \"{text}\"; the inputs are:\n{}",
                            if ports.is_empty() {
                                String::from("  none")
                            } else {
                                listed()
                            }
                        ));
                    }
                }
            }
        },
        None => match ports.len() {
            0 => return Ok(Input::Keyboard),
            1 => 0,
            n => {
                return Err(format!(
                    "there are {n} MIDI inputs; choose one with --midi <index|name>:\n{}",
                    listed()
                ));
            }
        },
    };
    let name = ports.get(index).cloned().unwrap_or_default();
    Ok(Input::Midi(u32::try_from(index).unwrap_or(0), name))
}

#[cfg(not(windows))]
fn choose_input(o: &Options) -> Result<Input, String> {
    if o.keyboard || o.midi.is_some() {
        return Err(String::from(
            "live input is built for Windows (WinMM and the console) only",
        ));
    }
    Ok(Input::None)
}

/// A MIDI input, started: the port, the count of invalid messages WinMM
/// reported, and the port count when it opened, to notice a keyboard
/// unplugged mid-jam (WinMM tells its callback nothing then).
#[cfg(windows)]
struct Midi {
    port: host::winmm::MidiIn,
    errors: Arc<std::sync::atomic::AtomicU64>,
    ports: u32,
    told: bool,
}

#[cfg(windows)]
impl Midi {
    /// Says once if WinMM lists fewer MIDI input ports than when the jam
    /// began.
    fn watch(&mut self) {
        let now = host::winmm::port_count();
        if !self.told && now < self.ports {
            self.told = true;
            println!(
                "  WinMM now lists {now} MIDI input ports, {} when the jam began. If the \
                 keyboard was unplugged, its notes stopped arriving; WinMM does not say so \
                 otherwise.",
                self.ports
            );
        }
    }
}

/// The input, started: what must be stopped when the jam ends.
enum Started {
    #[cfg(windows)]
    Midi(Midi),
    #[cfg(windows)]
    Keyboard(thread::JoinHandle<Result<(), String>>),
    #[cfg(not(windows))]
    None,
}

#[cfg(windows)]
fn start_input(
    input: Input,
    session: &Session,
    stop: &Arc<AtomicBool>,
    monitor: Producer<Monitor>,
    presses: Producer<Press>,
) -> Result<Started, String> {
    match input {
        Input::Midi(port, name) => {
            let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let sink = host::winmm::Sink {
                monitor,
                input: presses,
                stream: Arc::clone(&session.playing.stream),
                errors: Arc::clone(&errors),
            };
            let ports = host::winmm::port_count();
            let midi = host::winmm::MidiIn::open(port, sink)?;
            println!("Live input: MIDI port {port}, \"{name}\". Press Enter to stop.");
            stop_on_enter(Arc::clone(stop));
            Ok(Started::Midi(Midi {
                port: midi,
                errors,
                ports,
                told: false,
            }))
        }
        Input::Keyboard => {
            let stream = Arc::clone(&session.playing.stream);
            let flag = Arc::clone(stop);
            let keys = thread::spawn(move || host::console::read(&flag, &stream, monitor, presses));
            println!(
                "Live input: the computer keyboard (no MIDI input port was chosen or found). {}",
                host::console::LAYOUT
            );
            Ok(Started::Keyboard(keys))
        }
    }
}

#[cfg(not(windows))]
fn start_input(
    _input: Input,
    _session: &Session,
    stop: &Arc<AtomicBool>,
    _monitor: Producer<Monitor>,
    _presses: Producer<Press>,
) -> Result<Started, String> {
    println!(
        "Live input: none; live input is built for Windows. Playing the score and the click. \
         Press Enter to stop."
    );
    stop_on_enter(Arc::clone(stop));
    Ok(Started::None)
}

/// Watches the input while the jam runs.
fn watch_input(started: &mut Started) {
    match started {
        #[cfg(windows)]
        Started::Midi(midi) => midi.watch(),
        #[cfg(windows)]
        Started::Keyboard(_) => {}
        #[cfg(not(windows))]
        Started::None => {}
    }
}

/// Stops the input, and reports what WinMM said on the way.
fn finish_input(started: Started) {
    match started {
        #[cfg(windows)]
        Started::Midi(midi) => {
            let errors = midi.errors.load(Ordering::Relaxed);
            if errors > 0 {
                println!("WinMM reported {errors} invalid MIDI messages (MIM_ERROR).");
            }
            if let Err(e) = midi.port.close() {
                eprintln!("host: {e}");
            }
        }
        #[cfg(windows)]
        Started::Keyboard(keys) => match keys.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("host: the keyboard stopped: {e}"),
            Err(_) => eprintln!("host: the keyboard reader stopped"),
        },
        #[cfg(not(windows))]
        Started::None => {}
    }
}

/// Moves the readings into the audio clock and passes every press to the
/// law, in the order they were received.
fn take_presses(
    session: &mut Session,
    presses: &mut Consumer<Press>,
    clocks: &mut Clocks,
    held: &mut Held,
    law: &mut Law,
    passed: &mut Vec<Passed>,
) {
    while let Ok(reading) = session.readings.pop() {
        clocks.audio.push(reading);
    }
    while let Ok(press) = presses.pop() {
        let Some(instant) = clocks.instant(press.stamp) else {
            continue;
        };
        pass(law, clocks, held, press, instant, passed);
    }
}

fn jam(o: &Options) -> Result<(), String> {
    let piece = Piece::entertainer(&root())?;
    let input = choose_input(o)?;
    let output = device::choose_output(o.output.as_deref())?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    let (monitor_in, monitor_out) = RingBuffer::<Monitor>::new(256);
    let (presses_in, mut presses) = RingBuffer::<Press>::new(4_096);
    let mut session = open(&output, &mut law, Some(monitor_out), o.mute)?;
    describe(&output, &session);
    let stop = stop_flag(&session);
    let mut started = start_input(input, &session, &stop, monitor_in, presses_in)?;
    println!(
        "Play along with the score (the pure voice) and the click; your notes sound in the \
         reedy voice. The score ends at {}.",
        clock(piece.end)
    );

    let mut clocks = Clocks::default();
    let mut held = Held::default();
    let mut passed: Vec<Passed> = Vec::new();
    let end = piece.end + 48_000;
    let mut failed = None;
    let mut latency = Latency::default();
    let mut startup = Startup::default();
    let mut watched = std::time::Instant::now();
    while !stop.load(Ordering::Relaxed) {
        latency.watch(&output, &session);
        startup.watch(&session);
        if watched.elapsed() >= Duration::from_millis(500) {
            watched = std::time::Instant::now();
            watch_input(&mut started);
        }
        let frame = session.playing.shared.frames.load(Ordering::Acquire);
        if frame >= end {
            break;
        }
        // Presses first, at the playhead they were played under; then the
        // law steps to the sample heard now.
        take_presses(
            &mut session,
            &mut presses,
            &mut clocks,
            &mut held,
            &mut law,
            &mut passed,
        );
        let heard = clocks
            .audio
            .sample_at(device::nanos(session.playing.stream.now()));
        if let Err(e) = session.pump(&mut law, heard) {
            failed = Some(e);
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    stop.store(true, Ordering::SeqCst);
    finish_input(started);
    // Presses still queued pass now, and notes still held end now.
    take_presses(
        &mut session,
        &mut presses,
        &mut clocks,
        &mut held,
        &mut law,
        &mut passed,
    );
    let now = device::nanos(session.playing.stream.now());
    release_all(&mut law, &clocks, &mut held, now, &mut passed);
    let _ = session.playing.stream.pause();
    // The law's transport runs on alone until every score note before the
    // stop has closed, so every row of the jam is final.
    let stopped_at = clocks.audio.sample_at(now).unwrap_or(0);
    let closed = close_through(&mut law, stopped_at).map_err(refused);
    println!("{}", counted(&session));
    let report = closed.and_then(|()| verdicts(&mut law, &passed));
    host::outcome(report, device_error(&session).or(failed))
}

/// Prints the live take's verdicts: what the law took and refused, how the
/// note-ons kept the delivery allowance, the rows for the notes played, and how
/// many score notes the jam passed unplayed. The law shows a row only once it
/// is final, when its score note has closed; the jam ran the law's transport
/// on past the stop first, so the rows reach as far as the jam did, and every
/// one is final.
fn verdicts(law: &mut Law, passed: &[Passed]) -> Result<(), String> {
    let record = law.record().map_err(refused)?;
    let (ons, offs): (Vec<&Passed>, Vec<&Passed>) = passed.iter().partition(|p| p.down);
    let placed = |list: &[&Passed]| list.iter().filter(|p| p.sample.is_some()).count();
    let took = |list: &[&Passed]| {
        list.iter()
            .filter(|p| p.sample.is_some() && p.refused.is_none())
            .count()
    };
    println!(
        "{} keys went down: the law took {} note-ons and refused {}; {} came before law time \
         and were not passed. It took {} note-offs and refused {}.",
        ons.len(),
        took(&ons),
        placed(&ons) - took(&ons),
        ons.len() - placed(&ons),
        took(&offs),
        placed(&offs) - took(&offs)
    );
    for p in passed {
        if let (Some(sample), Some(r)) = (p.sample, &p.refused) {
            let what = if p.down { "note-on" } else { "note-off" };
            println!(
                "  {what} {} ({}) at law sample {sample}: {}",
                name(p.pitch),
                p.pitch,
                r.reason
            );
        }
    }
    if let Some((median, largest, past)) = delivery(passed) {
        let ms = |samples: i64| samples as f64 / 48.0;
        println!(
            "Delivery: each note-on reached the law a median {:.1} ms and at most {:.1} ms \
             after the law's playhead passed it; the allowance is 100 ms, and {past} came later.",
            ms(median),
            ms(largest)
        );
    }
    let (never, played): (Vec<&str>, Vec<&str>) = record
        .rows
        .lines()
        .partition(|r| r.ends_with(": never played"));
    let count = |word: &str| played.iter().filter(|r| r.ends_with(word)).count();
    println!("The law's rows for the notes you played, in the order they became final:");
    for row in &played {
        println!("  {row}");
    }
    println!(
        "match {}, early {}, late {}, wrong pitch {}, addition {}; the jam passed {} more score \
         notes that no note played, and the law's row for each says never played",
        count(": match"),
        count(": early"),
        count(": late"),
        count(": wrong pitch"),
        count(": addition"),
        never.len()
    );
    Ok(())
}

fn jitter(o: &Options) -> Result<(), String> {
    let output = device::choose_output(o.output.as_deref())?;
    let (_events_in, events_out) = RingBuffer::<Event>::new(1);
    let (readings_in, mut readings) = RingBuffer::<Reading>::new(1_024);
    // A silent stream: the synth has no events, so nothing is heard, and there
    // is nothing for a pre-roll to wait for.
    let shared = Arc::new(Shared::default());
    shared.covered.store(u64::MAX, Ordering::Release);
    let callback = Callback::new(
        Synth::new(0),
        events_out,
        None,
        readings_in,
        Arc::clone(&shared),
        true,
    );
    let playing = device::start(&output, callback, shared)?;
    println!("Measuring {} for 3 s with a silent stream...", output.name);
    let mut all = Vec::new();
    for _ in 0..600 {
        while let Ok(r) = readings.pop() {
            all.push(r);
        }
        thread::sleep(Duration::from_millis(5));
    }
    let latency_ms = playing.shared.latency_nanos.load(Ordering::Relaxed) as f64 / 1e6;
    // The first callbacks fill the device's buffer back to back; the clock is
    // read from the rest.
    let steady = all.get(10..).unwrap_or(&[]);
    match residuals(steady) {
        Some((rms, worst)) => println!(
            "Audio clock: {} callbacks; the playback readings stray from their line by \
             {rms:.2} samples RMS ({:.1} us), {worst:.2} at most ({:.1} us). Output latency \
             {latency_ms:.1} ms.",
            steady.len(),
            rms / 48.0 * 1_000.0,
            worst / 48.0 * 1_000.0
        ),
        None => println!("Audio clock: too few callbacks to fit ({})", all.len()),
    }
    console_jitter(&playing);
    let _ = playing.stream.pause();
    match playing.shared.error.load(Ordering::SeqCst) {
        0 => Ok(()),
        code => Err(device::kind_text(code).to_owned()),
    }
}

#[cfg(windows)]
fn console_jitter(playing: &Playing) {
    match host::console::measure(&playing.stream, 300) {
        Ok(mut delays) => {
            delays.sort_unstable();
            let at = |q: f64| {
                let i = ((delays.len() - 1) as f64 * q).round() as usize;
                delays.get(i).copied().unwrap_or(0) as f64 / 1e3
            };
            let mean = delays.iter().sum::<u64>() as f64 / delays.len() as f64;
            let sd = (delays
                .iter()
                .map(|d| (*d as f64 - mean).powi(2))
                .sum::<f64>()
                / delays.len() as f64)
                .sqrt();
            println!(
                "Console key path, {} keys written into the console and stamped when read: min \
                 {:.1} us, median {:.1} us, p99 {:.1} us, max {:.1} us, standard deviation \
                 {:.1} us. The keyboard's own scan and USB polling come before this and are not \
                 measured here.",
                delays.len(),
                at(0.0),
                at(0.5),
                at(0.99),
                at(1.0),
                sd / 1e3
            );
        }
        Err(e) => println!("Console key path: not measured: {e}"),
    }
}

#[cfg(not(windows))]
fn console_jitter(_playing: &Playing) {
    println!("Console key path: the keyboard fallback is built for Windows only");
}
