//! Live input, from a press to the law's live verbs.
//!
//! A press is a key or MIDI note going down or up, stamped the moment the host
//! receives it: a key with the stream clock, a MIDI message with its WinMM
//! time and its arrival on the stream clock. The monitor has already started
//! or released a voice for it by then; this side passes it to the law, through
//! the clocks in [`crate::anchor`], at the law sample at which it was heard: a
//! key going down as a note-on, a key coming up as a note-off. The law decides
//! what the note cites and grades it, each score note at most once, so the
//! presses go to it in the order they were received. The host decides
//! nothing.
//!
//! # The delivery allowance
//!
//! The law closes a score note once its playhead has passed the note's onset
//! by the reach and then by the delivery allowance, H (4,800 samples,
//! 100 ms): a note handed over later than its onset plus the allowance can no
//! longer be graded against a note that has closed. [`Passed::lag`] records,
//! for every note handed over, how far the playhead was past the note's onset,
//! so `jam` can show the allowance was kept. In a jam the playhead follows the
//! audio clock ([`crate::schedule::steps_for`]), so the lag is the time from
//! the key to the law: the input's own path, and the law thread's loop, which
//! takes presses before it steps the law.

use std::sync::atomic::{AtomicU64, Ordering};

use law::{LIVE_ALLOWANCE_SAMPLES, QUANTUM_SAMPLES};
use rtrb::{Consumer, Producer};

use crate::anchor::{AudioClock, MidiClock, Reading};
use crate::bridge::{Law, Refused};
use crate::event::Monitor;

/// Messages an input could not hand on because a ring was full: presses for
/// the law thread, and monitor messages for the audio callback. A full ring
/// drops the message rather than wait, and these count it.
#[derive(Default)]
pub struct Dropped {
    pub presses: AtomicU64,
    pub monitor: AtomicU64,
}

impl Dropped {
    /// The two counts, presses first.
    pub fn counts(&self) -> (u64, u64) {
        (
            self.presses.load(Ordering::Relaxed),
            self.monitor.load(Ordering::Relaxed),
        )
    }
}

/// The keys held down, as the presses the law thread takes show them, whether
/// or not the law took each one: what the monitor is sounding.
pub struct Keys {
    down: [bool; 128],
}

impl Default for Keys {
    fn default() -> Self {
        Keys { down: [false; 128] }
    }
}

impl Keys {
    /// A key went down or came up.
    pub fn press(&mut self, press: &Press) {
        if let Some(slot) = self.down.get_mut(usize::from(press.pitch)) {
            *slot = press.down;
        }
    }

    /// Takes every key held down, lowest first, and forgets them.
    pub fn release_all(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for (pitch, slot) in (0u8..).zip(self.down.iter_mut()) {
            if std::mem::take(slot) {
                out.push(pitch);
            }
        }
        out
    }
}

/// When a press was received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stamp {
    /// A key, stamped on the stream clock when it arrived.
    Stream { nanos: u64 },
    /// A MIDI message: its WinMM time in microseconds, and its arrival on the
    /// stream clock.
    Midi { micros: u64, arrived: u64 },
}

/// A key or a MIDI note going down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub down: bool,
    pub pitch: u8,
    pub velocity: u8,
    pub stamp: Stamp,
}

/// A MIDI short message as WinMM hands it over (status in the low byte, then
/// two data bytes), read as `(down, pitch, velocity)`: a note-on with a
/// velocity above 0 goes down; a note-off, or a note-on with velocity 0, goes
/// up. Every channel counts; any other message is `None`.
pub fn midi_press(message: u32) -> Option<(bool, u8, u8)> {
    let [status, pitch, velocity, _] = message.to_le_bytes();
    let (pitch, velocity) = (pitch & 0x7F, velocity & 0x7F);
    match status & 0xF0 {
        0x90 if velocity > 0 => Some((true, pitch, velocity)),
        0x90 | 0x80 => Some((false, pitch, velocity)),
        _ => None,
    }
}

