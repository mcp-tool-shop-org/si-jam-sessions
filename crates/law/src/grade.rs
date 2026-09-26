//! Grading: every take note against the score note it cites, and one row per
//! note whose text states the comparison in digits.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write};

use crate::frames::LiveLength;
use crate::refusal::Refusal;
use crate::score::{LawNote, LawScore};
use crate::take::{ScoreNoteId, TakeNote};
use crate::{CLOSE_SAMPLES, GATE_SAMPLES, QUANTUM_SAMPLES, SAMPLES_PER_MS};

/// How a cited take note compares with the score note it cites.
///
/// `delta` is the take note's onset minus the score note's onset, in samples.
/// Timing is on time when `|delta| <= GATE_SAMPLES` (both edges inclusive; see
/// [`GATE_SAMPLES`]). Pitch must be exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CitedKind {
    /// Exact pitch, and `|delta| <= GATE_SAMPLES`.
    Match,
    /// Exact pitch, and `delta < -GATE_SAMPLES`.
    Early,
    /// Exact pitch, and `delta > GATE_SAMPLES`.
    Late,
    /// The pitch is not the score note's. A wrong pitch is this verdict
    /// whatever its timing; its row still states the timing in digits, so a
    /// wrong note that is also late reads as both.
    WrongPitch,
}

/// One graded note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// A take note that cites a score note.
    Cited {
        /// The cited score note.
        note: ScoreNoteId,
        /// The take note's index in the take.
        take: u32,
        kind: CitedKind,
        /// Take onset minus score onset, in samples.
        delta: i64,
        /// The take note's onset.
        onset_sample: u64,
        take_pitch: u8,
        score_pitch: u8,
    },
    /// A take note that cites no score note.
    Addition {
        /// The take note's index in the take.
        take: u32,
        onset_sample: u64,
        pitch: u8,
    },
    /// A score note that no take note cites.
    NeverPlayed {
        note: ScoreNoteId,
        /// The score note's onset.
        onset_sample: u64,
        pitch: u8,
    },
}

impl Verdict {
    /// The verdict's word, which ends its row.
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Cited { kind, .. } => match kind {
                CitedKind::Match => "match",
                CitedKind::Early => "early",
                CitedKind::Late => "late",
                CitedKind::WrongPitch => "wrong pitch",
            },
            Verdict::Addition { .. } => "addition",
            Verdict::NeverPlayed { .. } => "never played",
        }
    }

    /// The verdict's kind as the snapshot writes it: match 0, early 1, late 2,
    /// wrong pitch 3, addition 4, never played 5.
    pub fn kind_code(&self) -> u8 {
        match self {
            Verdict::Cited { kind, .. } => match kind {
                CitedKind::Match => 0,
                CitedKind::Early => 1,
                CitedKind::Late => 2,
                CitedKind::WrongPitch => 3,
            },
            Verdict::Addition { .. } => 4,
            Verdict::NeverPlayed { .. } => 5,
        }
    }
}

fn reserved<T>(len: usize) -> Result<Vec<T>, Refusal> {
    let mut v = Vec::new();
    v.try_reserve_exact(len).map_err(|_| Refusal::OutOfMemory)?;
    Ok(v)
}

fn cited(note: ScoreNoteId, take: u32, t: &TakeNote, s: &LawNote) -> Result<Verdict, Refusal> {
    // Both onsets are at most MAX_SAMPLE = i64::MAX, so both convert and the
    // difference cannot overflow; the checks stay because the law does not
    // rely on a proof in a comment.
    let onset = i64::try_from(t.onset_sample).map_err(|_| Refusal::Overflow)?;
    let score_onset = i64::try_from(s.onset_sample).map_err(|_| Refusal::Overflow)?;
    let delta = onset.checked_sub(score_onset).ok_or(Refusal::Overflow)?;
    let kind = if t.pitch != s.pitch {
        CitedKind::WrongPitch
    } else if delta.unsigned_abs() <= u64::from(GATE_SAMPLES) {
        CitedKind::Match
    } else if delta < 0 {
        CitedKind::Early
    } else {
        CitedKind::Late
    };
    Ok(Verdict::Cited {
        note,
        take,
        kind,
        delta,
        onset_sample: t.onset_sample,
        take_pitch: t.pitch,
        score_pitch: s.pitch,
    })
}

