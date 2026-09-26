//! `host`: the native host of si-jam-sessions.
//!
//! ```text
//! cargo run -p host --release -- devices
//! cargo run -p host --release -- play [--piece <name>] [--output <index|name>] [--mute] [--voice piano|osc] [--samples <dir>]
//! cargo run -p host --release -- render <out.wav> [--piece <name>] [--voice piano|osc] [--samples <dir>]
//! cargo run -p host --release -- jam [--piece <name>] [--output <index|name>] [--midi <index|name> | --keyboard] [--mute] [--voice piano|osc] [--samples <dir>]
//! cargo run -p host --release -- notes <out.json> [--piece <name>]
//! cargo run -p host --release -- jitter [--output <index|name>]
//! cargo run -p host --release -- fetch-piano [--dir <dir>] [--archive <file>] [--keep-archive]
//! cargo run -p host --release -- preview <in.mid> <out.wav> [--samples <dir>]
//! cargo run -p host --release -- notices
//! ```
//!
//! Exit status: 0 when the command finished, 1 when it stopped on an error (a
//! refused law call, a device error, a bad argument), with the reason printed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::StreamTrait;
use golden::take::Perturbation;
use host::anchor::{Reading, residuals};
use host::bridge::{Law, Refused};
use host::callback::{Callback, Shared};
use host::device::{self, Output, Playing};
use host::event::{Event, Monitor};
use host::live::{Dropped, Passed, Press, Taker, close_through, delivery, release_all};
use host::piano::{self, Bank, Needs};
use host::schedule::{Scheduler, steps_for};
use host::score::{Name, Piece, clock, root};
use host::synth::Synth;
use host::{RING_EVENTS, fetch, notes, offline, preview};
use rtrb::{Consumer, Producer, RingBuffer};

const USAGE: &str = "usage:
  host devices                  list the audio outputs and the MIDI inputs
  host play [--piece P] [--output X] [--mute] [--voice piano|osc] [--samples DIR]
                                play the piece: a Battle Hymn arrangement is the score alone; The
                                Entertainer is its constructed take against the score and the click;
                                --mute renders and counts everything and sends the device silence
  host render <out.wav> [--piece P] [--voice piano|osc] [--samples DIR]
                                render the same mix to a WAV file, with no device
  host jam [--piece P] [--output X] [--midi X | --keyboard] [--mute] [--voice piano|osc] [--samples DIR]
                                play the score and the click, take a live take, print its verdicts
  host notes <out.json> [--piece P]
                                write the notes the law commits for the piece's score as compact
                                JSON: onset and length in samples at 48 kHz, pitch, velocity and
                                MIDI track, and the beats
  host jitter [--output X]      measure the audio clock's readings and the console's key path
  host fetch-piano [--dir DIR] [--archive FILE] [--keep-archive]
                                download the piano's samples (742 MB, CC BY 3.0) into the per-user
                                cache or DIR, check them against the SHA-256 the host pins, unpack them
  host preview <in.mid> <out.wav> [--samples DIR]
                                a PREVIEW: render a MIDI file straight through the piano, without the
                                law, to audition a draft; it is not the law's committed frames
  host notices                  print the licences of the crates and the samples the host uses
