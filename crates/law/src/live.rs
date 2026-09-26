//! The live verbs' inputs, and the rule that decides which score note a live
//! note answers: slice 1's placeholder score follower.
//!
//! A live note is a record, not a proposal (PHASE-0, "Live input is a record,
//! not a proposal"). The host passes a note-on when a key goes down, with its
//! onset on the law's sample clock, its pitch and its velocity; and a note-off
//! when the key comes up. The host decides nothing else, so the law decides
//! what the note cites, by the rule in [`cite`]. The note then goes through the
//! same admission as a take note ([`crate::Law::admit`], without the commit
//! horizon) and the same grading.
//!
//! PHASE-0 names a real score follower as a later direction; this rule stands
//! in for it until then.

use crate::refusal::Refusal;
use crate::score::LawScore;
use crate::take::ScoreNoteId;
use crate::{GATE_SAMPLES, LIVE_REACH_SAMPLES};

/// A note-on a person played, as the host passes it to the live verb
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
}

/// A note-off: the key of `pitch` came up at `off_sample`
/// ([`crate::Law::live_off`]). It ends the latest live note of that pitch
/// still held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveNoteOff {
    /// 0..=127.
    pub pitch: u32,
    /// The release on the law's sample clock.
    pub off_sample: i64,
}

/// The score note a live note of `pitch` at `onset` answers, or `None` for an
/// addition. `cited[i]` says whether a take note already cites score note `i`.
///
/// 1. **The same pitch, within the reach.** Among the score notes of the live
///    note's pitch whose onset is at most [`LIVE_REACH_SAMPLES`] away, the
///    nearest in onset. On a tie in distance, the one before the live note
///    (the note is late for it, not early for the next); on a tie at one onset
///    (a pitch doubled on two tracks), the lowest id. Grading then says match,
///    early or late.
/// 2. **Another pitch, within the gate.** Otherwise, among the score notes
///    whose onset is at most [`GATE_SAMPLES`] away, the nearest in onset, then
///    the nearest in pitch, then the lower pitch, then the lowest id. Grading
///    says wrong pitch.
/// 3. **Otherwise an addition.**
///
/// **A score note is cited at most once.** A score note some take note already
/// cites is passed over at both steps, so a second live note in its reach
/// cites the next candidate in the same order, or becomes an addition.
///
/// **Chords.** Step 1 runs before step 2, so every note of a chord that is
/// played at its own pitch finds its own score note, in whatever order the
/// chord's keys go down. A wrong key that goes down before the right one takes
/// the chord note nearest in pitch, and the right key, finding it cited, takes
/// the next: a placeholder follower does not revise a citation it has made.
///
/// Because cited notes are passed over, a note's citation depends on the notes
/// admitted before it, so the verb takes live notes in the order the host
/// passes them: the order their keys went down.
pub(crate) fn cite(
    score: &LawScore,
    cited: &[bool],
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
        if cited.get(index).copied().unwrap_or(false) {
            continue;
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