/// The audio and MIDI clocks, which a press's stamp goes through.
#[derive(Default)]
pub struct Clocks {
    pub audio: AudioClock,
    pub midi: MidiClock,
}

impl Clocks {
    /// The stream instant of a press. A MIDI press's arrival first tightens
    /// the MIDI clock.
    pub fn instant(&mut self, stamp: Stamp) -> Option<u64> {
        match stamp {
            Stamp::Stream { nanos } => Some(nanos),
            Stamp::Midi { micros, arrived } => {
                self.midi.arrival(micros, arrived);
                self.midi.nanos_at(micros)
            }
        }
    }
}

/// One note-on or note-off the host passed to the law, and what the law said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passed {
    /// A note-on when true; a note-off when false.
    pub down: bool,
    pub pitch: u8,
    /// The note-on's velocity; 0 for a note-off.
    pub velocity: u8,
    /// The law sample the press was heard at. `None` when the audio clock had
    /// no reading yet, before law time started: then nothing was passed.
    pub sample: Option<i64>,
    /// How far the law's playhead was past `sample` when the press was
    /// handed over, in samples; negative when it had not reached it. The
    /// delivery allowance bounds it ([`LIVE_ALLOWANCE_SAMPLES`]). `None` when
    /// nothing was passed.
    pub lag: Option<i64>,
    /// The law's refusal. `None` when the law took it, or when nothing was
    /// passed.
    pub refused: Option<Refused>,
}

/// How far the law's playhead, the first sample of its playhead quantum, is
/// past `sample`, in samples, or `None` while its transport is stopped.
fn lag(law: &Law, sample: i64) -> Option<i64> {
    let playhead = law
        .steps()
        .checked_sub(1)?
        .checked_mul(u64::from(QUANTUM_SAMPLES))?;
    i64::try_from(playhead).ok()?.checked_sub(sample)
}

/// The pitches whose live note the law holds: a note-on it admitted, and no
/// note-off it took since.
pub struct Held {
    pitches: [bool; 128],
}

impl Default for Held {
    fn default() -> Self {
        Held {
            pitches: [false; 128],
        }
    }
}

impl Held {
    /// Whether the law holds a live note of `pitch`.
    pub fn holds(&self, pitch: u8) -> bool {
        self.pitches
            .get(usize::from(pitch))
            .copied()
            .unwrap_or(false)
    }

    fn set(&mut self, pitch: u8, held: bool) {
        if let Some(slot) = self.pitches.get_mut(usize::from(pitch)) {
            *slot = held;
        }
    }
}

/// Passes one press to the law at the law sample it was heard at, `instant`
/// on the stream clock, and logs what was passed.
///
/// - A key going down is a note-on. If the law still holds a note of that
///   pitch (a second note-on with no note-off between, which some keyboards
///   send), a note-off at the same sample ends it first, so the law holds at
///   most one live note of a pitch and a note-off ends the one the host means.
/// - A key coming up is a note-off, if the law holds a note of that pitch.
///   Otherwise there is nothing to end (its note-on was refused, or came
///   before law time), and nothing is passed.
///
/// Presses are passed as measured: the law refuses what it refuses with its
/// own code (a note-off on its note-on's sample, for one), and the refusal is
/// logged. A refused note-off leaves the note held; the next note-on of the
/// pitch, or the end of the jam, ends it.
pub fn pass(
    law: &mut Law,
    clocks: &Clocks,
    held: &mut Held,
    press: Press,
    instant: u64,
    log: &mut Vec<Passed>,
) {
    let sample = clocks.audio.sample_at(instant);
    if press.down {
        if held.holds(press.pitch) {
            note_off(law, held, press.pitch, sample, log);
        }
        let lag = sample.and_then(|at| lag(law, at));
        let refused = sample.and_then(|at| {
            law.live_note(at, u32::from(press.pitch), u32::from(press.velocity))
                .err()
        });
        if sample.is_some() && refused.is_none() {
            held.set(press.pitch, true);
        }
        log.push(Passed {
            down: true,
            pitch: press.pitch,
            velocity: press.velocity,
            sample,
            lag,
            refused,
        });
    } else if held.holds(press.pitch) {
        note_off(law, held, press.pitch, sample, log);
    }
}

