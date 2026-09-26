//! The piano's voices: the samples of [`crate::piano`] played on their onset
//! sample, pitched by resampling, and released when their note ends.
//!
//! [`Piano::render`] runs inside the audio callback, through
//! [`crate::synth::Synth`]. It allocates nothing, takes no lock and makes no
//! system call: the samples were decoded into memory before the stream
//! existed, the voices live in one fixed array, and a voice reads its sample
//! through a shared reference (`alloc_free` counts the allocations of the
//! callback with the piano in it: none).
//!
//! # What a note does
//!
//! - **Its sample.** The key's zone at the layer its velocity picks
//!   ([`crate::piano::layer`]). Its first frame lands on the note's onset
//!   sample.
//! - **Its pitch.** A key a semitone either side of its zone's sampled key
//!   plays the sample 2^(±1/12) as fast, read between frames with a
//!   four-point cubic Hermite (Catmull-Rom) interpolation. The position is a
//!   32.32 fixed-point frame count, so it never drifts; on the sampled key the
//!   step is exactly one frame and the sample plays verbatim.
//! - **Its loudness.** The SFZ's `amp_veltrack=73`, as sfizz reads it: a gain
//!   of `1 - 0.73 × (1 - (velocity / 127)²)` on top of the layer.
//! - **Its end.** When its length is over, the string falls as the SFZ's
//!   `ampeg_release` says, sfizz's exponential release, 78 dB over 1 s for a
//!   damped key and over 5 s for the undamped top keys (F6 up, the SFZ's
//!   "notes without dampers"); below -80 dB the string stops. On that same
//!   sample the key's hammer-noise release starts, at the SFZ's `volume=-37`,
//!   `amp_veltrack=82` and `rt_decay=2` (2 dB quieter for every second the
//!   key was held).
//! - **No pedal.** The law carries no sustain pedal: a score writes sustain as
//!   note lengths, and a note sounds exactly as long as the law committed.
//!
//! # Bit for bit
//!
//! The arithmetic is IEEE addition, subtraction, multiplication and division
//! only, in a fixed order. Every constant that would need a power or an
//! exponential (the semitone steps, the release falls, the -37 dB) is a
//! literal here, computed once to 60 digits, and `rt_decay`'s power of the
//! note's length is taken by repeated squaring. No transcendental function
//! runs, so a render is the same bits on every machine that runs it.

use std::sync::Arc;

use crate::piano::{self, Bank};

/// Voices the piano sounds at once, with their releases.
pub const VOICES: usize = 96;

/// The piano's level in the mix: a fortissimo chord of the lowest octave
/// stays under full scale.
pub const MASTER: f32 = 0.3;

/// 2^(k/12) for k = -1, 0, +1 in 32.32 fixed point: a semitone down, the
/// sampled key, a semitone up, each the nearest integer to the exact value.
const STEPS: [u64; 3] = [4_053_909_305, 1 << 32, 4_550_359_342];
/// 2^-32, exact in f32.
const FRACTION: f32 = 1.0 / 4_294_967_296.0;

/// The SFZ's `amp_veltrack`: 73 % for the notes, 82 % for the hammer.
const NOTE_VELTRACK: f32 = 0.73;
const HAMMER_VELTRACK: f32 = 0.82;
/// The hammer releases' `volume=-37`: 10^(-37/20), the nearest f32.
const HAMMER_VOLUME: f32 = 0.014_125_375;
/// `rt_decay=2`, 2 dB for every second the key was held, per sample:
/// 10^(-2 / (20 × 48,000)), the nearest f64.
const RT_DECAY: f64 = 0.999_995_202_959_228_7;
/// The release's fall per sample, exp(-9 / (seconds × 48,000)): 78 dB over
/// `ampeg_release`, 1 s for a damped key, 5 s for an undamped one. The
/// nearest f32s.
const FALL_DAMPED: f32 = 0.999_812_54;
const FALL_UNDAMPED: f32 = 0.999_962_5;
/// The lowest key without a damper, F6: the SFZ's second note group starts
/// here.
const UNDAMPED: u8 = 89;
/// -80 dB: a released string quieter than this stops.
const FLOOR: f32 = 1.0e-4;

