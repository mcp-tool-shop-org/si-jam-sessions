//! The committed frames: what a host plays, read out of the law for a window
//! of quanta ([`crate::Law::frames`]).
//!
//! A frame window is a range of whole quanta, `first..=last`, and it holds
//! every event whose onset falls in one of them: every note-on of the score,
//! of the take and of the live take, and every beat of the score's meter. An
//! event belongs to exactly one quantum, `onset_sample / Q`, so consecutive
//! windows that share no quantum and leave none out hand a host every event
//! exactly once. Only committed quanta can be read, so what a window holds for
//! the score and the take never changes; a live note, which is a record, can
//! still arrive in a quantum a host has already read.
//!
//! Reading frames changes nothing in the law, and nothing here is hashed: the
//! frames are derived from the score, the tempo map and the take, which the
//! snapshot holds, and from the lengths of live notes, which it does not (see
//! [`crate::Law::live`]).

use alloc::vec::Vec;

use crate::refusal::Refusal;
use crate::score::LawScore;
use crate::take::{ScoreNoteId, TakeKey, TakeNote};
use crate::{MAX_SAMPLE, PPQ, QUANTUM_SAMPLES};

/// Which voice a frame's note belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Voice {
    /// A score note.
    Score,
    /// A take note admitted as a take ([`crate::Law::admit`]).
    Take,
    /// A take note admitted by the live verb ([`crate::Law::live`]).
    Live,
}

impl Voice {
    /// The voice as the frame layout writes it: score 0, take 1, live 2.
    pub fn code(self) -> u8 {
        match self {
            Voice::Score => 0,
            Voice::Take => 1,
            Voice::Live => 2,
        }
    }
}

/// One committed note-on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameNote {
    /// The onset on the law's sample clock.
    pub onset_sample: u64,
    pub voice: Voice,
    /// The score note: for a score frame the note itself, for a take or live
    /// frame the score note it cites; `None` for an addition.
    pub note: Option<ScoreNoteId>,
    pub pitch: u8,
    pub velocity: u8,
    /// How long the note sounds, in samples. A score note's is its own. A take
    /// note's is the score note's it cites, and an addition admitted as a take
    /// has none, so 0. A live note's is the length the host passed.
    pub duration_samples: u64,
}

impl FrameNote {
    /// The frame order: onset, then voice, then pitch, then an addition before
    /// a citation, then the score note's id. No two frame notes share it.
    pub fn order(&self) -> (u64, Voice, u8, Option<ScoreNoteId>) {
        (self.onset_sample, self.voice, self.pitch, self.note)
    }
}

/// One beat of the score's meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beat {
    /// The beat's position on the law's sample clock, through the tempo map.
    pub onset_sample: u64,
    /// The bar, counted from 0 at tick 0.
    pub bar: u64,
    /// The beat within its bar, counted from 0.
    pub beat: u8,
    /// True on beat 0 of a bar.
    pub downbeat: bool,
}

/// Everything committed in the quanta `first_quantum..=last_quantum`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frames {
    pub first_quantum: u64,
    pub last_quantum: u64,
    /// Every note-on in the window, in [`FrameNote::order`].
    pub notes: Vec<FrameNote>,
    /// Every beat in the window, by onset.
    pub beats: Vec<Beat>,
}

/// A whole note in law ticks: four quarters.
const WHOLE_NOTE_TICKS: u64 = 13_440;

// A whole note is 2^7 × 105 ticks, so every beat unit a meter can name (a
// denominator up to 2^6, as score-model validates) is a whole number of ticks.
#[allow(clippy::arithmetic_side_effects, clippy::as_conversions)]
const _: () = {
    assert!(WHOLE_NOTE_TICKS == 4 * PPQ as u64);
    assert!(WHOLE_NOTE_TICKS.is_multiple_of(1 << 7));
};

/// The length of a live note, which the take does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LiveLength {
    pub(crate) key: TakeKey,
    pub(crate) duration_samples: u64,
}

/// The window's samples, `first × Q ..= (last + 1) × Q - 1`, cut to the law's
/// last sample; `None` when the window starts past it, where no event can be.
fn samples(first: u64, last: u64) -> Result<Option<(u64, u64)>, Refusal> {
    let q = u128::from(QUANTUM_SAMPLES);
    let start = u128::from(first).checked_mul(q).ok_or(Refusal::Overflow)?;
    let end = u128::from(last)
        .checked_add(1)
        .and_then(|n| n.checked_mul(q))
        .and_then(|n| n.checked_sub(1))
        .ok_or(Refusal::Overflow)?;
    let max = u128::from(MAX_SAMPLE);
    if start > max {
        return Ok(None);
    }
    let start = u64::try_from(start).map_err(|_| Refusal::Overflow)?;
    let end = u64::try_from(end.min(max)).map_err(|_| Refusal::Overflow)?;
    Ok(Some((start, end)))
}

