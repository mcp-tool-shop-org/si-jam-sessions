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

use crate::anchor::{AudioClock, MidiClock};
use crate::bridge::{Law, Refused};

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
    /// The law's refusal. `None` when the law took it, or when nothing was
    /// passed.
    pub refused: Option<Refused>,
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
            refused,
        });
    } else if held.holds(press.pitch) {
        note_off(law, held, press.pitch, sample, log);
    }
}

fn note_off(law: &mut Law, held: &mut Held, pitch: u8, sample: Option<i64>, log: &mut Vec<Passed>) {
    let refused = sample.and_then(|at| law.live_note_off(u32::from(pitch), at).err());
    if sample.is_some() && refused.is_none() {
        held.set(pitch, false);
    }
    log.push(Passed {
        down: false,
        pitch,
        velocity: 0,
        sample,
        refused,
    });
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

    /// Plays the score's first twelve notes, six octaves, as a person would
    /// who hears them exactly: each key down at the instant its score note is
    /// heard and up 4,000 samples later, through a device whose readings put
    /// law sample 0 at stream instant 5 s. `wrong` plays one of the notes on
    /// another key. `stamp` stamps a press received at a stream instant.
    /// Returns the rows and what was passed.
    fn play_the_opening(
        stamp: impl Fn(u64) -> Stamp,
        wrong: Option<(usize, u8)>,
    ) -> (Vec<String>, Vec<Passed>) {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        let mut notes: Vec<(u64, u8)> = piece
            .score
            .notes()
            .iter()
            .take(12)
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
            // frames, and the law stepped past it.
            while frame <= onset + 4_800 {
                clocks.audio.push(Reading {
                    sample: frame,
                    nanos: heard_at(frame),
                });
                frame += 480;
            }
            while law.steps() * 48 < onset + 4_800 {
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
        let (rows, passed) = play_the_opening(|nanos| Stamp::Stream { nanos }, Some((4, 71)));
        assert_eq!(passed.len(), 24);
        assert!(passed.iter().all(|p| p.refused.is_none()), "{passed:#?}");
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
