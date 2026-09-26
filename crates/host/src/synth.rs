//! The render: two oscillator voices and a click, from committed events.
//!
//! [`Synth::render`] is what the audio callback calls, and what the offline
//! render calls. It allocates nothing, takes no lock and makes no system call:
//! its voices live in fixed arrays, it pops events from an `rtrb` ring that was
//! created before the stream (so the ring allocated then), and it does only
//! arithmetic (`tests/alloc_free.rs` counts the allocations of many calls).
//!
//! # Where a note lands
//!
//! The synth renders frames `frame..frame + n` of the law's sample clock. An
//! event whose onset falls in that span starts at buffer index `onset - frame`,
//! exactly: every voice is silent before its onset and makes a non-zero sample
//! on it, because the first sample of the attack is `1 / 48` of the peak and
//! every waveform starts a quarter cycle in, at its crest. An event that
//! arrives after its onset has passed is late: it starts at index 0, as far
//! into its sound as it should be, and is counted.
//!
//! # The voices
//!
//! - **Score:** a sine, with a 1 ms attack and a slow decay, like a struck
//!   string.
//! - **Take, and live notes heard back:** a soft square (the 1st, 3rd and 5th
//!   harmonics below 20 kHz), with no decay, so a late take note is heard as a
//!   second, reedier voice against the score's.
//! - **Click:** a short bright burst, 2 kHz on a beat and 3 kHz, louder, on a
//!   downbeat.
//!
//! The waveform is picture: floats are used freely, and nothing here is hashed.

use core::f64::consts::TAU;

use rtrb::Consumer;

use crate::event::{Event, Monitor, Voice};

/// The law's sample rate, which the stream runs at.
pub const RATE: u32 = law::SAMPLE_RATE;
/// The most frames rendered in one pass. A longer device buffer is rendered
/// in several passes; events are taken per pass, so the split is inaudible.
pub const BLOCK: usize = 1_024;
/// Note voices, score and take together.
pub const NOTE_VOICES: usize = 64;
/// Click voices.
pub const CLICK_VOICES: usize = 8;

/// 1 ms: the attack rises from 1/48 of the peak on the onset sample to the
/// peak on the 48th.
const ATTACK: u64 = 48;
/// 30 ms: a note's release after its length.
const NOTE_RELEASE: u64 = 1_440;
/// 12 ms: a click's whole length, all of it a release.
const CLICK_RELEASE: u64 = 576;
/// Every waveform starts at its crest, a quarter cycle in.
const START_PHASE: f64 = 0.25;

/// Peak levels at full velocity. A chord of four score notes, four take notes
/// and a downbeat stays under full scale: *The Entertainer* renders with no
/// sample clipped.
pub const SCORE_PEAK: f32 = 0.11;
pub const TAKE_PEAK: f32 = 0.07;
pub const BEAT_PEAK: f32 = 0.16;
pub const DOWNBEAT_PEAK: f32 = 0.24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Timbre {
    Sine,
    SoftSquare,
    Click,
}

/// One sounding voice.
#[derive(Clone, Copy, Debug)]
struct Tone {
    active: bool,
    timbre: Timbre,
    /// Held open by the live monitor until a note-off for this pitch.
    held: Option<u8>,
    /// The fundamental, in cycles per sample.
    step: f64,
    /// The phase, in cycles.
    phase: f64,
    /// The soft square's odd harmonics below 20 kHz: 1, 2 or 3 of them.
    harmonics: u8,
    peak: f32,
    /// Multiplies the level every sample.
    decay: f32,
    level: f32,
    /// Samples since the onset.
    age: u64,
    /// Samples before the release begins.
    length: u64,
    release: u64,
    /// Where the tone starts in the pass being rendered.
    start: usize,
}

const SILENT: Tone = Tone {
    active: false,
    timbre: Timbre::Sine,
    held: None,
    step: 0.0,
    phase: 0.0,
    harmonics: 1,
    peak: 0.0,
    decay: 1.0,
    level: 1.0,
    age: 0,
    length: 0,
    release: 0,
    start: 0,
};

