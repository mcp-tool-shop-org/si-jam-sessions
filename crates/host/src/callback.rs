//! The audio callback's body, apart from cpal.
//!
//! [`Callback::run`] is what the stream's data callback calls, and all it
//! calls. It lives outside [`crate::device`] so the tests can hand it the
//! buffers a device would, the first fill among them, with no device. It
//! allocates nothing, takes no lock and makes no system call: it renders
//! through [`Synth::render`], pops and pushes `rtrb` rings made before the
//! stream, and stores atomics (`alloc_free` counts it).
//!
//! # The silent pre-roll
//!
//! Before the stream starts, the scheduler has pushed every committed event of
//! the first H + 1 quanta, law samples 0 to 4,847, and published how far the
//! ring is complete ([`Shared::covered`]). A device's first callback can ask
//! for more than that: WASAPI hands the first callback the whole empty device
//! buffer. Rendering such a callback from law sample 0 would take every event
//! between the ring's cover and the callback's end after its onset: late.
//!
//! So law time starts at the first callback whose frames the ring covers.
//! Until then each callback is silence, and law time does not move: the synth
//! stays at law sample 0, takes no event and pushes no [`Reading`], and the
//! scheduler, which steps the law to the synth's position, has nothing to do.
//! The first callback that fits starts at law sample 0 with everything it
//! needs in the ring, so the startup fill makes no event late, whatever the
//! score. After [`PREROLL_CAP_FRAMES`] of silence law time starts at the next
//! callback whatever its size, so a device whose every callback is larger than
//! the ring's cover still plays, and the synth counts what arrives late.
//!
//! Once law time has started it runs with the device: the pre-roll happens
//! once, at the start.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use rtrb::{Consumer, Producer};

use crate::anchor::Reading;
use crate::event::{Event, Monitor};
use crate::synth::{RATE, Synth};

/// One second: the most silence the pre-roll renders before law time starts
/// at the next callback whatever its size.
pub const PREROLL_CAP_FRAMES: u64 = RATE as u64;

/// What the callback and the error callback share with the rest of the host.
#[derive(Default)]
pub struct Shared {
    /// The law sample of the next frame to render: frames rendered since law
    /// time started.
    pub frames: AtomicU64,
    /// Published by the law thread after every pump: the law sample before
    /// which every committed event is in the ring (see
    /// [`crate::schedule::Pumped::covered`]).
    pub covered: AtomicU64,
    /// Law time has started: the pre-roll is over.
    pub started: AtomicBool,
    /// Frames of silence rendered before law time started.
    pub preroll_frames: AtomicU64,
    /// The frames the device's first callback asked for.
    pub first_buffer: AtomicU64,
    /// Raised by the error callback, or by the command to stop.
    pub stop: AtomicBool,
    /// The error callback's error, as [`crate::device::kind_code`] numbers
    /// it; 0 for none.
    pub error: AtomicU32,
    /// The latest `playback - callback`, in nanoseconds: the output latency
    /// cpal predicts.
    pub latency_nanos: AtomicU64,
    pub callbacks: AtomicU64,
    /// The frames of the callback that started law time. It stands in for the
    /// lookahead until a later callback has rendered law time
    /// ([`Shared::lookahead`]).
    pub starting_buffer: AtomicU64,
    /// The largest callback that rendered law time after the one that started
    /// it, in frames: the pre-roll's silent callbacks and the starting callback
    /// are not counted.
    pub largest_buffer: AtomicU64,
    pub notes: AtomicU64,
    pub beats: AtomicU64,
    pub late: AtomicU64,
    pub dropped: AtomicU64,
    pub monitored: AtomicU64,
}

impl Shared {
    /// The frames the next callbacks may ask for before the law thread pumps
    /// again, which [`crate::schedule::steps_for`] keeps the committed horizon
    /// ahead of: two of the largest callbacks that rendered law time after the
    /// one that started it, or two of the starting callback until one has.
    ///
    /// The starting callback is a one-off: a device's first callback fills its
    /// whole buffer, and later ones ask for a period. Held as the lookahead for
    /// the whole jam, a covered first fill of a few thousand frames keeps the
    /// playhead that far ahead of the ear, and a key played up to 80 ms late
    /// can reach the law after its score note has closed (issue #7). Left out
    /// entirely, it leaves the law stepped only to the ring's first cover, and
    /// a device asked for half its buffer at a time (4,800 frames, then 2,400
    /// a callback) renders its second callback past that cover. So the
    /// starting callback counts only until the next callback has rendered.
    pub fn lookahead(&self) -> u64 {
        let largest = self.largest_buffer.load(Ordering::Relaxed);
        let frames = if largest == 0 {
            self.starting_buffer.load(Ordering::Relaxed)
        } else {
            largest
        };
        frames.saturating_mul(2)
    }
}