/// Every take note's citation as (score note, take index), sorted. Take
/// indices are distinct, so no two pairs are equal and the unstable sort has
/// one possible result.
fn citations(take: &[TakeNote]) -> Result<Vec<(ScoreNoteId, u32)>, Refusal> {
    let too_long = Refusal::TakeTooLong { count: take.len() };
    let mut citations: Vec<(ScoreNoteId, u32)> = reserved(take.len())?;
    for (index, t) in take.iter().enumerate() {
        if let Some(id) = t.cites {
            citations.push((id, u32::try_from(index).map_err(|_| too_long)?));
        }
    }
    citations.sort_unstable();
    Ok(citations)
}

/// Grades a proposed take against a score, as law version 3 graded every take.
///
/// The verdicts come in one fixed order:
/// 1. for each score note in id order, a [`Verdict::Cited`] for every take
///    note that cites it, in take order, or one [`Verdict::NeverPlayed`] when
///    none does;
/// 2. then a [`Verdict::Addition`] for every take note that cites nothing, in
///    take order.
///
/// Every take note gets exactly one verdict, and so does every score note that
/// no take note cites. A live take is graded by [`final_verdicts`].
pub(crate) fn verdicts(score: &LawScore, take: &[TakeNote]) -> Result<Vec<Verdict>, Refusal> {
    let too_long = Refusal::TakeTooLong { count: take.len() };
    let citations = citations(take)?;

    let notes = score.notes();
    let total = notes
        .len()
        .checked_add(take.len())
        .ok_or(Refusal::Overflow)?;
    let mut out = reserved(total)?;
    let mut pending = citations.iter().peekable();
    for (index, s) in notes.iter().enumerate() {
        let id = ScoreNoteId(
            u32::try_from(index).map_err(|_| Refusal::TooManyNotes { count: notes.len() })?,
        );
        let mut played = false;
        while let Some(&&(cites, take_index)) = pending.peek() {
            if cites != id {
                break;
            }
            pending.next();
            played = true;
            let t = usize::try_from(take_index)
                .ok()
                .and_then(|i| take.get(i))
                .ok_or(Refusal::Overflow)?;
            out.push(cited(id, take_index, t, s)?);
        }
        if !played {
            out.push(Verdict::NeverPlayed {
                note: id,
                onset_sample: s.onset_sample,
                pitch: s.pitch,
            });
        }
    }
    // A citation past the last score note is refused at admission; one that
    // got here anyway is refused rather than dropped.
    if let Some(&&(cites, take_index)) = pending.peek() {
        return Err(Refusal::TakeCitation {
            index: usize::try_from(take_index).map_err(|_| Refusal::Overflow)?,
            cites: cites.0,
            notes: notes.len(),
        });
    }
    for (index, t) in take.iter().enumerate() {
        if t.cites.is_none() {
            out.push(Verdict::Addition {
                take: u32::try_from(index).map_err(|_| too_long)?,
                onset_sample: t.onset_sample,
                pitch: t.pitch,
            });
        }
    }
    Ok(out)
}

/// A row's place in a live take's stream: the step from which it is final,
/// its close point, score notes (0) before additions (1), then the score
/// note's id and the take index, or the addition's take index. Every row has
/// its own place.
type Place = (u64, u64, u8, u64, u64);