impl Tone {
    /// The tone's next sample, and one sample on.
    fn next(&mut self) -> f32 {
        let x = TAU * self.phase;
        let wave = match self.timbre {
            Timbre::Sine | Timbre::Click => x.sin(),
            Timbre::SoftSquare => {
                let mut w = x.sin();
                if self.harmonics >= 2 {
                    w += (3.0 * x).sin() / 3.0;
                }
                if self.harmonics >= 3 {
                    w += (5.0 * x).sin() / 5.0;
                }
                0.8 * w
            }
        };
        let attack = if self.age < ATTACK {
            // Both are below 49: exact in f32.
            (self.age.saturating_add(1)) as f32 / ATTACK as f32
        } else {
            1.0
        };
        let release = if self.age < self.length {
            1.0
        } else {
            let into = self.age.saturating_sub(self.length);
            self.release.saturating_sub(into) as f32 / self.release.max(1) as f32
        };
        let out = (wave as f32) * self.peak * self.level * attack * release;
        self.level *= self.decay;
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.age = self.age.saturating_add(1);
        if self.held.is_none() && self.age >= self.length.saturating_add(self.release) {
            self.active = false;
        }
        out
    }

    /// Adds the tone into `mix` from its start index; it may end partway.
    fn render(&mut self, mix: &mut [f32]) {
        let start = self.start;
        self.start = 0;
        for slot in mix.iter_mut().skip(start) {
            if !self.active {
                break;
            }
            *slot += self.next();
        }
    }

    /// Moves the tone `samples` into its sound without rendering them, for an
    /// event that arrived late.
    fn skip(&mut self, samples: u64) {
        self.age = samples;
        // The fractional part of samples × step, reduced first so the product
        // stays exact enough in f64 for any length a note has.
        let cycles = (samples as f64 * self.step).fract();
        self.phase = (START_PHASE + cycles).fract();
        let decayed = self.decay.powf(samples as f32);
        self.level = if decayed.is_finite() { decayed } else { 0.0 };
        if self.held.is_none() && samples >= self.length.saturating_add(self.release) {
            self.active = false;
        }
    }
}

/// What the synth has done since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Note events started, score and take.
    pub notes: u64,
    /// Beats started.
    pub beats: u64,
    /// Events that arrived after their onset had been rendered.
    pub late: u64,
    /// Events dropped because every voice of their kind was sounding.
    pub dropped: u64,
    /// Live notes started by the monitor.
    pub monitored: u64,
}

/// The two voices and the click, and the position they have rendered to.
pub struct Synth {
    notes: [Tone; NOTE_VOICES],
    clicks: [Tone; CLICK_VOICES],
    mix: [f32; BLOCK],
    /// The fundamental of every MIDI pitch, in cycles per sample.
    steps: [f64; 128],
    /// The law sample of the next frame to render.
    frame: u64,
    counts: Counts,
}

impl Synth {
    /// A synth that renders from law sample `frame` on. Everything it needs
    /// is allocated here, before any stream exists.
    pub fn new(frame: u64) -> Box<Synth> {
        let mut steps = [0.0f64; 128];
        for (pitch, step) in steps.iter_mut().enumerate() {
            let semitones = pitch as f64 - 69.0;
            *step = 440.0 * (semitones / 12.0).exp2() / f64::from(RATE);
        }
        Box::new(Synth {
            notes: [SILENT; NOTE_VOICES],
            clicks: [SILENT; CLICK_VOICES],
            mix: [0.0; BLOCK],
            steps,
            frame,
            counts: Counts::default(),
        })
    }