P is the piece: battle-hymn-glm-5.3 (the default) or battle-hymn-kimi-k3, Battle Hymn of the
Republic as each model arranged it, or entertainer, The Entertainer. The click sounds against a
take: The Entertainer's constructed take in play and render, and your live take in jam.
X is an index from `host devices` or part of a name.
--voice is the score's voice: the piano when fetch-piano has put its samples in the per-user cache
(or in the DIR --samples names), the oscillator otherwise. The take, your live notes and the click
are oscillators either way. The piano prints its credit whenever it plays.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.split_first() {
        Some((command, rest)) => match command.as_str() {
            "devices" => options(rest, Accepts::NONE, 0).and_then(|_| devices()),
            "play" => options(rest, Accepts::PLAY, 0).and_then(|o| play(&o)),
            "render" => options(rest, Accepts::RENDER, 1).and_then(|o| render(&o)),
            "jam" => options(rest, Accepts::JAM, 0).and_then(|o| jam(&o)),
            "notes" => options(rest, Accepts::NOTES, 1).and_then(|o| write_notes(&o)),
            "jitter" => options(rest, Accepts::OUTPUT, 0).and_then(|o| jitter(&o)),
            "fetch-piano" => options(rest, Accepts::FETCH, 0).and_then(|o| fetch_piano(&o)),
            "preview" => options(rest, Accepts::SAMPLES, 2).and_then(|o| preview(&o)),
            "notices" => options(rest, Accepts::NONE, 0).map(|_| print!("{}", host::notices::TEXT)),
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

/// The score's voice a command was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VoiceChoice {
    Piano,
    Osc,
}

#[derive(Debug, Default)]
struct Options {
    piece: Option<Name>,
    output: Option<String>,
    midi: Option<String>,
    keyboard: bool,
    mute: bool,
    voice: Option<VoiceChoice>,
    samples: Option<PathBuf>,
    dir: Option<PathBuf>,
    archive: Option<PathBuf>,
    keep_archive: bool,
    /// The command's arguments that are not options, in order.
    positional: Vec<String>,
}

impl Options {
    /// The piece `--piece` named, or the default.
    fn piece(&self) -> Name {
        self.piece.unwrap_or(Name::DEFAULT)
    }
}

/// The options a command takes.
#[derive(Clone, Copy, Debug)]
struct Accepts {
    /// `--piece`.
    piece: bool,
    output: bool,
    /// `--midi` and `--keyboard`.
    live: bool,
    mute: bool,
    voice: bool,
    samples: bool,
    /// `--dir`, `--archive` and `--keep-archive`.
    fetch: bool,
}

impl Accepts {
    const NONE: Accepts = Accepts {
        piece: false,
        output: false,
        live: false,
        mute: false,
        voice: false,
        samples: false,
        fetch: false,
    };
    const OUTPUT: Accepts = Accepts {
        output: true,
        ..Accepts::NONE
    };
    const SAMPLES: Accepts = Accepts {
        samples: true,
        ..Accepts::NONE
    };
    const VOICE: Accepts = Accepts {
        voice: true,
        ..Accepts::SAMPLES
    };
    const RENDER: Accepts = Accepts {
        piece: true,
        ..Accepts::VOICE
    };
    const PLAY: Accepts = Accepts {
        output: true,
        mute: true,
        ..Accepts::RENDER
    };
    const JAM: Accepts = Accepts {
        live: true,
        ..Accepts::PLAY
    };
    const FETCH: Accepts = Accepts {
        fetch: true,
        ..Accepts::NONE
    };
    const NOTES: Accepts = Accepts {
        piece: true,
        ..Accepts::NONE
    };
}

/// Reads a command's options, and exactly `positional` other arguments.
fn options(rest: &[String], accepts: Accepts, positional: usize) -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--piece" if accepts.piece && o.piece.is_none() => {
                let name = value()?;
                o.piece = Some(Name::parse(&name).ok_or_else(|| {
                    let names: Vec<&str> = Name::ALL.iter().map(|n| n.id()).collect();
                    format!("--piece is {}, not \"{name}\"", names.join(", "))
                })?);
            }
            "--output" if accepts.output && o.output.is_none() => o.output = Some(value()?),
            "--midi" if accepts.live && o.midi.is_none() => o.midi = Some(value()?),
            "--keyboard" if accepts.live && !o.keyboard => o.keyboard = true,
            "--mute" if accepts.mute && !o.mute => o.mute = true,
            "--voice" if accepts.voice && o.voice.is_none() => {
                o.voice = Some(match value()?.as_str() {
                    "piano" => VoiceChoice::Piano,
                    "osc" => VoiceChoice::Osc,
                    other => {
                        return Err(format!("--voice is piano or osc, not \"{other}\""));
                    }
                });
            }
            "--samples" if accepts.samples && o.samples.is_none() => {
                o.samples = Some(PathBuf::from(value()?));
            }
            "--dir" if accepts.fetch && o.dir.is_none() => o.dir = Some(PathBuf::from(value()?)),
            "--archive" if accepts.fetch && o.archive.is_none() => {
                o.archive = Some(PathBuf::from(value()?));
            }
            "--keep-archive" if accepts.fetch && !o.keep_archive => o.keep_archive = true,
            other if !other.starts_with("--") && o.positional.len() < positional => {
                o.positional.push(other.to_owned());
            }
            other => return Err(format!("unexpected argument \"{other}\"\n{USAGE}")),
        }
    }
    if o.positional.len() != positional {
        return Err(format!(
            "that command takes {positional} argument{} besides its options\n{USAGE}",
            if positional == 1 { "" } else { "s" }
        ));
    }
    if o.keyboard && o.midi.is_some() {
        return Err(String::from(
            "--midi and --keyboard name two inputs; pick one",
        ));
    }
    Ok(o)
}