/// The verdicts of a live take that are final after `steps` steps, in the
/// order they became final.
///
/// Every row has a close point: its onset plus [`CLOSE_SAMPLES`], where the
/// onset is the score note's for a citation or a never-played verdict, and
/// the take note's own for an addition. The playhead after `n` steps is
/// `(n - 1) * Q`, and a row is final from the first step whose playhead has
/// passed its close point: step `close / Q + 2`.
/// - A score note closes there. Every take note that cites it was admitted
///   before (the live verb cites only open notes, and a proposal that cites a
///   closed one is refused), so its rows, one per citing take note in take
///   order or one never-played, are all known before they are shown.
/// - An addition's content is known at admission. It is shown from its close
///   point, or, when it was admitted after its close point had passed (a live
///   note delivered past its allowance), from the step after the one it was
///   admitted at.
///
/// So a row's place in the stream is fixed before the step that shows it is
/// reached, and the rows shown after any step begin with the rows shown after
/// every earlier step, in the same order. A score note still open has no row.
pub(crate) fn final_verdicts(
    score: &LawScore,
    take: &[TakeNote],
    live: &[LiveLength],
    steps: u64,
) -> Result<Vec<Verdict>, Refusal> {
    let too_long = Refusal::TakeTooLong { count: take.len() };
    let quantum = u64::from(QUANTUM_SAMPLES);
    let close = u64::from(CLOSE_SAMPLES);
    let final_from = |onset: u64| -> Result<(u64, u64), Refusal> {
        let point = onset.checked_add(close).ok_or(Refusal::Overflow)?;
        let step = point
            .checked_div(quantum)
            .and_then(|n| n.checked_add(2))
            .ok_or(Refusal::Overflow)?;
        Ok((step, point))
    };
    let citations = citations(take)?;
    let notes = score.notes();
    let mut placed: Vec<(Place, Verdict)> = Vec::new();
    let mut pending = citations.iter().peekable();
    for (index, s) in notes.iter().enumerate() {
        let id = ScoreNoteId(
            u32::try_from(index).map_err(|_| Refusal::TooManyNotes { count: notes.len() })?,
        );
        let (step, point) = final_from(s.onset_sample)?;
        let shown = step <= steps;
        let mut played = false;
        while let Some(&&(cites, take_index)) = pending.peek() {
            if cites != id {
                break;
            }
            pending.next();
            played = true;
            if shown {
                let t = usize::try_from(take_index)
                    .ok()
                    .and_then(|i| take.get(i))
                    .ok_or(Refusal::Overflow)?;
                let place = (step, point, 0, u64::from(id.0), u64::from(take_index));
                placed.try_reserve(1).map_err(|_| Refusal::OutOfMemory)?;
                placed.push((place, cited(id, take_index, t, s)?));
            }
        }
        if shown && !played {
            let place = (step, point, 0, u64::from(id.0), 0);
            placed.try_reserve(1).map_err(|_| Refusal::OutOfMemory)?;
            placed.push((
                place,
                Verdict::NeverPlayed {
                    note: id,
                    onset_sample: s.onset_sample,
                    pitch: s.pitch,
                },
            ));
        }
    }
    if let Some(&&(cites, take_index)) = pending.peek() {
        return Err(Refusal::TakeCitation {
            index: usize::try_from(take_index).map_err(|_| Refusal::Overflow)?,
            cites: cites.0,
            notes: notes.len(),
        });
    }
    for (index, t) in take.iter().enumerate() {
        if t.cites.is_some() {
            continue;
        }
        let (mut step, point) = final_from(t.onset_sample)?;
        if let Ok(i) = live.binary_search_by(|l| l.key.cmp(&t.key())) {
            let admitted = live.get(i).ok_or(Refusal::Overflow)?.admitted_at;
            step = step.max(admitted.checked_add(1).ok_or(Refusal::Overflow)?);
        }
        if step <= steps {
            let take_index = u32::try_from(index).map_err(|_| too_long)?;
            let place = (step, point, 1, u64::from(take_index), 0);
            placed.try_reserve(1).map_err(|_| Refusal::OutOfMemory)?;
            placed.push((
                place,
                Verdict::Addition {
                    take: take_index,
                    onset_sample: t.onset_sample,
                    pitch: t.pitch,
                },
            ));
        }
    }
    // Places are distinct, so the unstable sort has one possible result.
    placed.sort_unstable_by_key(|&(place, _)| place);
    let mut out = reserved(placed.len())?;
    out.extend(placed.into_iter().map(|(_, verdict)| verdict));
    Ok(out)
}

/// How an addition's row names its take note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Naming {
    /// By its index in the take, as law version 3 names it: a proposed take.
    Index,
    /// By its onset and pitch alone: a live take. A live note delivered past
    /// its allowance can land before notes already shown and move their
    /// indices, and a row once shown never changes.
    Onset,
}

/// The longest row is under 130 bytes (twenty-digit numbers included).
const ROW_CAPACITY: usize = 192;