/// Collects the frames of `first..=last`. The caller has checked the window
/// against the committed horizon.
pub(crate) fn collect(
    score: &LawScore,
    take: &[TakeNote],
    live: &[LiveLength],
    first: u64,
    last: u64,
) -> Result<Frames, Refusal> {
    let mut frames = Frames {
        first_quantum: first,
        last_quantum: last,
        notes: Vec::new(),
        beats: Vec::new(),
    };
    let Some((start, end)) = samples(first, last)? else {
        return Ok(frames);
    };

    let notes = score.notes();
    let from = notes.partition_point(|n| n.onset_sample < start);
    let to = notes.partition_point(|n| n.onset_sample <= end);
    let played_from = take.partition_point(|t| t.onset_sample < start);
    let played_to = take.partition_point(|t| t.onset_sample <= end);
    let count = to
        .saturating_sub(from)
        .checked_add(played_to.saturating_sub(played_from))
        .ok_or(Refusal::Overflow)?;
    frames
        .notes
        .try_reserve_exact(count)
        .map_err(|_| Refusal::OutOfMemory)?;

    for index in from..to {
        let n = notes.get(index).ok_or(Refusal::Overflow)?;
        let id = u32::try_from(index).map_err(|_| Refusal::TooManyNotes { count: notes.len() })?;
        frames.notes.push(FrameNote {
            onset_sample: n.onset_sample,
            voice: Voice::Score,
            note: Some(ScoreNoteId(id)),
            pitch: n.pitch,
            velocity: n.velocity,
            duration_samples: n.duration_samples,
        });
    }
    for t in take.get(played_from..played_to).ok_or(Refusal::Overflow)? {
        let key = t.key();
        let (voice, duration_samples) = match live.binary_search_by(|l| l.key.cmp(&key)) {
            Ok(i) => (
                Voice::Live,
                live.get(i).ok_or(Refusal::Overflow)?.duration_samples,
            ),
            Err(_) => {
                let length = match t.cites {
                    Some(id) => score.note(id).ok_or(Refusal::Overflow)?.duration_samples,
                    None => 0,
                };
                (Voice::Take, length)
            }
        };
        frames.notes.push(FrameNote {
            onset_sample: t.onset_sample,
            voice,
            note: t.cites,
            pitch: t.pitch,
            velocity: t.velocity,
            duration_samples,
        });
    }
    frames.notes.sort_unstable_by_key(FrameNote::order);

    beats(score, start, end, &mut frames.beats)?;
    Ok(frames)
}

/// The sample of a beat's tick, or `None` when it is past the law's last
/// sample, where no window reaches.
fn beat_sample(score: &LawScore, tick: u64) -> Result<Option<u64>, Refusal> {
    match score.tempo_map().sample_at(tick) {
        Ok(sample) if sample <= MAX_SAMPLE => Ok(Some(sample)),
        Ok(_) | Err(Refusal::SampleOverflow { .. }) => Ok(None),
        Err(other) => Err(other),
    }
}

/// Every beat whose sample is in `start..=end`, by onset.
///
/// A beat is one unit of the meter's denominator, and a bar is `numerator`
/// beats: 2/4 has two quarter-note beats a bar, 6/8 six eighth-note beats.
/// Bars count from 0 at tick 0. A meter change starts a new bar at its tick,
/// so a bar it cuts short is still a bar, and its beats are counted up to the
/// change.
fn beats(score: &LawScore, start: u64, end: u64, out: &mut Vec<Beat>) -> Result<(), Refusal> {
    let meter = score.meter();
    let mut bars_before: u64 = 0;
    for (k, m) in meter.iter().enumerate() {
        let beat_ticks = WHOLE_NOTE_TICKS
            .checked_shr(u32::from(m.denominator_pow2))
            .filter(|&b| {
                b > 0 && b.checked_shl(u32::from(m.denominator_pow2)) == Some(WHOLE_NOTE_TICKS)
            })
            .ok_or(Refusal::Overflow)?;
        let per_bar = u64::from(m.numerator);
        if per_bar == 0 {
            return Err(Refusal::Overflow);
        }
        // How many beats the segment holds: up to the next change, or every
        // beat whose tick fits a u64 for the last segment.
        let beats_here = match meter.get(k.checked_add(1).ok_or(Refusal::Overflow)?) {
            Some(next) => next
                .tick
                .checked_sub(m.tick)
                .ok_or(Refusal::Overflow)?
                .div_ceil(beat_ticks),
            None => u64::MAX
                .checked_sub(m.tick)
                .and_then(|room| room.checked_div(beat_ticks))
                .and_then(|n| n.checked_add(1))
                .ok_or(Refusal::Overflow)?,
        };
        let tick_of = |j: u64| -> Result<u64, Refusal> {
            j.checked_mul(beat_ticks)
                .and_then(|t| t.checked_add(m.tick))
                .ok_or(Refusal::Overflow)
        };

        // The segment starts past the window: so does every later one.
        match beat_sample(score, m.tick)? {
            Some(first) if first <= end => {}
            _ => return Ok(()),
        }
        // The first beat at or after the window's start. Positions never
        // decrease with the tick, so the search is a lower bound.
        let (mut lo, mut hi) = (0u64, beats_here);
        while lo < hi {
            let mid = hi
                .checked_sub(lo)
                .and_then(|span| span.checked_div(2))
                .and_then(|half| lo.checked_add(half))
                .ok_or(Refusal::Overflow)?;
            match beat_sample(score, tick_of(mid)?)? {
                Some(sample) if sample < start => {
                    lo = mid.checked_add(1).ok_or(Refusal::Overflow)?
                }
                _ => hi = mid,
            }
        }
        let mut j = lo;
        while j < beats_here {
            let Some(sample) = beat_sample(score, tick_of(j)?)? else {
                break;
            };
            if sample > end {
                break;
            }
            let beat = j.checked_rem(per_bar).ok_or(Refusal::Overflow)?;
            let bar = j
                .checked_div(per_bar)
                .and_then(|b| b.checked_add(bars_before))
                .ok_or(Refusal::Overflow)?;
            out.try_reserve(1).map_err(|_| Refusal::OutOfMemory)?;
            out.push(Beat {
                onset_sample: sample,
                bar,
                beat: u8::try_from(beat).map_err(|_| Refusal::Overflow)?,
                downbeat: beat == 0,
            });
            j = j.checked_add(1).ok_or(Refusal::Overflow)?;
        }
        bars_before = bars_before
            .checked_add(beats_here.div_ceil(per_bar))
            .ok_or(Refusal::Overflow)?;
    }
    Ok(())
}
