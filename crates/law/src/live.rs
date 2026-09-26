//! The live verb's input, and the rule that decides which score note a live
//! note answers.
//!
//! A live note is a record, not a proposal (PHASE-0, "Live input is a record,
//! not a proposal"). The host timestamps it and passes its onset on the law's
//! sample clock, its pitch, its velocity and how long it sounded. The host
//! decides nothing else, so the law decides what the note cites, by the rule in
//! [`cite`]. The note then goes through the same admission as a take note
//! ([`crate::Law::admit`], without the commit horizon) and the same grading.

use crate::refusal::Refusal;
use crate::score::LawScore;
use crate::take::ScoreNoteId;
use crate::{GATE_SAMPLES, LIVE_REACH_SAMPLES};

/// One note a person played, as the host passes it to the live verb
/// ([`crate::Law::live`]).
///
/// The fields are wider than a take note's so that a value out of range is
/// refused by name rather than cut to fit: a negative onset is before the take
/// starts, and a pitch or velocity above 127 is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveNote {
    /// The onset on the law's 48 kHz sample clock, where sample 0 is score
    /// tick 0. Negative when the note was played before the take started.
    pub onset_sample: i64,
    /// 0..=127.
    pub pitch: u32,
    /// 1..=127.
    pub velocity: u32,
    /// How long the note sounded, from its note-on to its note-off, in
    /// samples: at least 1.
    pub duration_samples: u64,
}

/// The score note a live note of `pitch` at `onset` answers, or `None` for an
/// addition.
///
/// 1. **The same pitch, within the reach.** Among the score notes of the live
///    note's pitch whose onset is at most [`LIVE_REACH_SAMPLES`] away, the
///    nearest in onset. On a tie, the one before the live note (the note is
///    late for it, not early for the next); on a tie at one onset, the lowest
///    id. Grading then says match, early or late.
/// 2. **Another pitch, within the gate.** Otherwise, among the score notes
///    whose onset is at most [`GATE_SAMPLES`] away, the nearest in onset, then
///    the nearest in pitch, then the lower pitch, then the lowest id. Grading
///    says wrong pitch.
/// 3. **Otherwise an addition.**
///
/// The rule reads the score only, never the take, so a set of live notes gets
/// the same citations in whatever order it arrives, and two live notes may
/// answer one score note (a re-strike), as two take notes may.
pub(crate) fn cite(
    score: &LawScore,
    onset: u64,
    pitch: u8,
) -> Result<Option<ScoreNoteId>, Refusal> {
    let reach = u64::from(LIVE_REACH_SAMPLES);
    let gate = u64::from(GATE_SAMPLES);
    let low = onset.saturating_sub(reach);
    let high = onset.checked_add(reach).ok_or(Refusal::Overflow)?;
    let notes = score.notes();
    // Score notes are in canonical order, which is onset order: the tempo map
    // is monotone in the tick.
    let start = notes.partition_point(|n| n.onset_sample < low);
    // (distance, after the live note, id)
    let mut same: Option<(u64, bool, u32)> = None;
    // (distance, pitch distance, pitch, id)
    let mut other: Option<(u64, u8, u8, u32)> = None;
    for (index, n) in notes.iter().enumerate().skip(start) {
        if n.onset_sample > high {
            break;
        }
        let id = u32::try_from(index).map_err(|_| Refusal::TooManyNotes { count: notes.len() })?;
        let distance = n.onset_sample.abs_diff(onset);
        if n.pitch == pitch {
            let key = (distance, n.onset_sample > onset, id);
            if same.is_none_or(|best| key < best) {
                same = Some(key);
            }
        } else if distance <= gate {
            let key = (distance, n.pitch.abs_diff(pitch), n.pitch, id);
            if other.is_none_or(|best| key < best) {
                other = Some(key);
            }
        }
    }
    Ok(match (same, other) {
        (Some((_, _, id)), _) => Some(ScoreNoteId(id)),
        (None, Some((_, _, _, id))) => Some(ScoreNoteId(id)),
        (None, None) => None,
    })
}