fn note_off(law: &mut Law, held: &mut Held, pitch: u8, sample: Option<i64>, log: &mut Vec<Passed>) {
    let lag = sample.and_then(|at| lag(law, at));
    let refused = sample.and_then(|at| law.live_note_off(u32::from(pitch), at).err());
    if sample.is_some() && refused.is_none() {
        held.set(pitch, false);
    }
    log.push(Passed {
        down: false,
        pitch,
        velocity: 0,
        sample,
        lag,
        refused,
    });
}

/// How the note-ons in `log` kept the delivery allowance: the median and the
/// largest lag, in samples, and how many were past the allowance. `None` when
/// no note-on was handed over.
pub fn delivery(log: &[Passed]) -> Option<(i64, i64, usize)> {
    let mut lags: Vec<i64> = log
        .iter()
        .filter(|p| p.down)
        .filter_map(|p| p.lag)
        .collect();
    lags.sort_unstable();
    let median = *lags.get(lags.len() / 2)?;
    let largest = *lags.last()?;
    let allowance = i64::from(LIVE_ALLOWANCE_SAMPLES);
    let past = lags.iter().filter(|&&l| l > allowance).count();
    Some((median, largest, past))
}

/// Runs the law's transport on after a jam until every score note with an
/// onset before `stop`, the law sample heard when the jam stopped, has closed,
/// and one step past the last note handed over: then every row of the jam is
/// final and shown. The playhead stops on the first quantum boundary at or past
/// `stop` plus [`law::CLOSE_SAMPLES`], so a score note less than a quantum
/// after the stop can close too, and none later. The audio has stopped; the
/// law's step clock runs alone.
pub fn close_through(law: &mut Law, stop: i64) -> Result<(), Refused> {
    let quantum = u64::from(QUANTUM_SAMPLES);
    let stop = u64::try_from(stop).unwrap_or(0);
    // Closed once the playhead, (steps - 1) * Q, is past onset + CLOSE for
    // every onset up to stop - 1: once it is at or past stop + CLOSE.
    let close = stop.saturating_add(u64::from(law::CLOSE_SAMPLES));
    let target = close
        .div_ceil(quantum)
        .saturating_add(1)
        .max(law.steps().saturating_add(1));
    while law.steps() < target {
        law.step()?;
    }
    Ok(())
}

/// The law thread's side of a live take: the clocks a press goes through, the
/// keys the monitor is sounding, the notes the law holds, and every press
/// passed so far.
#[derive(Default)]
pub struct Taker {
    pub clocks: Clocks,
    pub held: Held,
    pub keys: Keys,
    pub passed: Vec<Passed>,
}

impl Taker {
    /// Moves the audio clock's readings in, then passes every press queued
    /// so far to the law, in the order it was received.
    pub fn take(
        &mut self,
        law: &mut Law,
        readings: &mut Consumer<Reading>,
        presses: &mut Consumer<Press>,
    ) {
        while let Ok(reading) = readings.pop() {
            self.clocks.audio.push(reading);
        }
        while let Ok(press) = presses.pop() {
            self.keys.press(&press);
            let Some(instant) = self.clocks.instant(press.stamp) else {
                continue;
            };
            pass(
                law,
                &self.clocks,
                &mut self.held,
                press,
                instant,
                &mut self.passed,
            );
        }
    }

    /// One pass of a jam's loop over its input: [`Taker::take`], then, when
    /// `gone` says the input may have gone away, a monitor note-off through
    /// `hush` for every key still held, since such a key gets no note-off of
    /// its own.
    ///
    /// The presses are taken first. A key that went down just before the
    /// input went away may still be in the ring; the monitor already sounds
    /// it, and only once it is taken is it among the keys released. `gone` is
    /// read before this is called, so every press the input sent before it
    /// went away is in the ring by then.
    pub fn step(
        &mut self,
        law: &mut Law,
        readings: &mut Consumer<Reading>,
        presses: &mut Consumer<Press>,
        gone: bool,
        hush: &mut Producer<Monitor>,
    ) {
        self.take(law, readings, presses);
        if gone {
            for pitch in self.keys.release_all() {
                let _ = hush.push(Monitor::Off { pitch });
            }
        }
    }
}