/// What became of a note the piano was asked to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Played,
    /// Every voice was sounding.
    Dropped,
    /// The key is not on the piano (A0 to C8).
    Outside,
    /// Its sample was not loaded: the piece's needs left it out.
    Missing,
}

/// One sounding key: its string and, once released, its hammer noise.
#[derive(Clone, Copy, Debug)]
struct Voice {
    active: bool,
    key: u8,
    zone: usize,
    layer: usize,
    /// Where the string is in its sample, in 32.32 fixed-point frames, and
    /// how far it moves each output sample.
    pos: u64,
    step: u64,
    /// The sample's unit, times the velocity's gain, times [`MASTER`].
    gain: f32,
    /// Samples since the onset, and the note's length in samples.
    age: u64,
    length: u64,
    /// The release: 1 until the note ends, then times `fall` every sample.
    level: f32,
    fall: f32,
    /// The string still sounds: its sample has frames left and its release
    /// is above the floor.
    string: bool,
    /// The hammer release's gain, and its frames played so far.
    hammer_gain: f32,
    hammer: usize,
    /// Where the voice starts in the pass being rendered.
    start: usize,
}

const SILENT: Voice = Voice {
    active: false,
    key: 0,
    zone: 0,
    layer: 0,
    pos: 0,
    step: 1 << 32,
    gain: 0.0,
    age: 0,
    length: 0,
    level: 1.0,
    fall: FALL_DAMPED,
    string: false,
    hammer_gain: 0.0,
    hammer: 0,
    start: 0,
};

/// A velocity's gain under an `amp_veltrack` of `track`, as sfizz computes
/// it: `1 - track × (1 - v²)`, with `v` the velocity over 127.
fn velocity_gain(velocity: u8, track: f32) -> f32 {
    let v = f32::from(velocity.min(127)) / 127.0;
    1.0 - track * (1.0 - v * v)
}

/// `base` to the power `exp`, by repeated squaring: the same multiplications
/// in the same order on every machine.
fn power_f64(base: f64, mut exp: u64) -> f64 {
    let (mut result, mut square) = (1.0f64, base);
    while exp > 0 {
        if exp & 1 == 1 {
            result *= square;
        }
        square *= square;
        exp >>= 1;
    }
    result
}

fn power_f32(base: f32, mut exp: u64) -> f32 {
    let (mut result, mut square) = (1.0f32, base);
    while exp > 0 {
        if exp & 1 == 1 {
            result *= square;
        }
        square *= square;
        exp >>= 1;
    }
    result
}

