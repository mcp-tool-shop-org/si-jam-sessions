//! Live input, from a press to the law's live verb.
//!
//! A press is a key or MIDI note going down or up, stamped the moment the host
//! receives it: a key with the stream clock, a MIDI message with its WinMM
//! time and its arrival on the stream clock. The monitor has already started
//! or released a voice for it by then; this side pairs presses into notes and
//! passes each note to the law, through the clocks in [`crate::anchor`], at the
//! law sample at which it was heard. The law decides what the note cites and
//! grades it; the host decides nothing.

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

/// A note from its press to its release, on the stream clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Played {
    pub pitch: u8,
    pub velocity: u8,
    pub on: u64,
    pub off: u64,
}

/// Presses paired into notes, one held note per pitch.
pub struct Pairing {
    held: [Option<(u8, u64)>; 128],
}

impl Default for Pairing {
    fn default() -> Self {
        Pairing { held: [None; 128] }
    }
}

impl Pairing {
    /// A press at stream instant `nanos`. A release ends the note held at its
    /// pitch; a press ends one still held there first (a second note-on
    /// without a note-off) and starts another. Returns the note that ended.
    pub fn press(&mut self, down: bool, pitch: u8, velocity: u8, nanos: u64) -> Option<Played> {
        let slot = self.held.get_mut(usize::from(pitch & 0x7F))?;
        let ended = slot.take().map(|(v, on)| Played {
            pitch,
            velocity: v,
            on,
            off: nanos,
        });
        if down {
            *slot = Some((velocity, nanos));
        }
        ended
    }

    /// Ends every held note at `nanos`, when the jam stops.
    pub fn release_all(&mut self, nanos: u64) -> Vec<Played> {
        let mut out = Vec::new();
        for (pitch, slot) in self.held.iter_mut().enumerate() {
            if let Some((velocity, on)) = slot.take() {
                out.push(Played {
                    pitch: u8::try_from(pitch).unwrap_or(0),
                    velocity,
                    on,
                    off: nanos,
                });
            }
        }
        out.sort_by_key(|p| p.on);
        out
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

/// What the law made of one live note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passed {
    pub played: Played,
    /// The law sample the note was heard at, and its length in samples.
    pub onset: i64,
    pub duration: u64,
    /// `None` when the law admitted it.
    pub refused: Option<Refused>,
}

/// Passes a played note to the law's live verb, at the law samples its press
/// and release were heard at. A note is passed as measured: one the audio
/// clock cannot place yet, or one released on the sample it was pressed on,
/// is refused by the law with its own code, and reported.
pub fn pass(law: &mut Law, clocks: &Clocks, played: Played) -> Passed {
    let onset = clocks.audio.sample_at(played.on);
    let end = clocks.audio.sample_at(played.off);
    let (onset, duration) = match (onset, end) {
        (Some(onset), Some(end)) => (onset, u64::try_from(end - onset).unwrap_or(0)),
        _ => (i64::MIN, 0),
    };
    let refused = law
        .live_note(
            onset,
            u32::from(played.pitch),
            u32::from(played.velocity),
            duration,
        )
        .err();
    Passed {
        played,
        onset,
        duration,
        refused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Reading;
    use crate::score::{Piece, root};

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

    #[test]
    fn presses_pair_into_notes() {
        let mut pairing = Pairing::default();
        assert_eq!(pairing.press(true, 60, 90, 1_000), None);
        assert_eq!(pairing.press(true, 64, 80, 1_500), None);
        assert_eq!(
            pairing.press(false, 60, 0, 2_000),
            Some(Played {
                pitch: 60,
                velocity: 90,
                on: 1_000,
                off: 2_000
            })
        );
        assert_eq!(pairing.press(false, 60, 0, 2_100), None, "no note held");
        // A second note-on while held ends the first there.
        assert_eq!(
            pairing.press(true, 64, 70, 3_000),
            Some(Played {
                pitch: 64,
                velocity: 80,
                on: 1_500,
                off: 3_000
            })
        );
        assert_eq!(
            pairing.release_all(9_000),
            [Played {
                pitch: 64,
                velocity: 70,
                on: 3_000,
                off: 9_000
            }]
        );
        assert!(pairing.release_all(9_500).is_empty());
    }

    /// Plays the score's first notes as a person would who hears them exactly:
    /// each press at the instant its score note is heard, 4,000 samples long,
    /// through a device whose readings put law sample 0 at stream instant 5 s.
    /// `stamp` stamps a press received at a stream instant. Returns the rows of
    /// the notes played, and what was passed.
    fn play_the_opening(
        stamp: impl Fn(u64) -> Stamp,
        extra: Option<(u64, u8)>,
    ) -> (Vec<String>, Vec<Passed>) {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        let heard_at = |sample: u64| 5_000_000_000 + sample * 1_000_000_000 / 48_000;
        let mut notes: Vec<(u64, u8)> = piece
            .score
            .notes()
            .iter()
            .take(12)
            .map(|n| (n.onset_sample, n.pitch))
            .collect();
        notes.extend(extra);
        let mut clocks = Clocks::default();
        let mut pairing = Pairing::default();
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
                let instant = clocks.instant(stamp(heard_at(at))).unwrap();
                if let Some(played) = pairing.press(down, pitch, 100, instant) {
                    passed.push(pass(&mut law, &clocks, played));
                }
            }
        }
        let rows = law
            .record()
            .unwrap()
            .rows
            .lines()
            .filter(|r| !r.ends_with("never played"))
            .map(String::from)
            .collect();
        (rows, passed)
    }

    /// Keys pressed exactly when the score's notes are heard read as matches at
    /// +0 samples; a key on a beat that no score note near it plays reads as a
    /// wrong pitch at +0 samples, against the beat's note nearest in pitch.
    #[test]
    fn keys_played_on_time_grade_as_matches_at_zero() {
        let (rows, passed) = play_the_opening(|nanos| Stamp::Stream { nanos }, Some((0, 30)));
        assert!(passed.iter().all(|p| p.refused.is_none()), "{passed:#?}");
        assert!(passed.iter().all(|p| p.duration == 4_000), "{passed:#?}");
        assert_eq!(rows.len(), 13, "{rows:#?}");
        let (wrong, matched): (Vec<&String>, Vec<&String>) =
            rows.iter().partition(|r| r.ends_with("wrong pitch"));
        assert_eq!(wrong.len(), 1, "{rows:#?}");
        assert!(
            wrong[0].contains("onset +0 samples (+0.0 ms)"),
            "{}",
            wrong[0]
        );
        assert!(wrong[0].contains("pitch 30 vs "), "{}", wrong[0]);
        for row in matched {
            assert!(row.contains("onset +0 samples (+0.0 ms)"), "{row}");
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