/// Ends every note the law holds at `instant`, when the jam stops, lowest
/// pitch first.
pub fn release_all(
    law: &mut Law,
    clocks: &Clocks,
    held: &mut Held,
    instant: u64,
    log: &mut Vec<Passed>,
) {
    let sample = clocks.audio.sample_at(instant);
    for pitch in 0..=127u8 {
        if held.holds(pitch) {
            note_off(law, held, pitch, sample, log);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Reading;
    use crate::score::{Piece, root};
    use law::Voice;

    #[test]
    fn midi_messages_read_as_presses() {
        // Status, pitch, velocity, in the low three bytes.
        assert_eq!(midi_press(0x00_64_3C_90), Some((true, 60, 100)));
        assert_eq!(
            midi_press(0x00_00_3C_90),
            Some((false, 60, 0)),
            "velocity 0"
        );
        assert_eq!(midi_press(0x00_40_3C_80), Some((false, 60, 64)));
        assert_eq!(
            midi_press(0x00_64_45_9F),
            Some((true, 69, 100)),
            "channel 16"
        );
        assert_eq!(midi_press(0x00_7F_07_B0), None, "a controller");
        assert_eq!(midi_press(0x00_00_00_F8), None, "a clock tick");
    }

    /// Stream instant 5 s is law sample 0, at 48 kHz.
    fn heard_at(sample: u64) -> u64 {
        5_000_000_000 + sample * 1_000_000_000 / 48_000
    }

    /// Clocks with one reading per 480 frames through law sample `through`.
    fn clocks_through(through: u64) -> Clocks {
        let mut clocks = Clocks::default();
        let mut frame = 0;
        while frame <= through {
            clocks.audio.push(Reading {
                sample: frame,
                nanos: heard_at(frame),
            });
            frame += 480;
        }
        clocks
    }

    fn key(down: bool, pitch: u8, at: u64) -> (Press, u64) {
        let press = Press {
            down,
            pitch,
            velocity: if down { 100 } else { 0 },
            stamp: Stamp::Stream {
                nanos: heard_at(at),
            },
        };
        (press, heard_at(at))
    }

    /// A key going down is a note-on at the sample it was heard, and coming up
    /// a note-off; the law's frames carry the length between. A release with
    /// no note held passes nothing; a second note-on of a held pitch ends the
    /// held note first; the end of the jam ends what is still held.
    #[test]
    fn presses_become_note_ons_and_note_offs() {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        while law.steps() < 2_000 {
            law.step().unwrap();
        }
        let clocks = clocks_through(90_000);
        let mut held = Held::default();
        let mut log = Vec::new();
        for (down, pitch, at) in [
            (true, 74, 0),
            (true, 86, 12),
            (false, 74, 4_000),
            (false, 74, 4_100),
            (true, 76, 9_999),
            (true, 76, 12_000),
        ] {
            let (press, instant) = key(down, pitch, at);
            pass(&mut law, &clocks, &mut held, press, instant, &mut log);
        }
        release_all(&mut law, &clocks, &mut held, heard_at(15_000), &mut log);
        let summary: Vec<(bool, u8, Option<i64>)> =
            log.iter().map(|p| (p.down, p.pitch, p.sample)).collect();
        assert_eq!(
            summary,
            [
                (true, 74, Some(0)),
                (true, 86, Some(12)),
                (false, 74, Some(4_000)),
                (true, 76, Some(9_999)),
                (false, 76, Some(12_000)),
                (true, 76, Some(12_000)),
                (false, 76, Some(15_000)),
                (false, 86, Some(15_000)),
            ],
            "the second release of 74 passed nothing; 76 went down twice"
        );
        assert!(log.iter().all(|p| p.refused.is_none()), "{log:#?}");
        assert!(!held.holds(74) && !held.holds(76) && !held.holds(86));
        let lengths: Vec<(u64, u8, u64)> = law
            .frames(0, 1_000)
            .unwrap()
            .notes
            .iter()
            .filter(|n| n.voice == Voice::Live)
            .map(|n| (n.onset_sample, n.pitch, n.duration_samples))
            .collect();
        assert_eq!(
            lengths,
            [
                (0, 74, 4_000),
                (12, 86, 14_988),
                (9_999, 76, 2_001),
                (12_000, 76, 3_000)
            ]
        );
    }

    /// A press before law time has no sample and is passed to no one; its
    /// release has nothing to end. A note-off on its note-on's sample is
    /// refused by the law, the note stays held, and the next note-on of the
    /// pitch ends it.
    #[test]
    fn what_the_law_cannot_take_is_logged_and_the_note_stays_held() {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        while law.steps() < 2_000 {
            law.step().unwrap();
        }
        let mut log = Vec::new();
        let mut held = Held::default();
        let (press, instant) = key(true, 74, 0);
        pass(
            &mut law,
            &Clocks::default(),
            &mut held,
            press,
            instant,
            &mut log,
        );
        let (press, instant) = key(false, 74, 100);
        pass(
            &mut law,
            &Clocks::default(),
            &mut held,
            press,
            instant,
            &mut log,
        );
        assert_eq!(log.len(), 1, "{log:#?}");
        assert_eq!((log[0].sample, &log[0].refused), (None, &None));
        assert!(!held.holds(74));

        let clocks = clocks_through(20_000);
        log.clear();
        for (down, at) in [(true, 1_000), (false, 1_000), (true, 5_000)] {
            let (press, instant) = key(down, 74, at);
            pass(&mut law, &clocks, &mut held, press, instant, &mut log);
        }
        let codes: Vec<(bool, Option<i64>, Option<u32>)> = log
            .iter()
            .map(|p| (p.down, p.sample, p.refused.as_ref().map(|r| r.code)))
            .collect();
        assert_eq!(
            codes,
            [
                (true, Some(1_000), None),
                (false, Some(1_000), Some(165)),
                (false, Some(5_000), None),
                (true, Some(5_000), None),
            ]
        );
        assert!(held.holds(74));
    }

    /// Plays the score's first `count` notes (the first twelve are six
    /// octaves) as a person would who hears them exactly: each key down at the
    /// instant its score note is heard and up 4,000 samples later, through a
    /// device whose readings put law sample 0 at stream instant 5 s, with the
    /// law's playhead on the quantum heard, as a jam steps it. `wrong` plays
    /// one of the notes on another key; `stamp` stamps a press received at a
    /// stream instant. The jam stops at law sample `stop`, and the law's
    /// transport runs on until every score note before it has closed. Returns
    /// the rows and what was passed.
    fn play_the_opening(
        stamp: impl Fn(u64) -> Stamp,
        wrong: Option<(usize, u8)>,
        count: usize,
        stop: u64,
    ) -> (Vec<String>, Vec<Passed>) {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        let mut notes: Vec<(u64, u8)> = piece
            .score
            .notes()
            .iter()
            .take(count)
            .map(|n| (n.onset_sample, n.pitch))
            .collect();
        if let Some((index, pitch)) = wrong {
            notes[index].1 = pitch;
        }
        let mut clocks = Clocks::default();
        let mut held = Held::default();
        let mut passed = Vec::new();
        let mut frame = 0u64;
        for &(onset, pitch) in &notes {
            // Every callback's reading up to past this note, one per 480
            // frames, and the playhead on the quantum of its onset.
            while frame <= onset + 4_800 {
                clocks.audio.push(Reading {
                    sample: frame,
                    nanos: heard_at(frame),
                });
                frame += 480;
            }
            while law.steps() * 48 <= onset {
                law.step().unwrap();
            }
            for (down, at) in [(true, onset), (false, onset + 4_000)] {
                let stamp = stamp(heard_at(at));
                let instant = clocks.instant(stamp).unwrap();
                let press = Press {
                    down,
                    pitch,
                    velocity: 100,
                    stamp,
                };
                pass(&mut law, &clocks, &mut held, press, instant, &mut passed);
            }
        }
        close_through(&mut law, i64::try_from(stop).unwrap()).unwrap();
        let rows = law
            .record()
            .unwrap()
            .rows
            .lines()
            .map(String::from)
            .collect();
        (rows, passed)
    }

    /// Keys pressed exactly when the score's notes are heard read as matches at
    /// +0 samples; a key a semitone below its note, on the note's own sample,
    /// reads as a wrong pitch at +0 samples against that note. No score note
    /// the performance has reached is left unplayed, so no row says never
    /// played.
    #[test]
    fn keys_played_on_time_grade_as_matches_at_zero() {
        // Note 4 is 72 at sample 19,999; 71 is played for it.
        let (rows, passed) =
            play_the_opening(|nanos| Stamp::Stream { nanos }, Some((4, 71)), 12, 60_000);
        assert_eq!(passed.len(), 24);
        assert!(passed.iter().all(|p| p.refused.is_none()), "{passed:#?}");
        // Handed over as heard: the playhead had not passed a note-on's onset.
        assert!(
            passed.iter().filter(|p| p.down).all(|p| p.lag <= Some(0)),
            "{passed:#?}"
        );
        assert_eq!(delivery(&passed).map(|(_, _, past)| past), Some(0));
        assert_eq!(rows.len(), 12, "{rows:#?}");
        assert_eq!(
            rows[4],
            "note 4: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 71 vs 72: wrong pitch"
        );
        for (i, row) in rows.iter().enumerate().filter(|(i, _)| *i != 4) {
            assert!(
                row.starts_with(&format!("note {i}: onset +0 samples (+0.0 ms)")),
                "{row}"
            );
            assert!(row.ends_with(": match"), "{row}");
        }
    }

    /// A jam that stops partway: eight notes played, the jam stopped at sample
    /// 55,000. The transport runs on until every score note before the stop has
    /// closed, so the eight have their rows, notes 8 and 9 (sample 49,999),
    /// passed with no key, are never played, and notes 10 and 11 (59,999),
    /// after the stop, have no row.
    #[test]
    fn a_jam_closes_through_its_stop() {
        let (rows, _) = play_the_opening(|nanos| Stamp::Stream { nanos }, None, 8, 55_000);
        assert_eq!(rows.len(), 10, "{rows:#?}");
        assert!(
            rows[..8].iter().all(|r| r.ends_with(": match")),
            "{rows:#?}"
        );
        assert_eq!(
            rows[8],
            "note 8: onset 49999 samples, pitch 71, no take note cites it: never played"
        );
        assert!(rows[9].starts_with("note 9: ") && rows[9].ends_with("never played"));
    }

    /// Closing through a stop, to the sample: a note one sample before the
    /// stop closes, one a quantum after it does not.
    #[test]
    fn closing_through_a_stop_closes_every_note_before_it() {
        use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};
        // One tick is one sample: 70,000 us a quarter at source PPQ 3,360.
        let notes = [1_000u64, 2_000, 2_049]
            .iter()
            .map(|&start| IngestedNote {
                start_tick: start,
                pitch: 60,
                track: 1,
                channel: 0,
                end_tick: start + 10,
                velocity: 80,
            })
            .collect();
        let score = law::wire::encode_score(&IngestedScore {
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
        .unwrap();
        let mut law = Law::acquire();
        law.load_score(&score).unwrap();
        law.step().unwrap();
        close_through(&mut law, 2_001).unwrap();
        let rows = law.record().unwrap().rows;
        let rows: Vec<&str> = rows.lines().collect();
        assert_eq!(
            rows,
            [
                "note 0: onset 1000 samples, pitch 60, no take note cites it: never played",
                "note 1: onset 2000 samples, pitch 60, no take note cites it: never played",
            ]
        );
    }

    /// Issue #7's second finding. A MIDI keyboard is unplugged while the law
    /// thread has a key's note-on still in the presses ring: the monitor has
    /// sounded the key, and no note-off will ever come for it. The jam's
    /// loop, told the input may be gone, releases every key held in the
    /// monitor, and that key is among them, so its voice does not drone.
    #[test]
    fn a_press_queued_when_the_input_goes_away_is_released_in_the_monitor() {
        use rtrb::RingBuffer;

        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        while law.steps() < 2_000 {
            law.step().unwrap();
        }
        let (mut readings_in, mut readings) = RingBuffer::new(64);
        let (mut presses_in, mut presses) = RingBuffer::new(64);
        let (mut hush, mut released) = RingBuffer::new(64);
        readings_in
            .push(Reading {
                sample: 0,
                nanos: heard_at(0),
            })
            .unwrap();
        let mut taker = Taker::default();
        // One key held and taken, and a second still queued when the input
        // goes away.
        for pitch in [62, 67] {
            let (press, _) = key(true, pitch, 1_000);
            presses_in.push(press).unwrap();
            if pitch == 62 {
                taker.step(&mut law, &mut readings, &mut presses, false, &mut hush);
            }
        }
        taker.step(&mut law, &mut readings, &mut presses, true, &mut hush);
        let mut offs = Vec::new();
        while let Ok(Monitor::Off { pitch }) = released.pop() {
            offs.push(pitch);
        }
        assert_eq!(offs, [62, 67], "every key the monitor sounds is released");
        // The law took both note-ons.
        assert!(taker.held.holds(62) && taker.held.holds(67));
    }

    /// The keys held down follow the presses, and releasing them all names
    /// each held key once, lowest first.
    #[test]
    fn the_keys_held_follow_the_presses() {
        let press = |down, pitch| Press {
            down,
            pitch,
            velocity: 90,
            stamp: Stamp::Stream { nanos: 0 },
        };
        let mut keys = Keys::default();
        for p in [
            press(true, 64),
            press(true, 60),
            press(true, 67),
            press(false, 64),
        ] {
            keys.press(&p);
        }
        assert_eq!(keys.release_all(), [60, 67]);
        assert!(keys.release_all().is_empty());
    }

    /// The delivery summary: the median and largest lag of the note-ons, and
    /// how many were past the allowance.
    #[test]
    fn delivery_reads_the_note_ons_lags() {
        let at = |down, lag| Passed {
            down,
            pitch: 60,
            velocity: 0,
            sample: Some(0),
            lag,
            refused: None,
        };
        assert_eq!(delivery(&[]), None);
        let log = [
            at(true, Some(40)),
            at(true, Some(-7)),
            at(false, Some(9_000)),
            at(true, Some(4_801)),
            at(true, None),
            at(true, Some(4_800)),
        ];
        assert_eq!(delivery(&log), Some((4_800, 4_801, 1)));
    }

    /// The same notes through MIDI: WinMM stamps each in whole milliseconds
    /// since it started (stream instant 4 s), and each arrives 0.1 to 0.4 ms
    /// after it happened. The MIDI clock's offset comes from the earliest
    /// arrival, so every note lands within WinMM's millisecond, 48 samples,
    /// and grades a match.
    #[test]
    fn midi_notes_played_on_time_grade_as_matches_within_a_millisecond() {
        let arrival = std::cell::Cell::new(0u64);
        let (rows, passed) = play_the_opening(
            |nanos| {
                let delay = 100_000 + arrival.get() % 4 * 100_000;
                arrival.set(arrival.get() + 1);
                Stamp::Midi {
                    micros: (nanos - 4_000_000_000) / 1_000_000 * 1_000,
                    arrived: nanos + delay,
                }
            },
            None,
            12,
            60_000,
        );
        assert!(passed.iter().all(|p| p.refused.is_none()), "{passed:#?}");
        assert_eq!(rows.len(), 12, "{rows:#?}");
        for row in &rows {
            assert!(row.ends_with(": match"), "{row}");
            let samples: i64 = row
                .split("onset ")
                .nth(1)
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap();
            assert!(samples.abs() <= 48, "{row}");
        }
    }
}