    /// The law sample of the next frame to render: the frames rendered so
    /// far, counted from sample 0.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn counts(&self) -> Counts {
        self.counts
    }

    /// Voices sounding now, notes and clicks.
    pub fn sounding(&self) -> usize {
        self.notes
            .iter()
            .chain(self.clicks.iter())
            .filter(|t| t.active)
            .count()
    }

    /// Renders `out`, interleaved with `channels` channels, and moves the
    /// position on by its frames. Events whose onset falls in the frames are
    /// popped from `events` and started on their sample; live notes waiting in
    /// `monitor` start on the first frame. Every channel carries the same mix,
    /// clipped to [-1, 1]. Trailing samples that do not fill a frame are left
    /// as they are.
    pub fn render(
        &mut self,
        out: &mut [f32],
        channels: usize,
        events: &mut Consumer<Event>,
        mut monitor: Option<&mut Consumer<Monitor>>,
    ) {
        if channels == 0 {
            return;
        }
        let frames = out.len() / channels;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(BLOCK);
            if let Some(m) = monitor.as_deref_mut() {
                while let Ok(heard) = m.pop() {
                    self.monitor(heard);
                }
            }
            self.pass(n, events);
            let written = out.iter_mut().skip(done * channels).take(n * channels);
            for (i, sample) in written.enumerate() {
                let value = self.mix.get(i / channels).copied().unwrap_or(0.0);
                *sample = value.clamp(-1.0, 1.0);
            }
            done += n;
        }
    }

    /// One pass of `n` frames, at most [`BLOCK`], into `mix`.
    fn pass(&mut self, n: usize, events: &mut Consumer<Event>) {
        let n = n.min(BLOCK);
        let end = self.frame.saturating_add(n as u64);
        loop {
            match events.peek() {
                Ok(event) if event.onset() < end => {}
                _ => break,
            }
            let Ok(event) = events.pop() else {
                break;
            };
            self.start(event);
        }
        let Some(mix) = self.mix.get_mut(..n) else {
            return;
        };
        mix.fill(0.0);
        for tone in self.notes.iter_mut().chain(self.clicks.iter_mut()) {
            if tone.active {
                tone.render(mix);
            }
        }
        self.frame = end;
    }

    fn start(&mut self, event: Event) {
        let onset = event.onset();
        let late = onset < self.frame;
        let offset = onset.saturating_sub(self.frame) as usize;
        let tone = match event {
            Event::Note {
                voice,
                pitch,
                velocity,
                duration,
                ..
            } => {
                let step = self.steps.get(usize::from(pitch)).copied().unwrap_or(0.0);
                let loudness = f32::from(velocity.min(127)) / 127.0;
                let tone = match voice {
                    Voice::Score => Tone {
                        timbre: Timbre::Sine,
                        peak: SCORE_PEAK * loudness,
                        // Halves every 0.8 s.
                        decay: 0.999_981_95,
                        ..note_tone(step, duration)
                    },
                    Voice::Take => Tone {
                        timbre: Timbre::SoftSquare,
                        harmonics: harmonics(step),
                        peak: TAKE_PEAK * loudness,
                        ..note_tone(step, duration)
                    },
                };
                (tone, self.notes.as_mut_slice(), &mut self.counts.notes)
            }
            Event::Beat { downbeat, .. } => {
                let hz = if downbeat { 3_000.0 } else { 2_000.0 };
                let tone = Tone {
                    active: true,
                    timbre: Timbre::Click,
                    step: hz / f64::from(RATE),
                    phase: START_PHASE,
                    peak: if downbeat { DOWNBEAT_PEAK } else { BEAT_PEAK },
                    // A time constant of 3 ms.
                    decay: 0.993_079,
                    length: 0,
                    release: CLICK_RELEASE,
                    start: offset,
                    ..SILENT
                };
                (tone, self.clicks.as_mut_slice(), &mut self.counts.beats)
            }
        };
        let (mut tone, pool, started) = tone;
        tone.start = offset;
        let Some(slot) = pool.iter_mut().find(|t| !t.active) else {
            self.counts.dropped = self.counts.dropped.saturating_add(1);
            return;
        };
        if late {
            tone.skip(self.frame.saturating_sub(onset));
            self.counts.late = self.counts.late.saturating_add(1);
        }
        *started = started.saturating_add(1);
        *slot = tone;
    }

    fn monitor(&mut self, heard: Monitor) {
        match heard {
            Monitor::On { pitch, velocity } => {
                let step = self.steps.get(usize::from(pitch)).copied().unwrap_or(0.0);
                let loudness = f32::from(velocity.min(127)) / 127.0;
                let tone = Tone {
                    timbre: Timbre::SoftSquare,
                    harmonics: harmonics(step),
                    held: Some(pitch),
                    peak: TAKE_PEAK * loudness,
                    ..note_tone(step, u64::MAX)
                };
                match self.notes.iter_mut().find(|t| !t.active) {
                    Some(slot) => {
                        *slot = tone;
                        self.counts.monitored = self.counts.monitored.saturating_add(1);
                    }
                    None => self.counts.dropped = self.counts.dropped.saturating_add(1),
                }
            }
            Monitor::Off { pitch } => {
                for tone in &mut self.notes {
                    if tone.active && tone.held == Some(pitch) {
                        tone.held = None;
                        tone.length = tone.age;
                    }
                }
            }
        }
    }
}

