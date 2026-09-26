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
    pub largest_buffer: AtomicU64,
    pub notes: AtomicU64,
    pub beats: AtomicU64,
    pub late: AtomicU64,
    pub dropped: AtomicU64,
    pub monitored: AtomicU64,
}

/// Everything the audio callback owns: the synth, and its ends of the rings.
pub struct Callback {
    synth: Box<Synth>,
    events: Consumer<Event>,
    monitor: Option<Consumer<Monitor>>,
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
            readings,
            shared,
            mute,
            started: false,
            silent: 0,
            cap: PREROLL_CAP_FRAMES,
        }
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
        s.largest_buffer.fetch_max(frames, Ordering::Relaxed);

        if !self.started {
            let end = self.synth.frame().saturating_add(frames);
            if end <= s.covered.load(Ordering::Acquire) || self.silent >= self.cap {
                self.started = true;
                s.started.store(true, Ordering::Release);
            } else {
                data.fill(0.0);
                self.silent = self.silent.saturating_add(frames);
                s.preroll_frames.store(self.silent, Ordering::Relaxed);
                return;
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
}