/// Everything the audio callback owns: the synth, and its ends of the rings.
pub struct Callback {
    synth: Box<Synth>,
    events: Consumer<Event>,
    monitor: Option<Consumer<Monitor>>,
    /// Monitor messages from the law thread: note-offs for keys whose input
    /// went away while they were held.
    hush: Option<Consumer<Monitor>>,
    readings: Producer<Reading>,
    shared: Arc<Shared>,
    mute: bool,
    started: bool,
    silent: u64,
    cap: u64,
}

impl Callback {
    /// The callback's state, made before the stream: the synth renders from
    /// law sample 0 once the pre-roll ends. With `mute`, everything is
    /// rendered and counted as usual and the device is sent silence.
    pub fn new(
        synth: Box<Synth>,
        events: Consumer<Event>,
        monitor: Option<Consumer<Monitor>>,
        readings: Producer<Reading>,
        shared: Arc<Shared>,
        mute: bool,
    ) -> Callback {
        Callback {
            synth,
            events,
            monitor,
            hush: None,
            readings,
            shared,
            mute,
            started: false,
            silent: 0,
            cap: PREROLL_CAP_FRAMES,
        }
    }

    /// The same, taking monitor messages from the law thread too, through a
    /// second ring made before the stream (the input's ring has one producer,
    /// the input).
    pub fn with_hush(mut self, hush: Consumer<Monitor>) -> Callback {
        self.hush = Some(hush);
        self
    }

    /// The same, with the pre-roll capped at `cap` frames of silence; 0 turns
    /// it off.
    pub fn with_cap(mut self, cap: u64) -> Callback {
        self.cap = cap;
        self
    }

