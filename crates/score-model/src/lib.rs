//! The interface between score ingest and the law.
//!
//! Ingest turns a source file into an [`IngestedScore`] in the file's own metrical
//! resolution; the law rescales it to its own PPQ, and refuses it when a tick cannot
//! be represented exactly. Everything here is an integer. There is no I/O, no clock
//! and no float, so this crate builds for `wasm32-unknown-unknown` inside the law.
//!
//! Ingest must hand over a score that passes [`IngestedScore::validate`]; the law
//! calls it again and refuses on any error, so neither side trusts the other.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

/// The SMF default tempo, 120 quarter notes per minute.
pub const DEFAULT_US_PER_QUARTER: u32 = 500_000;

/// SMF stores tempo in 24 bits.
pub const MAX_US_PER_QUARTER: u32 = 0x00FF_FFFF;

/// A metrical score in the source file's resolution. Timecode (SMPTE) files never
/// reach this type: ingest refuses them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestedScore {
    /// Ticks per quarter note in the source file (the SMF header's metrical division).
    pub source_ppq: u16,
    /// Sorted by tick with no duplicate ticks; the first change is at tick 0.
    pub tempo: Vec<TempoChange>,
    /// Sorted by tick with no duplicate ticks; the first change is at tick 0.
    pub meter: Vec<MeterChange>,
    /// Sorted by `(start_tick, pitch, track, channel, end_tick)`, no exact duplicates.
    pub notes: Vec<IngestedNote>,
}

/// A tempo change, in the source file's ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoChange {
    pub tick: u64,
    /// Microseconds per quarter note, 1..=[`MAX_US_PER_QUARTER`].
    pub us_per_quarter: u32,
}

/// A time-signature change, in the source file's ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeterChange {
    pub tick: u64,
    pub numerator: u8,
    /// The denominator as a power of two, as SMF stores it: 2 means a quarter note.
    pub denominator_pow2: u8,
}

/// One sounding note, in the source file's ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IngestedNote {
    pub start_tick: u64,
    pub pitch: u8,
    pub track: u16,
    pub channel: u8,
    /// Strictly after `start_tick`.
    pub end_tick: u64,
    /// 1..=127. A note-on with velocity 0 is a note-off in SMF and never a note.
    pub velocity: u8,
}

/// Why a score is not well-formed. The law turns each into a refusal with a reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    ZeroPpq,
    NoTempoAtZero,
    TempoNotSorted { index: usize },
    TempoOutOfRange { index: usize },
    NoMeterAtZero,
    MeterNotSorted { index: usize },
    MeterOutOfRange { index: usize },
    NoNotes,
    NotesNotSorted { index: usize },
    DuplicateNote { index: usize },
    EmptyNote { index: usize },
    PitchOutOfRange { index: usize },
    ChannelOutOfRange { index: usize },
    VelocityOutOfRange { index: usize },
}