/// The piano's samples for the notes a command will play, `(pitch,
/// velocity)`, loaded before any stream exists; `None` when the score plays
/// on the oscillator. The piano plays when `--voice piano` asks for it, and by
/// default when `fetch-piano` has put verified samples where `--samples` (or
/// the per-user cache) says; `--voice osc` plays the oscillator.
fn piano_for(
    o: &Options,
    notes: impl Iterator<Item = (u8, u8)>,
) -> Result<Option<Arc<Bank>>, String> {
    if o.voice == Some(VoiceChoice::Osc) {
        return Ok(None);
    }
    let asked = o.voice == Some(VoiceChoice::Piano);
    let dir = match fetch::dir_or_default(o.samples.as_deref()) {
        Ok(dir) => dir,
        Err(e) if !asked => {
            println!("The score plays on the oscillator: {e}.");
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    if !asked && fetch::verified(&dir).is_err() {
        println!(
            "The score plays on the oscillator: the piano's samples are not in {}. `host \
             fetch-piano` downloads them (742 MB).",
            dir.display()
        );
        return Ok(None);
    }
    let mut needs = Needs::default();
    for (pitch, velocity) in notes {
        needs.note(pitch, velocity);
    }
    load_piano(&dir, &needs).map(Some)
}

/// Loads the samples `needs` names from `dir`, and prints the piano's credit
/// and what it holds.
fn load_piano(dir: &Path, needs: &Needs) -> Result<Arc<Bank>, String> {
    let started = Instant::now();
    let bank = Bank::open(dir, needs)?;
    let (count, bytes) = bank.size();
    println!("{}", piano::CREDIT);
    println!(
        "  {count} samples loaded from {} in {:.1} s: {:.1} MiB of audio in memory.",
        dir.display(),
        started.elapsed().as_secs_f64(),
        bytes as f64 / 1_048_576.0
    );
    Ok(Arc::new(bank))
}

/// The synth a command plays through: the score on the piano when there is
/// one, and the click only when `click` says ([`click`]).
fn synth_for(piano: Option<&Arc<Bank>>, click: bool) -> Box<Synth> {
    let synth = match piano {
        Some(bank) => Synth::new(0).with_piano(Arc::clone(bank)),
        None => Synth::new(0),
    };
    if click { synth } else { synth.without_click() }
}

/// Whether the click sounds: against a take, the piece's constructed take or
/// the live take of a jam. A piece with no take, played or rendered, is the
/// score alone.
fn click(piece: &Piece, jam: bool) -> bool {
    jam || piece.take.is_some()
}

/// What `play` and `render` play, for their reports.
fn playing(piece: &Piece, piano: bool) -> String {
    let voice = if piano { "the piano" } else { "the oscillator" };
    match piece.take {
        Some(_) => format!(
            "{}'s constructed take, the score on {voice} and the click",
            piece.name.title()
        ),
        None => format!("{}, the score alone on {voice}", piece.name.title()),
    }
}

/// A render's loudest sample in dB below full scale, and how many samples
/// reached full scale and were clipped there.
fn level(samples: &[f32]) -> String {
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let clipped = samples.iter().filter(|s| s.abs() >= 1.0).count();
    if peak == 0.0 {
        return String::from("silent");
    }
    format!(
        "peak {:.1} dBFS, {clipped} samples at full scale",
        20.0 * peak.log10()
    )
}

/// The file's SHA-256, in hex.
fn sha256_hex(bytes: &[u8]) -> String {
    golden::run::hex(&golden::run::sha256(bytes))
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

/// The score's voice, as the listening guide names it.
fn score_voice(piano: bool) -> &'static str {
    if piano { "the piano" } else { "the pure one" }
}

/// Prints where to listen, for `play` and `render`, in time order: the notes
/// the seed drew, when the piece has a constructed take.
fn guide(piece: &Piece, piano: bool) {
    if piece.drawn.is_empty() {
        return;
    }
    let mut drawn = piece.drawn.clone();
    drawn.sort_by_key(|d| piece.score.note(d.note).map_or(0, |n| n.onset_sample));
    println!(
        "Where to listen (the take is the reedy voice, the score {}):",
        score_voice(piano)
    );
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

/// The law's rows for the five drawn notes, when the piece has a
/// constructed take.
fn drawn_rows(piece: &Piece, rows: &str) {
    if piece.drawn.is_empty() {
        return;
    }
    println!("The law's verdicts for them:");
    for d in &piece.drawn {
        let prefix = format!("note {}: ", d.note.0);
        if let Some(row) = rows.lines().find(|r| r.starts_with(&prefix)) {
            println!("  {row}");
        }
    }
}

/// The notes the score plays, for the piano's needs.
fn score_notes(piece: &Piece) -> impl Iterator<Item = (u8, u8)> + '_ {
    piece.score.notes().iter().map(|n| (n.pitch, n.velocity))
}

fn render(o: &Options) -> Result<(), String> {
    let path = o.positional.first().map_or("", String::as_str);
    let piece = Piece::load(&root(), o.piece())?;
    let piano = piano_for(o, score_notes(&piece))?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    if let Some(take) = &piece.take {
        law.admit_take(take).map_err(refused)?;
    }
    let end = piece.stop();
    let synth = synth_for(piano.as_ref(), click(&piece, false));
    let (samples, channels, counts) =
        offline::render_with(&mut law, end, 480, synth).map_err(refused)?;
    let info: &[([u8; 4], &str)] = if piano.is_some() {
        &offline::PIANO_INFO
    } else {
        &[]
    };
    let mut bytes = Vec::with_capacity(samples.len() * 4 + 512);
    offline::write_wav_with(&mut bytes, &samples, channels as u16, info)
        .map_err(|e| e.to_string())?;
    std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
    let record = law.record().map_err(refused)?;
    let frames = samples.len() / channels.max(1);
    println!(
        "Rendered {frames} frames ({}) of {}, to {path}: {} 32-bit float at 48 kHz; frame n of \
         the file is law sample n.",
        clock(frames as u64),
        playing(&piece, piano.is_some()),
        if channels == 2 { "stereo" } else { "mono" }
    );
    println!(
        "  {} notes and {} beats started, {} late, {} dropped; {}; file SHA-256 {}",
        counts.notes,
        counts.beats,
        counts.late,
        counts.dropped,
        level(&samples),
        sha256_hex(&bytes)
    );
    if piano.is_some() {
        println!("  The WAV carries the piano's credit in its LIST/INFO chunk (ICMT).");
    }
    guide(&piece, piano.is_some());
    drawn_rows(&piece, &record.rows);
    Ok(())
}

/// `notes`: the notes the law commits for the piece's score, as compact JSON
/// ([`notes`]), read through the law's C ABI and never from the MIDI file.
fn write_notes(o: &Options) -> Result<(), String> {
    let path = o.positional.first().map_or("", String::as_str);
    let piece = Piece::load(&root(), o.piece())?;
    let mut law = Law::acquire();
    let read = notes::read(&mut law, &piece).map_err(refused)?;
    let json = read.json();
    std::fs::write(path, &json).map_err(|e| format!("{path}: {e}"))?;
    println!(
        "Wrote the {} notes and {} beats the law commits for {} to {path}: {} bytes of JSON, \
         SHA-256 {}. The law's snapshot of the score hashes to {}.",
        read.notes.len(),
        read.beats.len(),
        piece.name.title(),
        json.len(),
        sha256_hex(json.as_bytes()),
        golden::run::hex(&read.golden)
    );
    Ok(())
}

/// `fetch-piano`: the piano's samples downloaded, verified and unpacked.
fn fetch_piano(o: &Options) -> Result<(), String> {
    let dir = fetch::dir_or_default(o.dir.as_deref())?;
    println!("{}", piano::CREDIT);
    let fetched = fetch::fetch(&dir, o.archive.as_deref(), o.keep_archive)?;
    if fetched.fetched {
        println!("Unpacked {} files into {}.", fetched.files, dir.display());
        if let Some(kept) = &fetched.kept {
            println!("  The archive is kept at {}.", kept.display());
        }
    } else {
        println!(
            "{} already holds the piano's samples, fetched and verified against SHA-256 {}.",
            dir.display(),
            fetch::ARCHIVE_SHA256
        );
    }
    println!(
        "  The samples are CC BY 3.0: the host prints their credit whenever the piano plays and \
         writes it into every WAV it renders, and `host notices` prints the licence. To remove \
         them, delete {}.",
        dir.display()
    );
    Ok(())
}

/// What a preview's WAV says it is, in its LIST/INFO chunk.
const PREVIEW_LABEL: &str = "si-jam-sessions host preview: a MIDI file rendered on the piano \
                             without the law; not the law's committed frames";

/// `preview`: a MIDI file straight through the piano, without the law.
fn preview(o: &Options) -> Result<(), String> {
    let (midi, wav) = match o.positional.as_slice() {
        [midi, wav] => (midi, wav),
        _ => return Err(String::from("preview takes a MIDI file and a WAV to write")),
    };
    println!(
        "PREVIEW, not the law's committed frames: {midi} rendered straight through the piano, \
         without the law (no receipt, no licence predicate, no step, no hash)."
    );
    let bytes = std::fs::read(midi).map_err(|e| format!("{midi}: {e}"))?;
    let draft = preview::read(&bytes).map_err(|e| format!("{midi}: {e}"))?;
    let dir = fetch::dir_or_default(o.samples.as_deref())?;
    let bank = load_piano(&dir, &draft.needs)?;
    let (samples, counts) = preview::render(&draft, bank);
    let mut out = Vec::with_capacity(samples.len() * 4 + 512);
    offline::write_wav_with(
        &mut out,
        &samples,
        2,
        &[(*b"ICMT", piano::CREDIT), (*b"ISFT", PREVIEW_LABEL)],
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(wav, &out).map_err(|e| format!("{wav}: {e}"))?;
    let bpm = if draft.first_us_per_quarter == 0 {
        0.0
    } else {
        60_000_000.0 / f64::from(draft.first_us_per_quarter)
    };
    println!(
        "  {} notes at PPQ {}; {} tempo change{}, the first {bpm:.1} quarter notes a minute.",
        draft.events.len(),
        draft.ppq,
        draft.tempos,
        if draft.tempos == 1 { "" } else { "s" }
    );
    if draft.pedal > 0 {
        println!(
            "  {} sustain-pedal messages were ignored: the note lengths are the sustain.",
            draft.pedal
        );
    }
    if counts.outside > 0 {
        println!(
            "  {} notes are off the piano's keyboard (A0 to C8) and were not played.",
            counts.outside
        );
    }
    let frames = samples.len() / 2;
    println!(
        "Wrote a PREVIEW of {frames} frames ({}) to {wav}: stereo 32-bit float at 48 kHz; {} \
         notes played, {} dropped; {}; SHA-256 {}. It is not the law's committed frames.",
        clock(frames as u64),
        counts.notes,
        counts.dropped,
        level(&samples),
        sha256_hex(&out)
    );
    Ok(())
}

/// Rings, the scheduler and a started stream, for `play` and `jam`.
struct Session {
    playing: Playing,
    scheduler: Scheduler,
    events: Producer<Event>,
    readings: Consumer<Reading>,
}

/// Opens `output` and starts its stream, the score on the piano when `piano`
/// holds its samples, which were loaded before this is called, and the click
/// when `click` says.
fn open(
    output: &Output,
    law: &mut Law,
    monitor: Option<(Consumer<Monitor>, Consumer<Monitor>)>,
    mute: bool,
    piano: Option<&Arc<Bank>>,
    click: bool,
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
    let synth = synth_for(piano, click);
    let callback = match monitor {
        None => Callback::new(
            synth,
            events_out,
            None,
            readings_in,
            Arc::clone(&shared),
            mute,
        ),
        Some((monitor, hush)) => Callback::new(
            synth,
            events_out,
            Some(monitor),
            readings_in,
            Arc::clone(&shared),
            mute,
        )
        .with_hush(hush),
    };
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
    /// the horizon covers the next two callbacks ([`Shared::lookahead`]).
    /// Then fills the ring, and tells the callback how far it is complete.
    fn pump(&mut self, law: &mut Law, heard: Option<i64>) -> Result<(), String> {
        let shared = &self.playing.shared;
        let frame = shared.frames.load(Ordering::Acquire);
        let target = steps_for(frame, shared.lookahead(), heard);
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
         first callback asked for {} frames, the one that started law time for {}, the largest \
         after it for {}, and {} frames of silence came before law sample 0",
        s.notes.load(Ordering::Relaxed),
        s.beats.load(Ordering::Relaxed),
        s.monitored.load(Ordering::Relaxed),
        s.late.load(Ordering::Relaxed),
        s.dropped.load(Ordering::Relaxed),
        s.first_buffer.load(Ordering::Relaxed),
        s.starting_buffer.load(Ordering::Relaxed),
        s.largest_buffer.load(Ordering::Relaxed),
        s.preroll_frames.load(Ordering::Relaxed)
    )
}

fn play(o: &Options) -> Result<(), String> {
    let piece = Piece::load(&root(), o.piece())?;
    let output = device::choose_output(o.output.as_deref())?;
    // Every sample is in memory before the stream exists.
    let piano = piano_for(o, score_notes(&piece))?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    if let Some(take) = &piece.take {
        law.admit_take(take).map_err(refused)?;
    }
    let clicks = click(&piece, false);
    let mut session = open(&output, &mut law, None, o.mute, piano.as_ref(), clicks)?;
    describe(&output, &session);
    guide(&piece, piano.is_some());
    let muted = if o.mute { ", muted" } else { "" };
    println!(
        "Playing {} ({}){muted}; press Enter to stop.",
        playing(&piece, piano.is_some()),
        clock(piece.end)
    );
    let stop = stop_flag(&session);
    stop_on_enter(Arc::clone(&stop));
    let end = piece.stop();
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
    /// began, and returns true then.
    fn watch(&mut self) -> bool {
        let now = host::winmm::port_count();
        if !self.told && now < self.ports {
            self.told = true;
            println!(
                "  WinMM now lists {now} MIDI input ports, {} when the jam began. If the \
                 keyboard was unplugged, its notes stopped arriving; WinMM does not say so \
                 otherwise. The keys held now are released in the monitor.",
                self.ports
            );
            return true;
        }
        false
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
    dropped: Arc<Dropped>,
) -> Result<Started, String> {
    match input {
        Input::Midi(port, name) => {
            let errors = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let sink = host::winmm::Sink {
                monitor,
                input: presses,
                stream: Arc::clone(&session.playing.stream),
                errors: Arc::clone(&errors),
                dropped,
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
            let keys = thread::spawn(move || {
                host::console::read(&flag, &stream, monitor, presses, &dropped)
            });
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
    _dropped: Arc<Dropped>,
) -> Result<Started, String> {
    println!(
        "Live input: none; live input is built for Windows. Playing the score and the click. \
         Press Enter to stop."
    );
    stop_on_enter(Arc::clone(stop));
    Ok(Started::None)
}

/// Watches the input while the jam runs: true when a MIDI input may have
/// gone away (WinMM lists fewer ports than when the jam began).
fn watch_input(started: &mut Started) -> bool {
    match started {
        #[cfg(windows)]
        Started::Midi(midi) => midi.watch(),
        #[cfg(windows)]
        Started::Keyboard(_) => false,
        #[cfg(not(windows))]
        Started::None => false,
    }
}

/// What stopping an input leaves for the jam's exit status: the failure the
/// input stopped on, if it stopped on one. `closed` is how closing it went: a
/// close that fails at teardown comes after the music is over, so it is
/// printed as a warning and fails nothing. Only the MIDI input, which is
/// Windows-only, closes a port; the rule's test runs everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
fn stopped_input(stopped_on: Option<String>, closed: Result<(), String>) -> Option<String> {
    if let Err(e) = closed {
        eprintln!("host: warning: {e}");
    }
    stopped_on
}

/// Stops the input, and reports what it said on the way: WinMM's invalid
/// messages and what full rings dropped. Returns the input's failure, if it
/// stopped on one: then the jam exits 1.
fn finish_input(started: Started, dropped: &Dropped) -> Option<String> {
    let failure = match started {
        #[cfg(windows)]
        Started::Midi(midi) => {
            let errors = midi.errors.load(Ordering::Relaxed);
            if errors > 0 {
                println!("WinMM reported {errors} invalid MIDI messages (MIM_ERROR).");
            }
            stopped_input(None, midi.port.close())
        }
        #[cfg(windows)]
        Started::Keyboard(keys) => match keys.join() {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(format!("the keyboard stopped: {e}")),
            Err(_) => Some(String::from("the keyboard reader stopped")),
        },
        #[cfg(not(windows))]
        Started::None => None,
    };
    let (presses, monitor) = dropped.counts();
    if presses > 0 || monitor > 0 {
        println!(
            "The input dropped {presses} presses and {monitor} monitor messages: a ring was \
             full."
        );
    }
    if let Some(e) = &failure {
        eprintln!("host: {e}");
    }
    failure
}

fn jam(o: &Options) -> Result<(), String> {
    let piece = Piece::load(&root(), o.piece())?;
    let input = choose_input(o)?;
    let output = device::choose_output(o.output.as_deref())?;
    // Every sample is in memory before the stream exists.
    let piano = piano_for(o, score_notes(&piece))?;
    let mut law = Law::acquire();
    law.ingest(&piece.container).map_err(refused)?;
    let (monitor_in, monitor_out) = RingBuffer::<Monitor>::new(256);
    let (mut hush_in, hush_out) = RingBuffer::<Monitor>::new(256);
    let (presses_in, mut presses) = RingBuffer::<Press>::new(4_096);
    let mut session = open(
        &output,
        &mut law,
        Some((monitor_out, hush_out)),
        o.mute,
        piano.as_ref(),
        click(&piece, true),
    )?;
    describe(&output, &session);
    let stop = stop_flag(&session);
    let dropped = Arc::new(Dropped::default());
    let mut started = start_input(
        input,
        &session,
        &stop,
        monitor_in,
        presses_in,
        Arc::clone(&dropped),
    )?;
    println!(
        "Play along with {} ({}) and the click; your notes sound in the reedy voice. The score \
         ends at {}.",
        piece.name.title(),
        if piano.is_some() {
            "the piano"
        } else {
            "the pure voice"
        },
        clock(piece.end)
    );

    let mut taker = Taker::default();
    let end = piece.stop();
    let mut failed = None;
    let mut latency = Latency::default();
    let mut startup = Startup::default();
    let mut watched = std::time::Instant::now();
    while !stop.load(Ordering::Relaxed) {
        latency.watch(&output, &session);
        startup.watch(&session);
        // Whether the input may have gone away is read before the presses
        // are taken, so every press it sent first is in the ring by then; a
        // key held on it gets no note-off, and `step` releases its monitor
        // voice after taking the presses.
        let gone = watched.elapsed() >= Duration::from_millis(500) && {
            watched = std::time::Instant::now();
            watch_input(&mut started)
        };
        let frame = session.playing.shared.frames.load(Ordering::Acquire);
        if frame >= end {
            break;
        }
        // Presses first, at the playhead they were played under; then the
        // law steps to the sample heard now.
        taker.step(
            &mut law,
            &mut session.readings,
            &mut presses,
            gone,
            &mut hush_in,
        );
        let heard = taker
            .clocks
            .audio
            .sample_at(device::nanos(session.playing.stream.now()));
        if let Err(e) = session.pump(&mut law, heard) {
            failed = Some(e);
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    stop.store(true, Ordering::SeqCst);
    let input_failed = finish_input(started, &dropped);
    // Presses still queued pass now, and notes still held end now.
    taker.take(&mut law, &mut session.readings, &mut presses);
    let now = device::nanos(session.playing.stream.now());
    release_all(
        &mut law,
        &taker.clocks,
        &mut taker.held,
        now,
        &mut taker.passed,
    );
    let _ = session.playing.stream.pause();
    // The law's transport runs on alone until every score note before the
    // stop has closed, so every row of the jam is final.
    let stopped_at = taker.clocks.audio.sample_at(now).unwrap_or(0);
    let closed = close_through(&mut law, stopped_at).map_err(refused);
    println!("{}", counted(&session));
    let report = closed.and_then(|()| verdicts(&mut law, &taker.passed));
    host::outcome(report, device_error(&session).or(failed).or(input_failed))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #7's third finding. A MIDI port that will not close when the
    /// jam is over does not fail the jam: the music finished, and the close
    /// error is printed as a warning. An input that stopped on a failure of
    /// its own still fails it.
    #[test]
    fn a_close_error_at_teardown_does_not_fail_a_clean_jam() {
        let close = || Err(String::from("the MIDI port did not close (WinMM error 5)"));
        assert_eq!(stopped_input(None, close()), None);
        assert_eq!(stopped_input(None, Ok(())), None);
        assert_eq!(
            stopped_input(Some(String::from("the keyboard stopped")), close()),
            Some(String::from("the keyboard stopped"))
        );
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| String::from(*s)).collect()
    }

    /// `play`, `render`, `jam` and `notes` take `--piece`, and with none they
    /// play the Battle Hymn as glm-5.3 arranged it. A name that is not a
    /// piece is refused with the names that are, and the commands that play
    /// no piece refuse the option.
    #[test]
    fn the_playing_commands_take_a_piece_and_open_on_the_battle_hymn() {
        for (accepts, positional) in [
            (Accepts::PLAY, &[][..]),
            (Accepts::RENDER, &["out.wav"][..]),
            (Accepts::JAM, &[][..]),
            (Accepts::NOTES, &["out.json"][..]),
        ] {
            let n = positional.len();
            let none = options(&args(positional), accepts, n).unwrap();
            assert_eq!(none.piece, None);
            assert_eq!(none.piece(), Name::BattleHymnGlm53);
            for name in Name::ALL {
                let mut list = positional.to_vec();
                list.extend(["--piece", name.id()]);
                let o = options(&args(&list), accepts, n).unwrap();
                assert_eq!(o.piece(), name);
            }
            let mut list = positional.to_vec();
            list.extend(["--piece", "battle-hymn"]);
            let e = options(&args(&list), accepts, n).unwrap_err();
            for name in Name::ALL {
                assert!(e.contains(name.id()), "{e}");
            }
            let mut twice = positional.to_vec();
            twice.extend(["--piece", "entertainer", "--piece", "entertainer"]);
            assert!(options(&args(&twice), accepts, n).is_err());
        }
        for (accepts, positional) in [
            (Accepts::NONE, &[][..]),
            (Accepts::OUTPUT, &[][..]),
            (Accepts::SAMPLES, &["in.mid", "out.wav"][..]),
            (Accepts::FETCH, &[][..]),
        ] {
            let mut list = positional.to_vec();
            list.extend(["--piece", "entertainer"]);
            assert!(options(&args(&list), accepts, positional.len()).is_err());
        }
    }

    /// The help names every piece and the default, and gives `--piece` to
    /// each command that plays one.
    #[test]
    fn the_help_names_the_pieces_and_the_default() {
        for name in Name::ALL {
            assert!(USAGE.contains(name.id()), "{}", name.id());
        }
        assert!(USAGE.contains("battle-hymn-glm-5.3 (the default)"));
        for command in [
            "host play [--piece P]",
            "host render <out.wav> [--piece P]",
            "host jam [--piece P]",
            "host notes <out.json> [--piece P]",
        ] {
            assert!(USAGE.contains(command), "{command}");
        }
        assert!(!USAGE.contains("play The Entertainer's constructed take against"));
    }

    /// The click sounds against a take: The Entertainer's constructed take,
    /// and a live take in `jam`. A Battle Hymn played or rendered is the
    /// performance alone.
    #[test]
    fn the_click_sounds_against_a_take() {
        let root = root();
        let entertainer = Piece::load(&root, Name::Entertainer).unwrap();
        let hymn = Piece::load(&root, Name::BattleHymnGlm53).unwrap();
        assert!(click(&entertainer, false));
        assert!(!click(&hymn, false));
        assert!(click(&hymn, true));
        assert!(click(&entertainer, true));
    }
}