/// A note's tone before its timbre is set.
fn note_tone(step: f64, duration: u64) -> Tone {
    Tone {
        active: true,
        step,
        phase: START_PHASE,
        length: duration,
        release: NOTE_RELEASE,
        ..SILENT
    }
}

/// The soft square's odd harmonics (1st, 3rd, 5th) that stay below 20 kHz.
fn harmonics(step: f64) -> u8 {
    let hz = step * f64::from(RATE);
    1 + u8::from(3.0 * hz < 20_000.0) + u8::from(5.0 * hz < 20_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    fn score_note(onset: u64) -> Event {
        Event::Note {
            onset,
            voice: Voice::Score,
            pitch: 60,
            velocity: 100,
            duration: 2_000,
        }
    }

    fn take_note(onset: u64) -> Event {
        Event::Note {
            onset,
            voice: Voice::Take,
            pitch: 72,
            velocity: 100,
            duration: 2_000,
        }
    }

    fn beat(onset: u64) -> Event {
        Event::Beat {
            onset,
            downbeat: onset.is_multiple_of(2),
        }
    }

    /// Renders `frames` mono frames from `start`, `block` frames per call, with
    /// `events` in the ring.
    fn render(start: u64, frames: usize, block: usize, events: &[Event]) -> (Vec<f32>, Counts) {
        let (mut producer, mut consumer) = RingBuffer::new(events.len().max(1));
        for e in events {
            producer.push(*e).unwrap();
        }
        let mut synth = Synth::new(start);
        let mut out = vec![0.0f32; frames];
        for chunk in out.chunks_mut(block) {
            synth.render(chunk, 1, &mut consumer, None);
        }
        assert_eq!(synth.frame(), start + frames as u64);
        (out, synth.counts())
    }

    /// Every kind of event starts on its onset sample exactly, whatever the
    /// block size and wherever the onset falls against block and pass
    /// boundaries: silence before it, sound on it.
    #[test]
    fn every_voice_starts_on_its_onset_sample_exactly() {
        for make in [score_note, take_note, beat] {
            for block in [1, 7, 48, 480, 1_024, 1_500, 4_096] {
                for onset in [
                    0, 1, 46, 47, 48, 479, 480, 1_023, 1_024, 1_025, 2_047, 3_001,
                ] {
                    let (out, counts) = render(0, 5_000, block, &[make(onset)]);
                    let first = out.iter().position(|s| *s != 0.0);
                    assert_eq!(
                        first,
                        Some(onset as usize),
                        "{:?} at {onset}, blocks of {block}",
                        make(onset)
                    );
                    assert_eq!(counts.late, 0);
                }
            }
        }
    }

    /// The same, for a synth that starts partway through the piece.
    #[test]
    fn a_synth_started_partway_places_notes_on_the_same_samples() {
        let start = 1_000_000;
        for onset in [
            start,
            start + 1,
            start + 1_023,
            start + 1_024,
            start + 5_555,
        ] {
            let (out, _) = render(start, 8_000, 441, &[take_note(onset)]);
            let first = out.iter().position(|s| *s != 0.0).unwrap();
            assert_eq!(start + first as u64, onset);
        }
    }

    #[test]
    fn the_first_sample_is_the_attacks_first_step_at_the_crest() {
        let close = |a: f32, b: f32| (a - b).abs() <= 1e-6 * b.abs();
        let (out, _) = render(0, 10, 10, &[score_note(3)]);
        let peak = SCORE_PEAK * 100.0 / 127.0;
        assert!(close(out[3], peak / 48.0), "{}", out[3]);
        let (out, _) = render(0, 10, 10, &[take_note(3)]);
        // sin(pi/2) + sin(3 pi/2)/3 + sin(5 pi/2)/5, times 0.8.
        let crest = 0.8f32 * (1.0 - 1.0 / 3.0 + 1.0 / 5.0);
        assert!(
            close(out[3], crest * TAKE_PEAK * 100.0 / 127.0 / 48.0),
            "{}",
            out[3]
        );
        let (out, _) = render(0, 10, 10, &[beat(4)]);
        assert!(
            close(out[4], DOWNBEAT_PEAK / 48.0),
            "a downbeat: {}",
            out[4]
        );
    }

    /// A note sounds for its length and its release, and then stops.
    #[test]
    fn a_note_sounds_for_its_length_and_release() {
        let (out, _) = render(0, 6_000, 480, &[take_note(100)]);
        let last = out.iter().rposition(|s| *s != 0.0).unwrap();
        assert_eq!(last as u64, 100 + 2_000 + NOTE_RELEASE - 1);
        let (out, _) = render(0, 2_000, 480, &[beat(10)]);
        let last = out.iter().rposition(|s| *s != 0.0).unwrap();
        assert_eq!(last as u64, 10 + CLICK_RELEASE - 1);
    }

    /// An empty ring is no reason to wait: the synth renders silence and moves
    /// on.
    #[test]
    fn an_empty_ring_renders_silence() {
        let (out, counts) = render(500, 3_000, 256, &[]);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(counts, Counts::default());
    }

    /// An event that reaches the synth after its onset has been rendered is
    /// late: it starts on the next frame, as far into its sound as it should
    /// be, and is counted.
    #[test]
    fn a_late_event_is_counted_and_keeps_its_end() {
        let (mut producer, mut consumer) = RingBuffer::new(4);
        let mut synth = Synth::new(0);
        let mut out = vec![0.0f32; 1_000];
        synth.render(&mut out, 1, &mut consumer, None);
        producer.push(take_note(400)).unwrap();
        let mut rest = vec![0.0f32; 4_000];
        synth.render(&mut rest, 1, &mut consumer, None);
        assert_eq!(synth.counts().late, 1);
        assert_ne!(rest[0], 0.0, "starts on the first frame it can");
        let last = rest.iter().rposition(|s| *s != 0.0).unwrap() as u64 + 1_000;
        assert_eq!(last, 400 + 2_000 + NOTE_RELEASE - 1, "and ends on time");
    }

    #[test]
    fn with_every_voice_sounding_an_event_is_dropped_and_counted() {
        let events: Vec<Event> = (0..=NOTE_VOICES as u64).map(take_note).collect();
        let (_, counts) = render(0, 200, 100, &events);
        assert_eq!(counts.notes, NOTE_VOICES as u64);
        assert_eq!(counts.dropped, 1);
    }

    /// A live note sounds from the first frame after it arrives and releases
    /// on its note-off, without the law.
    #[test]
    fn a_monitored_note_sounds_at_once_and_stops_on_its_note_off() {
        let (_producer, mut events) = RingBuffer::<Event>::new(1);
        let (mut heard, mut monitor) = RingBuffer::new(4);
        let mut synth = Synth::new(0);
        let mut block = vec![0.0f32; 256];
        synth.render(&mut block, 1, &mut events, Some(&mut monitor));
        assert!(block.iter().all(|s| *s == 0.0));
        heard
            .push(Monitor::On {
                pitch: 64,
                velocity: 90,
            })
            .unwrap();
        synth.render(&mut block, 1, &mut events, Some(&mut monitor));
        assert_ne!(block[0], 0.0);
        assert_eq!(synth.counts().monitored, 1);
        for _ in 0..40 {
            synth.render(&mut block, 1, &mut events, Some(&mut monitor));
        }
        assert!(block.iter().any(|s| *s != 0.0), "held until its note-off");
        heard.push(Monitor::Off { pitch: 64 }).unwrap();
        let mut tail = vec![0.0f32; NOTE_RELEASE as usize + 10];
        synth.render(&mut tail, 1, &mut events, Some(&mut monitor));
        let release = NOTE_RELEASE as usize;
        assert_ne!(tail[0], 0.0);
        assert_ne!(tail[release - 1], 0.0, "the release's last sample");
        assert!(tail[release..].iter().all(|s| *s == 0.0));
        assert_eq!(synth.sounding(), 0);
    }

    #[test]
    fn every_channel_carries_the_same_mix() {
        let (mut producer, mut consumer) = RingBuffer::new(2);
        producer.push(score_note(5)).unwrap();
        producer.push(beat(9)).unwrap();
        let mut synth = Synth::new(0);
        let mut out = vec![0.0f32; 2 * 600 + 1];
        synth.render(&mut out, 2, &mut consumer, None);
        let (frames, _) = out.as_chunks::<2>();
        for [left, right] in frames {
            assert_eq!(left, right);
        }
        assert_eq!(out[2 * 600], 0.0, "a trailing sample is left alone");
        let first = out.iter().position(|s| *s != 0.0).unwrap();
        assert_eq!(first, 2 * 5);
    }
}