/// The four-point, third-order Hermite (Catmull-Rom) value at `t` between
/// `x0` and `x1`: exactly `x0` at `t = 0`.
fn hermite(xm1: f32, x0: f32, x1: f32, x2: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

impl Voice {
    /// Moves the voice `late` samples into its sound without rendering them,
    /// for a note that reached the piano after its onset had passed.
    fn skip(&mut self, late: u64, frames: usize) {
        self.age = late;
        let played = late.min(u64::try_from(frames).unwrap_or(u64::MAX).saturating_add(1));
        self.pos = played.saturating_mul(self.step);
        if late >= self.length {
            let released = late - self.length;
            self.level = power_f32(self.fall, released);
            self.hammer = usize::try_from(released).unwrap_or(usize::MAX);
            if self.level < FLOOR {
                self.string = false;
            }
        }
    }

    /// Adds the voice into `left` and `right` from its start index; it may end
    /// partway.
    fn render(&mut self, bank: &Bank, left: &mut [f32], right: &mut [f32]) {
        let start = std::mem::take(&mut self.start);
        let pcm: &[i16] = bank.note(self.zone, self.layer).map_or(&[], |s| &s.pcm[..]);
        let frames = pcm.len() / 2;
        let hammer: &[i16] = bank.release(self.key).map_or(&[], |s| &s.pcm[..]);
        let hammer_frames = hammer.len() / 2;
        let at = |index: usize, back: bool, ahead: usize, channel: usize| -> f32 {
            let frame = if back {
                index.checked_sub(1)
            } else {
                index.checked_add(ahead)
            };
            frame
                .filter(|&f| f < frames)
                .and_then(|f| pcm.get(f * 2 + channel))
                .map_or(0.0, |&v| f32::from(v))
        };
        for (l_out, r_out) in left.iter_mut().zip(right.iter_mut()).skip(start) {
            if !self.active {
                break;
            }
            let (mut l, mut r) = (0.0f32, 0.0f32);
            if self.string {
                let index = usize::try_from(self.pos >> 32).unwrap_or(usize::MAX);
                if index >= frames {
                    self.string = false;
                } else {
                    let t = (self.pos & 0xFFFF_FFFF) as f32 * FRACTION;
                    let g = self.gain * self.level;
                    l = hermite(
                        at(index, true, 0, 0),
                        at(index, false, 0, 0),
                        at(index, false, 1, 0),
                        at(index, false, 2, 0),
                        t,
                    ) * g;
                    r = hermite(
                        at(index, true, 0, 1),
                        at(index, false, 0, 1),
                        at(index, false, 1, 1),
                        at(index, false, 2, 1),
                        t,
                    ) * g;
                    self.pos = self.pos.saturating_add(self.step);
                }
            }
            if self.age >= self.length {
                if self.string {
                    self.level *= self.fall;
                    if self.level < FLOOR {
                        self.string = false;
                    }
                }
                if self.hammer < hammer_frames {
                    let h = self.hammer * 2;
                    l += hammer.get(h).map_or(0.0, |&v| f32::from(v)) * self.hammer_gain;
                    r += hammer.get(h + 1).map_or(0.0, |&v| f32::from(v)) * self.hammer_gain;
                    self.hammer += 1;
                }
            }
            *l_out += l;
            *r_out += r;
            self.age = self.age.saturating_add(1);
            if !self.string && self.age > self.length && self.hammer >= hammer_frames {
                self.active = false;
            }
        }
    }
}

/// The piano: its samples and its voices.
pub struct Piano {
    bank: Arc<Bank>,
    voices: [Voice; VOICES],
}

impl Piano {
    /// A silent piano over `bank`. Everything it needs is allocated here,
    /// before any stream exists.
    pub fn new(bank: Arc<Bank>) -> Box<Piano> {
        Box::new(Piano {
            bank,
            voices: [SILENT; VOICES],
        })
    }

    /// Starts a note of `key` at `velocity` that lasts `length` samples, at
    /// index `offset` of the pass being rendered, `late` samples into its
    /// sound (0 when it is on time).
    pub fn start(&mut self, offset: usize, late: u64, key: u8, velocity: u8, length: u64) -> Start {
        let Some((zone, semitones)) = piano::zone(key) else {
            return Start::Outside;
        };
        let layer = piano::layer(velocity);
        let Some(sample) = self.bank.note(zone, layer) else {
            return Start::Missing;
        };
        let Some(slot) = self.voices.iter_mut().find(|v| !v.active) else {
            return Start::Dropped;
        };
        let step = match semitones {
            -1 => STEPS[0],
            1 => STEPS[2],
            _ => STEPS[1],
        };
        let hammer_gain = self.bank.release(key).map_or(0.0, |s| {
            let held = power_f64(RT_DECAY, length) as f32;
            s.unit * HAMMER_VOLUME * velocity_gain(velocity, HAMMER_VELTRACK) * held * MASTER
        });
        *slot = Voice {
            active: true,
            key,
            zone,
            layer,
            pos: 0,
            step,
            gain: sample.unit * velocity_gain(velocity, NOTE_VELTRACK) * MASTER,
            age: 0,
            length,
            level: 1.0,
            fall: if key >= UNDAMPED {
                FALL_UNDAMPED
            } else {
                FALL_DAMPED
            },
            string: true,
            hammer_gain,
            hammer: 0,
            start: offset,
        };
        if late > 0 {
            slot.skip(late, sample.frames());
        }
        Start::Played
    }

    /// Adds every sounding voice into `left` and `right`.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let bank = &*self.bank;
        for voice in self.voices.iter_mut().filter(|v| v.active) {
            voice.render(bank, left, right);
        }
    }

    /// Voices sounding now.
    pub fn sounding(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::piano::{File, Needs};

    /// A bank of the samples `notes` need, each note sample `frames` long and
    /// each hammer release 96 frames.
    fn bank(notes: &[(u8, u8)], frames: usize) -> Arc<Bank> {
        let mut needs = Needs::default();
        for &(key, velocity) in notes {
            needs.note(key, velocity);
        }
        Arc::new(fixture::bank(&needs, |file| match file {
            File::Note { .. } => frames,
            File::Release { .. } => 96,
        }))
    }

    /// Renders `frames` frames of the notes (onset, key, velocity, length),
    /// in passes of `block`.
    fn render(
        bank: &Arc<Bank>,
        notes: &[(u64, u8, u8, u64)],
        frames: usize,
        block: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut piano = Piano::new(Arc::clone(bank));
        let (mut left, mut right) = (vec![0.0f32; frames], vec![0.0f32; frames]);
        let mut done = 0usize;
        while done < frames {
            let n = block.min(frames - done);
            let end = (done + n) as u64;
            for &(onset, key, velocity, length) in notes {
                if (done as u64..end).contains(&onset) {
                    let offset = usize::try_from(onset).unwrap() - done;
                    assert_eq!(piano.start(offset, 0, key, velocity, length), Start::Played);
                }
            }
            piano.render(&mut left[done..done + n], &mut right[done..done + n]);
            done += n;
        }
        (left, right)
    }

    /// The gain a note of `key` at `velocity` plays its sample with.
    fn gain(bank: &Bank, key: u8, velocity: u8) -> f32 {
        let (zone, _) = piano::zone(key).unwrap();
        let unit = bank.note(zone, piano::layer(velocity)).unwrap().unit;
        unit * velocity_gain(velocity, NOTE_VELTRACK) * MASTER
    }

    /// On its sampled key a note plays its sample verbatim from its onset
    /// sample, whatever the passes: silence before the onset, then every
    /// frame of the sample times the note's gain, exactly.
    #[test]
    fn on_its_sampled_key_a_note_plays_its_sample_from_its_onset_exactly() {
        let bank = bank(&[(60, 100)], 2_000);
        let sample = bank.note(13, piano::layer(100)).unwrap();
        let g = gain(&bank, 60, 100);
        for block in [1, 7, 480, 1_024] {
            for onset in [0u64, 1, 479, 480, 1_023, 1_500] {
                let (left, right) = render(&bank, &[(onset, 60, 100, 100_000)], 4_000, block);
                let first = left.iter().position(|s| *s != 0.0);
                assert_eq!(
                    first,
                    Some(onset as usize),
                    "onset {onset}, blocks of {block}"
                );
                for i in 0..2_000 {
                    let at = onset as usize + i;
                    assert_eq!(left[at], f32::from(sample.pcm[2 * i]) * g, "{i}");
                    assert_eq!(right[at], f32::from(sample.pcm[2 * i + 1]) * g, "{i}");
                }
                assert!(left[onset as usize + 2_000..].iter().all(|s| *s == 0.0));
            }
        }
    }

    /// Positive-going zero crossings of `x` after `from`, as the mean period
    /// between the first and the last of them.
    fn period(x: &[f32], from: usize) -> f64 {
        let ups: Vec<usize> = (from + 1..x.len())
            .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
            .collect();
        let (first, last) = (ups[0], *ups.last().unwrap());
        (last - first) as f64 / (ups.len() - 1) as f64
    }

    /// A key a semitone above or below its zone's sampled key plays the
    /// sample 2^(±1/12) as fast: C4's zone has a 66-sample period, which
    /// becomes 62.30 samples on C#4 and 69.93 on B3.
    #[test]
    fn a_semitone_either_side_resamples_by_a_twelfth_root_of_two() {
        let bank = bank(&[(59, 64), (60, 64), (61, 64)], 30_000);
        let sampled = f64::from(40 + 2 * 13);
        for (key, ratio) in [(59, -1.0f64), (60, 0.0), (61, 1.0)] {
            let (left, _) = render(&bank, &[(0, key, 64, 60_000)], 28_000, 480);
            let want = sampled / 2f64.powf(ratio / 12.0);
            let got = period(&left, 100);
            assert!(
                (got - want).abs() < 0.01,
                "key {key}: period {got}, want {want}"
            );
        }
    }

    /// Each velocity plays its layer: the sample's code in the first frame
    /// names the zone and the layer the SFZ gives that velocity.
    #[test]
    fn each_velocity_plays_its_layer() {
        let velocities = [
            1u8, 26, 27, 35, 37, 44, 47, 51, 57, 65, 73, 81, 89, 97, 105, 113, 121, 127,
        ];
        let notes: Vec<(u8, u8)> = velocities.iter().map(|&v| (72, v)).collect();
        let bank = bank(&notes, 64);
        for v in velocities {
            let (left, right) = render(&bank, &[(0, 72, v, 1_000)], 64, 64);
            let file = File::Note {
                zone: 17,
                layer: piano::layer(v),
            };
            assert_eq!(
                fixture::code_of(left[0], right[0]),
                fixture::code(file),
                "velocity {v}"
            );
        }
    }

    /// A note plays for its length, then its string falls by the release's
    /// fall every sample, and its hammer release starts on the note's last
    /// sample plus one, from its first frame.
    #[test]
    fn a_note_releases_on_its_length_with_its_hammer_noise() {
        let bank = bank(&[(60, 90)], 60_000);
        let g = gain(&bank, 60, 90);
        let (onset, length) = (100u64, 3_000u64);
        let (left, right) = render(&bank, &[(onset, 60, 90, length)], 60_000, 480);
        let sample = bank.note(13, piano::layer(90)).unwrap();
        let hammer = bank.release(60).unwrap();
        let hammer_gain = hammer.unit
            * HAMMER_VOLUME
            * velocity_gain(90, HAMMER_VELTRACK)
            * power_f64(RT_DECAY, length) as f32
            * MASTER;
        let off = (onset + length) as usize;
        // Before the release: the sample at full level.
        assert_eq!(
            left[off - 1],
            f32::from(sample.pcm[2 * (off - 1 - onset as usize)]) * g
        );
        // On the release's first sample: the string still at full level, and
        // the hammer's first frame.
        let i = off - onset as usize;
        let want = f32::from(sample.pcm[2 * i]) * g + f32::from(hammer.pcm[0]) * hammer_gain;
        assert_eq!(left[off], want);
        let want = f32::from(sample.pcm[2 * i + 1]) * g + f32::from(hammer.pcm[1]) * hammer_gain;
        assert_eq!(right[off], want);
        // The hammer's code, heard on its own: the string's part taken out.
        let l = left[off] - f32::from(sample.pcm[2 * i]) * g;
        let r = right[off] - f32::from(sample.pcm[2 * i + 1]) * g;
        assert_eq!(
            fixture::code_of(l, r),
            fixture::code(File::Release { key: 60 })
        );
        // One damped second in, the string is 78 dB below where it began:
        // its peak over a period against its peak over the release's first.
        let peak = |from: usize| {
            left[from..from + 66]
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()))
        };
        let fallen = f64::from(peak(off + 48_000)) / f64::from(peak(off + 200));
        let want = (-9.0f64).exp();
        assert!((fallen / want - 1.0).abs() < 0.1, "{fallen} vs {want}");
        // It stops where its level falls below the floor, ln(1e-4) / ln(fall)
        // samples into the release, to within the rounding of the f32 level.
        let last = left.iter().rposition(|s| *s != 0.0).unwrap();
        let floor = (1e-4f64.ln() / f64::from(FALL_DAMPED).ln()).ceil() as usize;
        assert!(
            last.abs_diff(off + floor - 1) <= 20,
            "{last} vs {}",
            off + floor - 1
        );
    }

    /// The top keys have no dampers: F6 and up release over 5 s, not 1 s.
    #[test]
    fn keys_without_dampers_release_slowly() {
        let bank = bank(&[(88, 90), (89, 90)], 70_000);
        for (key, fall) in [(88u8, FALL_DAMPED), (89, FALL_UNDAMPED)] {
            let mut piano = Piano::new(Arc::clone(&bank));
            piano.start(0, 0, key, 90, 10);
            let (mut l, mut r) = (vec![0.0f32; 60_000], vec![0.0f32; 60_000]);
            piano.render(&mut l, &mut r);
            assert_eq!(piano.voices[0].fall, fall, "key {key}");
            let sounding = piano.sounding();
            assert_eq!(sounding, usize::from(key == 89), "key {key} after 1.25 s");
        }
    }

    /// A note that reaches the piano late starts as far into its sound as it
    /// should be: from then on it is the same as if it had been on time, to
    /// the bit while it is held, and to the rounding of its release's power
    /// (taken by squaring, not sample by sample) once it has ended.
    #[test]
    fn a_late_note_keeps_its_place() {
        let bank = bank(&[(61, 70)], 10_000);
        let (on_time, _) = render(&bank, &[(0, 61, 70, 2_000)], 8_000, 8_000);
        for late in [1u64, 999, 1_999, 2_000, 2_001, 2_500] {
            let mut piano = Piano::new(Arc::clone(&bank));
            assert_eq!(piano.start(0, late, 61, 70, 2_000), Start::Played);
            let n = 8_000 - late as usize;
            let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
            piano.render(&mut l, &mut r);
            for i in 0..n.min(500) {
                let want = on_time[late as usize + i];
                if late + (i as u64) < 2_000 {
                    assert_eq!(l[i], want, "late {late}, sample {i}");
                } else {
                    let close = (l[i] - want).abs() <= 1e-4 * want.abs().max(1e-6);
                    assert!(close, "late {late}, sample {i}: {} vs {want}", l[i]);
                }
            }
        }
    }

    /// Keys off the keyboard play nothing, and a piano with every voice
    /// sounding drops a note and says so.
    #[test]
    fn a_note_the_piano_cannot_play_says_why() {
        let bank = bank(&[(60, 64)], 1_000);
        let mut piano = Piano::new(bank);
        assert_eq!(piano.start(0, 0, 20, 64, 100), Start::Outside);
        assert_eq!(piano.start(0, 0, 109, 64, 100), Start::Outside);
        assert_eq!(piano.start(0, 0, 62, 64, 100), Start::Missing);
        for _ in 0..VOICES {
            assert_eq!(piano.start(0, 0, 60, 64, 100), Start::Played);
        }
        assert_eq!(piano.start(0, 0, 60, 64, 100), Start::Dropped);
        assert_eq!(piano.sounding(), VOICES);
    }

    /// The constants are what their documentation says, to the nearest f32
    /// or f64 (checked here with the transcendental functions the render
    /// itself never calls).
    #[test]
    fn the_constants_are_their_formulas() {
        let two32 = 4_294_967_296.0f64;
        assert_eq!(STEPS[0], (two32 * 2f64.powf(-1.0 / 12.0)).round() as u64);
        assert_eq!(STEPS[2], (two32 * 2f64.powf(1.0 / 12.0)).round() as u64);
        assert_eq!(FALL_DAMPED, (-9.0f64 / 48_000.0).exp() as f32);
        assert_eq!(FALL_UNDAMPED, (-9.0f64 / 240_000.0).exp() as f32);
        assert_eq!(HAMMER_VOLUME, 10f64.powf(-37.0 / 20.0) as f32);
        assert!((RT_DECAY - 10f64.powf(-2.0 / (20.0 * 48_000.0))).abs() < 1e-16);
        assert_eq!(power_f64(3.0, 5), 243.0);
        assert_eq!(power_f32(0.5, 10), 1.0 / 1_024.0);
    }

    /// Hermite interpolation passes through its points and is exact at 0.
    #[test]
    fn the_interpolation_passes_through_its_points() {
        assert_eq!(hermite(1.0, 2.0, 3.0, 4.0, 0.0), 2.0);
        assert_eq!(hermite(1.0, 2.0, 3.0, 4.0, 1.0), 3.0);
        assert_eq!(hermite(1.0, 2.0, 3.0, 4.0, 0.5), 2.5, "a line stays a line");
    }
}