    /// One callback: `data` has `channels` interleaved channels; its first
    /// frame is heard at stream instant `playback` (nanoseconds), and
    /// `latency` is `playback` less the callback's own instant, when cpal
    /// gives both.
    pub fn run(&mut self, data: &mut [f32], channels: usize, playback: u64, latency: Option<u64>) {
        let frames = u64::try_from(data.len() / channels.max(1)).unwrap_or(u64::MAX);
        let s = &*self.shared;
        if let Some(nanos) = latency {
            s.latency_nanos.store(nanos, Ordering::Relaxed);
        }
        if s.callbacks.fetch_add(1, Ordering::Relaxed) == 0 {
            s.first_buffer.store(frames, Ordering::Relaxed);
        }

        let mut starting = false;
        if !self.started {
            let end = self.synth.frame().saturating_add(frames);
            if end <= s.covered.load(Ordering::Acquire) || self.silent >= self.cap {
                self.started = true;
                starting = true;
                s.started.store(true, Ordering::Release);
            } else {
                data.fill(0.0);
                self.silent = self.silent.saturating_add(frames);
                s.preroll_frames.store(self.silent, Ordering::Relaxed);
                return;
            }
        }

        // Only a callback that renders law time sets the lookahead a jam steps
        // the law with: the pre-roll's one-off first fill would hold the
        // playhead that far ahead of the ear for the rest of the jam. The
        // callback that starts law time stands in until the next one renders
        // ([`Shared::lookahead`]).
        if starting {
            s.starting_buffer.store(frames, Ordering::Relaxed);
        } else {
            s.largest_buffer.fetch_max(frames, Ordering::Relaxed);
        }
        if let Some(hush) = self.hush.as_mut() {
            while let Ok(heard) = hush.pop() {
                self.synth.hear(heard);
            }
        }
        let _ = self.readings.push(Reading {
            sample: self.synth.frame(),
            nanos: playback,
        });
        self.synth
            .render(data, channels, &mut self.events, self.monitor.as_mut());
        if self.mute {
            data.fill(0.0);
        }
        let counts = self.synth.counts();
        s.notes.store(counts.notes, Ordering::Relaxed);
        s.beats.store(counts.beats, Ordering::Relaxed);
        s.late.store(counts.late, Ordering::Relaxed);
        s.dropped.store(counts.dropped, Ordering::Relaxed);
        s.monitored.store(counts.monitored, Ordering::Relaxed);
        s.frames.store(self.synth.frame(), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Law;
    use crate::offline;
    use crate::schedule::{Scheduler, steps_for};
    use law::wire::encode_score;
    use law::{HORIZON_QUANTA, QUANTUM_SAMPLES};
    use rtrb::RingBuffer;
    use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};

    /// The law's cover at the start: quanta 0 to H, law samples 0..4,848.
    const LEAD: u64 = (HORIZON_QUANTA as u64 + 1) * QUANTUM_SAMPLES as u64;

    /// A score with an event at least every 10 ms from law sample 0 for two
    /// seconds: a note of 240 samples on every 480th sample, and a 4/4 beat on
    /// every 3,360th. One tick is one sample: 70,000 us a quarter at source PPQ
    /// 3,360.
    fn dense() -> Vec<u8> {
        let notes = (0..200u64)
            .map(|i| IngestedNote {
                start_tick: i * 480,
                pitch: 48 + (i % 36) as u8,
                track: 1,
                channel: 0,
                end_tick: i * 480 + 240,
                velocity: 90,
            })
            .collect();
        encode_score(&IngestedScore {
            source_ppq: 3_360,
            tempo: vec![TempoChange {
                tick: 0,
                us_per_quarter: 70_000,
            }],
            meter: vec![MeterChange {
                tick: 0,
                numerator: 4,
                denominator_pow2: 2,
            }],
            notes,
        })
        .unwrap()
    }

    /// Events of `dense` with an onset in `from..to`: (notes, beats).
    fn between(from: u64, to: u64) -> (u64, u64) {
        let notes = (0..200u64)
            .filter(|i| (from..to).contains(&(i * 480)))
            .count();
        let beats = (0..100u64)
            .filter(|k| (from..to).contains(&(k * 3_360)))
            .count();
        (notes as u64, beats as u64)
    }

    /// Runs the host's pipeline against a simulated device: the scheduler
    /// pumps once before the stream, as `open` does, and once between
    /// callbacks, as the law thread does; the device asks for `first` frames
    /// and then `rest` frames a callback until law time reaches `end`.
    /// Returns the frames the device was sent, mono, the shared counters and
    /// the readings pushed.
    fn device(
        cap: u64,
        first: usize,
        rest: usize,
        end: u64,
    ) -> (Vec<f32>, Arc<Shared>, Vec<Reading>) {
        let mut law = Law::acquire();
        law.load_score(&dense()).unwrap();
        let (mut events, events_out) = RingBuffer::new(crate::RING_EVENTS);
        let (readings_in, mut readings_out) = RingBuffer::new(4_096);
        let shared = Arc::new(Shared::default());
        let mut scheduler = Scheduler::new(0);
        let pumped = scheduler.pump(&mut law, 1, &mut events).unwrap();
        shared.covered.store(pumped.covered, Ordering::Release);
        let mut callback = Callback::new(
            Synth::new(0),
            events_out,
            None,
            readings_in,
            Arc::clone(&shared),
            false,
        )
        .with_cap(cap);
        let mut sent = Vec::new();
        let mut size = first;
        let mut heard = 1_000_000_000u64;
        while shared.frames.load(Ordering::Acquire) < end {
            // A pre-roll that never ends would never reach `end`.
            let silent = shared.preroll_frames.load(Ordering::Relaxed);
            assert!(silent <= 10 * PREROLL_CAP_FRAMES, "law time never started");
            let mut buffer = vec![0.0f32; size];
            callback.run(&mut buffer, 1, heard, Some(20_000_000));
            heard += size as u64 * 1_000_000_000 / 48_000;
            sent.extend_from_slice(&buffer);
            let frame = shared.frames.load(Ordering::Acquire);
            let target = steps_for(frame, 0, None);
            let pumped = scheduler.pump(&mut law, target, &mut events).unwrap();
            shared.covered.store(pumped.covered, Ordering::Release);
            size = rest;
        }
        let mut pushed = Vec::new();
        while let Ok(r) = readings_out.pop() {
            pushed.push(r);
        }
        (sent, shared, pushed)
    }

    /// The same score rendered offline, 480 frames a block.
    fn offline(end: u64) -> Vec<f32> {
        let mut law = Law::acquire();
        law.load_score(&dense()).unwrap();
        offline::render(&mut law, end, 480).unwrap().0
    }

    /// A slow first callback: the device's first fill asks for a whole second,
    /// 48,000 frames, ten times the ring's cover. The pre-roll renders it as
    /// silence and law time starts with the next callback, so no event is late,
    /// those of the first H quanta and those the fill would have spanned alike:
    /// after the second of silence, the device is sent the offline render,
    /// sample for sample.
    #[test]
    fn a_slow_first_callback_makes_no_event_late() {
        let end = 96_000;
        let (sent, shared, readings) = device(PREROLL_CAP_FRAMES, 48_000, 480, end);
        assert_eq!(shared.first_buffer.load(Ordering::Relaxed), 48_000);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 48_000);
        assert!(shared.started.load(Ordering::Relaxed));
        assert_eq!(shared.late.load(Ordering::Relaxed), 0);
        assert_eq!(
            (
                shared.notes.load(Ordering::Relaxed),
                shared.beats.load(Ordering::Relaxed)
            ),
            between(0, end)
        );
        let (silence, played) = sent.split_at(48_000);
        assert!(silence.iter().all(|s| *s == 0.0));
        assert_ne!(
            played[0], 0.0,
            "law sample 0 is the first sample after the pre-roll"
        );
        let expected = offline(end);
        assert_eq!(played.len(), expected.len());
        assert!(
            played == expected.as_slice(),
            "the played frames are the offline render"
        );
        // The first reading is law sample 0, heard when the second callback's
        // first frame is: no reading came from the pre-roll.
        assert_eq!(
            readings.first(),
            Some(&Reading {
                sample: 0,
                nanos: 2_000_000_000
            })
        );
    }