/// A row being written: a fixed buffer, so formatting never allocates.
struct Line {
    bytes: [u8; ROW_CAPACITY],
    len: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
        let room = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        room.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// `samples` in tenths of a millisecond, rounded half away from zero:
/// `floor((20 × samples + 48) / 96)`, which is `samples × 10 / 48` rounded.
///
/// 48 samples are 1 ms, so a difference that is a multiple of 24 samples
/// (half a millisecond) is exact at one decimal, as every offset of the
/// constructed take is (30, 45, 60 ms). Any other difference is rounded; the
/// row also carries the exact difference in samples.
fn tenths_of_ms(samples: u64) -> Result<u64, Refusal> {
    let per_ms = u128::from(SAMPLES_PER_MS);
    let twice = u128::from(samples)
        .checked_mul(20)
        .and_then(|n| n.checked_add(per_ms))
        .ok_or(Refusal::Overflow)?;
    let tenths = per_ms
        .checked_mul(2)
        .and_then(|d| twice.checked_div(d))
        .ok_or(Refusal::Overflow)?;
    u64::try_from(tenths).map_err(|_| Refusal::Overflow)
}

fn write_row(line: &mut Line, verdict: &Verdict, naming: Naming) -> Result<(), Refusal> {
    let word = verdict.word();
    let written = match *verdict {
        Verdict::Cited {
            note,
            delta,
            take_pitch,
            score_pitch,
            ..
        } => {
            let sign = if delta < 0 { '-' } else { '+' };
            let samples = delta.unsigned_abs();
            let tenths = tenths_of_ms(samples)?;
            let whole_ms = tenths.checked_div(10).ok_or(Refusal::Overflow)?;
            let tenth = tenths.checked_rem(10).ok_or(Refusal::Overflow)?;
            write!(
                line,
                "note {}: onset {sign}{samples} samples ({sign}{whole_ms}.{tenth} ms) vs gate \
                 \u{b1}{GATE_SAMPLES}, pitch {take_pitch} vs {score_pitch}: {word}",
                note.0
            )
        }
        Verdict::Addition {
            take,
            onset_sample,
            pitch,
        } => match naming {
            Naming::Index => write!(
                line,
                "take note {take}: onset {onset_sample} samples, pitch {pitch}, cites no score \
                 note: {word}"
            ),
            Naming::Onset => write!(
                line,
                "take note at onset {onset_sample} samples, pitch {pitch}, cites no score note: \
                 {word}"
            ),
        },
        Verdict::NeverPlayed {
            note,
            onset_sample,
            pitch,
        } => write!(
            line,
            "note {}: onset {onset_sample} samples, pitch {pitch}, no take note cites it: {word}",
            note.0
        ),
    };
    written.map_err(|_| Refusal::Overflow)
}

/// The row for one verdict. It states the comparison in digits, with integer
/// formatting only:
///
/// ```text
/// note 17: onset +2160 samples (+45.0 ms) vs gate ±1920, pitch 65 vs 65: late
/// take note 3: onset 96000 samples, pitch 60, cites no score note: addition
/// note 18: onset 98400 samples, pitch 67, no take note cites it: never played
/// ```
///
/// In a live take an addition's row names its take note by onset and pitch
/// alone ([`Naming::Onset`]):
///
/// ```text
/// take note at onset 96000 samples, pitch 60, cites no score note: addition
/// ```
///
/// A cited row gives the onset difference (take minus score) in samples and in
/// milliseconds to one decimal, the gate, and the take's pitch against the
/// score's. Every cited row carries the pitch comparison, so a wrong pitch is
/// read from its digits as a timing verdict is.
pub(crate) fn row(verdict: &Verdict, naming: Naming) -> Result<String, Refusal> {
    let mut line = Line {
        bytes: [0; ROW_CAPACITY],
        len: 0,
    };
    write_row(&mut line, verdict, naming)?;
    let bytes = line.bytes.get(..line.len).ok_or(Refusal::Overflow)?;
    let text = core::str::from_utf8(bytes).map_err(|_| Refusal::Overflow)?;
    let mut out = String::new();
    out.try_reserve_exact(text.len())
        .map_err(|_| Refusal::OutOfMemory)?;
    out.push_str(text);
    Ok(out)
}

/// One row per verdict, in verdict order.
pub(crate) fn rows(verdicts: &[Verdict], naming: Naming) -> Result<Vec<String>, Refusal> {
    let mut out = reserved(verdicts.len())?;
    for verdict in verdicts {
        out.push(row(verdict, naming)?);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]
mod tests {
    use super::*;
    use crate::score::tests::note;
    use alloc::vec;
    use score_model::{IngestedScore, MeterChange, TempoChange};

    /// Source PPQ 3360 at 120 BPM: one quarter is 24,000 samples. Notes at
    /// quarters 0..8, pitches 60, 62, 64, ...
    fn score() -> LawScore {
        let notes = (0..8u64)
            .map(|q| note(q * 3_360, 60 + 2 * q as u8, q * 3_360 + 1_680))
            .collect();
        LawScore::from_ingested(&IngestedScore {
            source_ppq: 3_360,
            tempo: vec![TempoChange {
                tick: 0,
                us_per_quarter: 500_000,
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

    fn played(s: &LawScore, id: u32, delta: i64, pitch_offset: u8) -> TakeNote {
        let n = s.note(ScoreNoteId(id)).unwrap();
        TakeNote {
            onset_sample: (n.onset_sample as i64 + delta) as u64,
            pitch: n.pitch + pitch_offset,
            velocity: 90,
            cites: Some(ScoreNoteId(id)),
        }
    }

    fn kind_of(v: &Verdict) -> CitedKind {
        match v {
            Verdict::Cited { kind, .. } => *kind,
            other => panic!("not a cited verdict: {other:?}"),
        }
    }

    #[test]
    fn the_gate_is_inclusive_at_both_edges() {
        let s = score();
        let cases = [
            (1_920, CitedKind::Match),
            (1_921, CitedKind::Late),
            (-1_920, CitedKind::Match),
            (-1_921, CitedKind::Early),
            (0, CitedKind::Match),
        ];
        for (delta, expected) in cases {
            let take = [played(&s, 3, delta, 0)];
            let v = verdicts(&s, &take).unwrap();
            let cited = v
                .iter()
                .find(|v| matches!(v, Verdict::Cited { .. }))
                .unwrap();
            assert_eq!(kind_of(cited), expected, "delta {delta}");
        }
    }

    #[test]
    fn rows_state_the_comparison_in_digits() {
        let s = score();
        let row_for = |delta: i64, pitch_offset: u8| {
            let v = verdicts(&s, &[played(&s, 2, delta, pitch_offset)]).unwrap();
            let cited = v
                .into_iter()
                .find(|v| matches!(v, Verdict::Cited { .. }))
                .unwrap();
            row(&cited, Naming::Index).unwrap()
        };
        assert_eq!(
            row_for(2_160, 0),
            "note 2: onset +2160 samples (+45.0 ms) vs gate \u{b1}1920, pitch 64 vs 64: late"
        );
        assert_eq!(
            row_for(1_920, 0),
            "note 2: onset +1920 samples (+40.0 ms) vs gate \u{b1}1920, pitch 64 vs 64: match"
        );
        assert_eq!(
            row_for(1_921, 0),
            "note 2: onset +1921 samples (+40.0 ms) vs gate \u{b1}1920, pitch 64 vs 64: late"
        );
        assert_eq!(
            row_for(-1_921, 0),
            "note 2: onset -1921 samples (-40.0 ms) vs gate \u{b1}1920, pitch 64 vs 64: early"
        );
        assert_eq!(
            row_for(0, 0),
            "note 2: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 64 vs 64: match"
        );
        assert_eq!(
            row_for(2_880, 1),
            "note 2: onset +2880 samples (+60.0 ms) vs gate \u{b1}1920, pitch 65 vs 64: \
             wrong pitch"
        );
    }

    #[test]
    fn milliseconds_round_half_away_from_zero_in_integers() {
        // (samples, tenths of a millisecond)
        let cases = [
            (0, 0),
            (1, 0),   // 0.208
            (2, 0),   // 0.417
            (3, 1),   // 0.625
            (11, 2),  // 2.29
            (12, 3),  // 2.5, a half, away from zero
            (23, 5),  // 4.79
            (24, 5),  // 5, exact
            (36, 8),  // 7.5, a half
            (48, 10), // one millisecond
            (1_440, 300),
            (2_160, 450),
            (2_880, 600),
            (u64::MAX, 3_843_071_682_022_823_253), // 1.8e19 × 10 / 48, rounded
        ];
        for (samples, tenths) in cases {
            assert_eq!(tenths_of_ms(samples), Ok(tenths), "{samples} samples");
        }
        assert_eq!(
            u128::from(u64::MAX) * 10 / 48,
            3_843_071_682_022_823_253,
            "the last case, checked with plain u128 arithmetic"
        );
    }

    #[test]
    fn a_wrong_pitch_is_the_verdict_whatever_its_timing() {
        let s = score();
        for delta in [0, 2_160, -2_160] {
            let v = verdicts(&s, &[played(&s, 5, delta, 2)]).unwrap();
            assert_eq!(kind_of(&v[5]), CitedKind::WrongPitch, "delta {delta}");
        }
    }

    #[test]
    fn every_note_gets_one_verdict_in_a_fixed_order() {
        let s = score();
        let addition = TakeNote {
            onset_sample: 10_000,
            pitch: 90,
            velocity: 40,
            cites: None,
        };
        // Note 0 on time, the addition, note 1 twice (the second a re-strike
        // 100 samples later), note 3 late. Notes 2 and 4..7 are never played.
        let take = [
            played(&s, 0, 0, 0),
            played(&s, 1, 0, 0),
            played(&s, 1, 100, 0),
            addition,
            played(&s, 3, 2_500, 0),
        ];
        let mut sorted = take;
        sorted.sort_by_key(TakeNote::key);
        assert_eq!(sorted[1], addition, "10,000 falls between notes 0 and 1");
        let v = verdicts(&s, &sorted).unwrap();
        let summary: vec::Vec<(&str, Option<u32>)> = v
            .iter()
            .map(|v| {
                let note = match v {
                    Verdict::Cited { note, .. } | Verdict::NeverPlayed { note, .. } => Some(note.0),
                    Verdict::Addition { .. } => None,
                };
                (v.word(), note)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("match", Some(0)),
                ("match", Some(1)),
                ("match", Some(1)),
                ("never played", Some(2)),
                ("late", Some(3)),
                ("never played", Some(4)),
                ("never played", Some(5)),
                ("never played", Some(6)),
                ("never played", Some(7)),
                ("addition", None),
            ]
        );
        assert_eq!(
            row(&v[9], Naming::Index).unwrap(),
            "take note 1: onset 10000 samples, pitch 90, cites no score note: addition"
        );
        assert_eq!(
            row(&v[3], Naming::Index).unwrap(),
            "note 2: onset 48000 samples, pitch 64, no take note cites it: never played"
        );
        assert_eq!(v.iter().map(Verdict::kind_code).max(), Some(5));
    }

    /// The slice-1 pattern: three notes late by 30, 45 and 60 ms, one early by
    /// 45 ms, one wrong pitch. 30 ms is 1,440 samples, inside the 1,920-sample
    /// gate, so that note grades as a match; the row still says +30.0 ms.
    #[test]
    fn the_constructed_take_pattern_grades_as_designed() {
        let s = score();
        let mut take = vec::Vec::new();
        for id in 0..8 {
            let (delta, pitch) = match id {
                1 => (1_440, 0),
                2 => (2_160, 0),
                4 => (2_880, 0),
                5 => (-2_160, 0),
                6 => (0, 1),
                _ => (0, 0),
            };
            take.push(played(&s, id, delta, pitch));
        }
        let v = verdicts(&s, &take).unwrap();
        let words: vec::Vec<&str> = v.iter().map(Verdict::word).collect();
        assert_eq!(
            words,
            [
                "match",
                "match",
                "late",
                "match",
                "late",
                "early",
                "wrong pitch",
                "match"
            ]
        );
        assert_eq!(
            row(&v[1], Naming::Index).unwrap(),
            "note 1: onset +1440 samples (+30.0 ms) vs gate \u{b1}1920, pitch 62 vs 62: match"
        );
        assert_eq!(
            row(&v[5], Naming::Index).unwrap(),
            "note 5: onset -2160 samples (-45.0 ms) vs gate \u{b1}1920, pitch 70 vs 70: early"
        );
    }

    #[test]
    fn a_citation_past_the_score_is_refused_not_dropped() {
        let s = score();
        let stray = TakeNote {
            onset_sample: 0,
            pitch: 60,
            velocity: 1,
            cites: Some(ScoreNoteId(8)),
        };
        assert_eq!(
            verdicts(&s, &[stray]),
            Err(Refusal::TakeCitation {
                index: 0,
                cites: 8,
                notes: 8
            })
        );
    }
}
