//! A played take: integer sample onsets at 48 kHz, each citing a score note or
//! marked as an addition.

use crate::QUANTUM_SAMPLES;
use crate::refusal::Refusal;

/// A score note's id: its index in the score's canonical order
/// ([`crate::LawScore::notes`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScoreNoteId(pub u32);

/// One note of a take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TakeNote {
    /// The onset on the law's 48 kHz sample clock, where sample 0 is score
    /// tick 0.
    pub onset_sample: u64,
    /// 0..=127.
    pub pitch: u8,
    /// 1..=127.
    pub velocity: u8,
    /// The score note this take note answers, or `None` for an addition.
    pub cites: Option<ScoreNoteId>,
}

/// A take note's position in the take's total order.
pub type TakeKey = (u64, u8, Option<ScoreNoteId>);

impl TakeNote {
    /// The take's total order: `(onset_sample, pitch, cites)`, with an
    /// addition (`None`) before any citation at the same onset and pitch.
    ///
    /// A take is strictly increasing in this key, so two notes may share an
    /// onset and a pitch only when they cite different score notes: a score
    /// can double one pitch on two tracks, and a take answers each. Velocity
    /// is not part of the key, so one key struck twice is refused as a
    /// duplicate rather than ordered by loudness.
    pub fn key(&self) -> TakeKey {
        (self.onset_sample, self.pitch, self.cites)
    }

    /// The quantum the onset falls in: `onset_sample / Q`.
    pub fn quantum(&self) -> Result<u64, Refusal> {
        self.onset_sample
            .checked_div(u64::from(QUANTUM_SAMPLES))
            .ok_or(Refusal::Overflow)
    }
}