    /// The negative control: with the pre-roll off, the same first fill is
    /// rendered from law sample 0, and every event between the ring's cover
    /// and the fill's end arrives late.
    #[test]
    fn without_the_pre_roll_the_first_fill_makes_events_late() {
        let end = 96_000;
        let (sent, shared, _) = device(0, 48_000, 480, end);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 0);
        // Every event from the cover, law sample 4,848, to the fill's end.
        let (notes, beats) = between(LEAD, 48_000);
        assert_eq!((notes, beats), (89, 13));
        assert_eq!(shared.late.load(Ordering::Relaxed), notes + beats);
        assert!(sent != offline(end), "the late events start late");
    }

    /// A first callback the ring covers needs no pre-roll: law time starts at
    /// once. One frame more than the cover waits one callback.
    #[test]
    fn a_first_callback_the_ring_covers_starts_at_once() {
        let (_, shared, _) = device(PREROLL_CAP_FRAMES, LEAD as usize, 480, 20_000);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 0);
        assert_eq!(shared.late.load(Ordering::Relaxed), 0);
        let (_, shared, _) = device(PREROLL_CAP_FRAMES, LEAD as usize + 1, 480, 20_000);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), LEAD + 1);
        assert_eq!(shared.late.load(Ordering::Relaxed), 0);
    }

    /// A device whose every callback is larger than the ring's cover: the
    /// pre-roll ends at its cap, law time starts, and the events that arrive
    /// late are counted.
    #[test]
    fn the_pre_roll_ends_at_its_cap() {
        let (_, shared, _) = device(PREROLL_CAP_FRAMES, 6_000, 6_000, 48_000);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 48_000);
        assert!(shared.started.load(Ordering::Relaxed));
        assert!(shared.late.load(Ordering::Relaxed) > 0);
    }

    /// A key held through the monitor sounds until a note-off; one from the
    /// law thread, through the second ring, releases it too, as `jam` sends
    /// when a MIDI input goes away with keys held.
    #[test]
    fn a_note_off_from_the_law_thread_releases_a_held_voice() {
        let (_events_in, events_out) = RingBuffer::<Event>::new(1);
        let (mut heard, monitor) = RingBuffer::new(4);
        let (mut hush_in, hush) = RingBuffer::new(4);
        let (readings_in, _readings) = RingBuffer::new(64);
        let shared = Arc::new(Shared::default());
        shared.covered.store(u64::MAX, Ordering::Release);
        let mut callback = Callback::new(
            Synth::new(0),
            events_out,
            Some(monitor),
            readings_in,
            Arc::clone(&shared),
            false,
        )
        .with_hush(hush);
        heard
            .push(Monitor::On {
                pitch: 64,
                velocity: 90,
            })
            .unwrap();
        let mut block = vec![0.0f32; 480];
        callback.run(&mut block, 1, 0, None);
        assert!(block.iter().any(|s| *s != 0.0));
        callback.run(&mut block, 1, 0, None);
        assert!(block.iter().any(|s| *s != 0.0), "held");
        hush_in.push(Monitor::Off { pitch: 64 }).unwrap();
        for _ in 0..4 {
            callback.run(&mut block, 1, 0, None);
        }
        assert!(
            block.iter().all(|s| *s == 0.0),
            "released: its 30 ms release is over"
        );
    }

    /// A jam, as `jam` runs it, on a device with a 6,000-frame buffer: its
    /// first callback asks for the whole buffer, more than the ring covers, so
    /// the pre-roll renders it as silence; then it asks for 480 frames every
    /// 10 ms, and each is heard 146.8 ms later (the 5,520 frames still in the
    /// buffer, then 31.8 ms of output). After each callback the law thread
    /// takes the keys pressed so far, then steps the law on the heard clock,
    /// with the lookahead `jam` uses ([`Shared::lookahead`]). Keys pressed
    /// exactly when score notes are heard grade as matches: the one-off first
    /// fill must not hold the playhead ahead of the ear.
    #[test]
    fn a_first_fill_past_the_cover_does_not_hold_the_playhead_ahead() {
        use crate::anchor::Reading;
        use crate::live::{Clocks, Held, Press, Stamp, close_through, pass};

        const NS: u64 = 1_000_000_000;
        let latency: u64 = 5_520 + 1_526;
        let mut law = Law::acquire();
        law.load_score(&dense()).unwrap();
        let (mut events, events_out) = RingBuffer::new(crate::RING_EVENTS);
        let (readings_in, mut readings) = RingBuffer::new(4_096);
        let shared = Arc::new(Shared::default());
        let mut scheduler = Scheduler::new(0);
        let pumped = scheduler.pump(&mut law, 1, &mut events).unwrap();
        shared.covered.store(pumped.covered, Ordering::Release);
        let mut callback = Callback::new(
            Synth::new(0),
            events_out,
            None,
            readings_in,
            Arc::clone(&shared),
            true,
        );
        // Law sample `sample` is heard at this stream instant.
        let start = NS;
        let heard_at = |sample: u64| start + (sample + latency) * NS / 48_000;
        // Every tenth score note of the first second, pressed as it is heard,
        // released 4,000 samples later.
        let mut keys: Vec<(u64, bool, u8)> = Vec::new();
        for i in (10..100u64).step_by(10) {
            let pitch = 48 + (i % 36) as u8;
            keys.push((heard_at(i * 480), true, pitch));
            keys.push((heard_at(i * 480 + 4_000), false, pitch));
        }
        keys.sort();

        let mut clocks = Clocks::default();
        let mut held = Held::default();
        let mut log = Vec::new();
        let mut next = 0;
        // The first fill, before law time.
        let mut buffer = vec![0.0f32; 6_000];
        callback.run(&mut buffer, 1, start - NS / 100, None);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 6_000);
        let mut buffer = vec![0.0f32; 480];
        let mut now = start;
        while shared.frames.load(Ordering::Acquire) < 60_000 {
            let frame = shared.frames.load(Ordering::Acquire);
            callback.run(&mut buffer, 1, heard_at(frame), None);
            now += NS / 100;
            while let Ok(r) = readings.pop() {
                clocks.audio.push(Reading {
                    sample: r.sample,
                    nanos: r.nanos,
                });
            }
            while next < keys.len() && keys[next].0 <= now {
                let (instant, down, pitch) = keys[next];
                let press = Press {
                    down,
                    pitch,
                    velocity: 100,
                    stamp: Stamp::Stream { nanos: instant },
                };
                pass(&mut law, &clocks, &mut held, press, instant, &mut log);
                next += 1;
            }
            let frame = shared.frames.load(Ordering::Acquire);
            let target = steps_for(frame, shared.lookahead(), clocks.audio.sample_at(now));
            let pumped = scheduler.pump(&mut law, target, &mut events).unwrap();
            shared.covered.store(pumped.covered, Ordering::Release);
        }
        assert_eq!(next, keys.len());
        let stop = clocks.audio.sample_at(now).unwrap();
        close_through(&mut law, stop).unwrap();
        let record = law.record().unwrap();
        let played: Vec<&str> = record
            .rows
            .lines()
            .filter(|r| !r.ends_with("never played"))
            .collect();
        assert_eq!(played.len(), 9, "{played:#?}");
        for row in &played {
            assert!(row.ends_with(": match"), "{row}");
        }
        // Handed over well inside the allowance.
        let lags: Vec<i64> = log
            .iter()
            .filter(|p| p.down)
            .filter_map(|p| p.lag)
            .collect();
        assert!(lags.iter().all(|&lag| lag < 4_800), "{lags:?}");
        assert_eq!(shared.largest_buffer.load(Ordering::Relaxed), 480);
        assert_eq!(shared.first_buffer.load(Ordering::Relaxed), 6_000);
        assert_eq!(shared.late.load(Ordering::Relaxed), 0);
    }

    /// What a simulated jam leaves: the rows of the score notes a key cited,
    /// each note-on's lag, what the synth counted late, and the device's
    /// first callback.
    struct Jammed {
        rows: Vec<String>,
        lags: Vec<i64>,
        late: u64,
        first: u64,
    }

    /// A jam as `jam` runs it, on a device whose buffer is `buffer` frames.
    /// Its first callback fills the whole buffer, which the ring covers, so
    /// law time starts with it. Each later callback asks for `period` frames
    /// once that much has played, so its first frame is heard `buffer -
    /// period` frames, then 31.8 ms (1,526 samples) of output, after the
    /// callback. The law thread runs once a millisecond, as `jam`'s loop does:
    /// it moves the readings into the audio clock, passes the keys pressed so
    /// far, then steps the law on the heard clock with the lookahead `jam`
    /// uses. `keys` are (law sample, pitch): each key goes down when that
    /// sample is heard and comes up 4,000 samples later. The jam runs until law
    /// time reaches `end`, and the law's transport runs on through the stop.
    fn jam(buffer: u64, period: u64, keys: &[(u64, u8)], end: u64) -> Jammed {
        use crate::live::{Clocks, Held, Press, Stamp, close_through, pass};

        const NS: u64 = 1_000_000_000;
        const OUTPUT: u64 = 1_526;
        let mut law = Law::acquire();
        law.load_score(&dense()).unwrap();
        let (mut events, events_out) = RingBuffer::new(crate::RING_EVENTS);
        let (readings_in, mut readings) = RingBuffer::new(4_096);
        let shared = Arc::new(Shared::default());
        let mut scheduler = Scheduler::new(0);
        let pumped = scheduler.pump(&mut law, 1, &mut events).unwrap();
        shared.covered.store(pumped.covered, Ordering::Release);
        assert!(buffer <= pumped.covered, "the ring covers the first fill");
        let mut callback = Callback::new(
            Synth::new(0),
            events_out,
            None,
            readings_in,
            Arc::clone(&shared),
            true,
        );
        // Callback 0 fills the empty device at `start`; frame f is heard at
        // start + (f + OUTPUT) / 48 kHz.
        let start = NS;
        let heard_at = |sample: u64| start + (sample + OUTPUT) * NS / 48_000;
        let mut presses: Vec<(u64, bool, u8)> = keys
            .iter()
            .flat_map(|&(at, pitch)| {
                [
                    (heard_at(at), true, pitch),
                    (heard_at(at + 4_000), false, pitch),
                ]
            })
            .collect();
        presses.sort();

        let mut clocks = Clocks::default();
        let mut held = Held::default();
        let mut log = Vec::new();
        let mut next = 0;
        let mut calls = 0u64;
        let mut now = start;
        while shared.frames.load(Ordering::Acquire) < end {
            // Callback 0 at `start`, callback k once k periods have played.
            let due = start + calls * period * NS / 48_000;
            if now >= due {
                let frame = shared.frames.load(Ordering::Acquire);
                let size = if calls == 0 { buffer } else { period };
                let mut data = vec![0.0f32; size as usize];
                callback.run(&mut data, 1, heard_at(frame), None);
                calls += 1;
            }
            while let Ok(r) = readings.pop() {
                clocks.audio.push(r);
            }
            while next < presses.len() && presses[next].0 <= now {
                let (instant, down, pitch) = presses[next];
                let press = Press {
                    down,
                    pitch,
                    velocity: 100,
                    stamp: Stamp::Stream { nanos: instant },
                };
                pass(&mut law, &clocks, &mut held, press, instant, &mut log);
                next += 1;
            }
            let frame = shared.frames.load(Ordering::Acquire);
            let target = steps_for(frame, shared.lookahead(), clocks.audio.sample_at(now));
            let pumped = scheduler.pump(&mut law, target, &mut events).unwrap();
            shared.covered.store(pumped.covered, Ordering::Release);
            now += NS / 1_000;
        }
        assert_eq!(next, presses.len(), "every key was pressed and released");
        close_through(&mut law, clocks.audio.sample_at(now).unwrap()).unwrap();
        let rows = law
            .record()
            .unwrap()
            .rows
            .lines()
            .filter(|r| !r.ends_with("never played"))
            .map(String::from)
            .collect();
        let lags = log
            .iter()
            .filter(|p| p.down)
            .filter_map(|p| p.lag)
            .collect();
        Jammed {
            rows,
            lags,
            late: shared.late.load(Ordering::Relaxed),
            first: shared.first_buffer.load(Ordering::Relaxed),
        }
    }

    /// Issue #7's test. A device whose 3,000-frame first fill the ring covers,
    /// then 480-frame callbacks, each heard 84.3 ms later (2,520 frames still
    /// in the buffer, then 31.8 ms of output). Keys pressed 45 to 80 ms after
    /// their score notes are heard, up to the law's reach, grade late, each
    /// by exactly how late it was played: the one-off first fill does not hold
    /// the playhead ahead of the ear, so no score note closes before the key
    /// that answers it is handed over.
    #[test]
    fn a_covered_first_fill_does_not_hold_the_playhead_ahead() {
        // Every tenth score note from the second tenth of a second, played
        // 45, 50, ..., 80 ms late: 2,160 to 3,840 samples.
        let keys: Vec<(u64, u8)> = (1..=8u64)
            .map(|j| {
                let i = 10 * j;
                (i * 480 + (40 + 5 * j) * 48, 48 + (i % 36) as u8)
            })
            .collect();
        let jammed = jam(3_000, 480, &keys, 60_000);
        assert_eq!(jammed.first, 3_000);
        assert_eq!(jammed.late, 0);
        assert_eq!(jammed.rows.len(), 8, "{:#?}", jammed.rows);
        for (j, row) in (1..=8u64).zip(&jammed.rows) {
            let late = (40 + 5 * j) * 48;
            assert!(
                row.starts_with(&format!("note {}: onset +{late} samples", 10 * j))
                    && row.ends_with(": late"),
                "{row}\n{:#?}\nlags {:?}",
                jammed.rows,
                jammed.lags
            );
        }
        assert!(
            jammed.lags.iter().all(|&lag| lag < 4_800),
            "{:?}",
            jammed.lags
        );
    }

    /// A device whose first fill the ring covers and whose later callbacks are
    /// half of it: a 4,800-frame buffer asked for 2,400 frames at a time. The
    /// second callback reaches past what the ring covered at the start, so the
    /// law must have stepped far enough for it before it comes, and no event is
    /// late anywhere in the jam.
    #[test]
    fn callbacks_of_half_a_covered_fill_are_still_covered() {
        let jammed = jam(4_800, 2_400, &[], 60_000);
        assert_eq!(jammed.first, 4_800);
        assert_eq!(jammed.late, 0);
    }
}