impl IngestedScore {
    /// Checks every invariant the law relies on. The law calls this again itself.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.source_ppq == 0 {
            return Err(ModelError::ZeroPpq);
        }
        if self.tempo.first().map(|t| t.tick) != Some(0) {
            return Err(ModelError::NoTempoAtZero);
        }
        for (index, t) in self.tempo.iter().enumerate() {
            if t.us_per_quarter == 0 || t.us_per_quarter > MAX_US_PER_QUARTER {
                return Err(ModelError::TempoOutOfRange { index });
            }
            if index > 0 && self.tempo[index - 1].tick >= t.tick {
                return Err(ModelError::TempoNotSorted { index });
            }
        }
        if self.meter.first().map(|m| m.tick) != Some(0) {
            return Err(ModelError::NoMeterAtZero);
        }
        for (index, m) in self.meter.iter().enumerate() {
            if m.numerator == 0 || m.denominator_pow2 > 6 {
                return Err(ModelError::MeterOutOfRange { index });
            }
            if index > 0 && self.meter[index - 1].tick >= m.tick {
                return Err(ModelError::MeterNotSorted { index });
            }
        }
        if self.notes.is_empty() {
            return Err(ModelError::NoNotes);
        }
        for (index, n) in self.notes.iter().enumerate() {
            if n.end_tick <= n.start_tick {
                return Err(ModelError::EmptyNote { index });
            }
            if n.pitch > 127 {
                return Err(ModelError::PitchOutOfRange { index });
            }
            if n.channel > 15 {
                return Err(ModelError::ChannelOutOfRange { index });
            }
            if n.velocity == 0 || n.velocity > 127 {
                return Err(ModelError::VelocityOutOfRange { index });
            }
            if index > 0 {
                let prev = &self.notes[index - 1];
                if prev == n {
                    return Err(ModelError::DuplicateNote { index });
                }
                if prev > n {
                    return Err(ModelError::NotesNotSorted { index });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn note(start_tick: u64, pitch: u8, end_tick: u64) -> IngestedNote {
        IngestedNote {
            start_tick,
            pitch,
            track: 1,
            channel: 0,
            end_tick,
            velocity: 64,
        }
    }

    fn score() -> IngestedScore {
        IngestedScore {
            source_ppq: 384,
            tempo: vec![TempoChange {
                tick: 0,
                us_per_quarter: DEFAULT_US_PER_QUARTER,
            }],
            meter: vec![MeterChange {
                tick: 0,
                numerator: 2,
                denominator_pow2: 2,
            }],
            notes: vec![note(0, 60, 192), note(0, 64, 192), note(192, 62, 384)],
        }
    }

    #[test]
    fn a_well_formed_score_passes() {
        assert_eq!(score().validate(), Ok(()));
    }

    #[test]
    fn field_order_is_the_sort_order() {
        // The derived Ord compares fields in declaration order, which is the
        // documented sort key: (start_tick, pitch, track, channel, end_tick).
        assert!(note(0, 60, 999) < note(0, 61, 1));
        assert!(note(0, 127, 1) < note(1, 0, 2));
    }

    #[test]
    fn each_invariant_refuses() {
        let mut s = score();
        s.source_ppq = 0;
        assert_eq!(s.validate(), Err(ModelError::ZeroPpq));

        let mut s = score();
        s.tempo[0].tick = 1;
        assert_eq!(s.validate(), Err(ModelError::NoTempoAtZero));

        let mut s = score();
        s.tempo.push(TempoChange {
            tick: 0,
            us_per_quarter: 400_000,
        });
        assert_eq!(s.validate(), Err(ModelError::TempoNotSorted { index: 1 }));

        let mut s = score();
        s.tempo[0].us_per_quarter = MAX_US_PER_QUARTER + 1;
        assert_eq!(s.validate(), Err(ModelError::TempoOutOfRange { index: 0 }));

        let mut s = score();
        s.meter.clear();
        assert_eq!(s.validate(), Err(ModelError::NoMeterAtZero));

        let mut s = score();
        s.meter[0].denominator_pow2 = 7;
        assert_eq!(s.validate(), Err(ModelError::MeterOutOfRange { index: 0 }));

        let mut s = score();
        s.notes.clear();
        assert_eq!(s.validate(), Err(ModelError::NoNotes));

        let mut s = score();
        s.notes.swap(0, 2);
        assert_eq!(s.validate(), Err(ModelError::NotesNotSorted { index: 1 }));

        let mut s = score();
        s.notes[1] = s.notes[0];
        assert_eq!(s.validate(), Err(ModelError::DuplicateNote { index: 1 }));

        let mut s = score();
        s.notes[2].end_tick = s.notes[2].start_tick;
        assert_eq!(s.validate(), Err(ModelError::EmptyNote { index: 2 }));

        let mut s = score();
        s.notes[2].pitch = 128;
        assert_eq!(s.validate(), Err(ModelError::PitchOutOfRange { index: 2 }));

        let mut s = score();
        s.notes[2].channel = 16;
        assert_eq!(
            s.validate(),
            Err(ModelError::ChannelOutOfRange { index: 2 })
        );

        let mut s = score();
        s.notes[2].velocity = 0;
        assert_eq!(
            s.validate(),
            Err(ModelError::VelocityOutOfRange { index: 2 })
        );
    }
}
