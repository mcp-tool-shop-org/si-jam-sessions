//! Integer musical time.
//!
//! Two conversions, both exact or refused:
//!
//! - [`rescale_tick`]: a source file's tick into law ticks at [`PPQ`].
//! - [`TempoMap::sample_at`]: a law tick into a sample position at
//!   [`SAMPLE_RATE`], through the tempo map.
//!
//! No float is involved; every intermediate is a `u128`.

use alloc::vec::Vec;

use score_model::{MAX_US_PER_QUARTER, ModelError};

use crate::refusal::{Event, Refusal};
use crate::{PPQ, SAMPLE_RATE};

/// `D = PPQ × 10⁶ = 3,360,000,000`, the denominator of every tick-to-sample
/// rational.
// Widening, and evaluated at compile time, where an overflow fails the build.
#[allow(clippy::as_conversions, clippy::arithmetic_side_effects)]
const DENOMINATOR: u128 = PPQ as u128 * 1_000_000;

/// A source-file tick in law ticks: `tick × 3360 / source_ppq`.
///
/// Exact or refused:
/// - [`Refusal::InexactTick`] when `tick × 3360` is not divisible by
///   `source_ppq`, so the tick has no whole law tick;
/// - [`Refusal::TickOverflow`] when the law tick does not fit a `u64`;
/// - [`Refusal::Model`] with `ZeroPpq` when `source_ppq` is 0.
///
/// `event` names the tick in the refusal. The product is taken in `u128`, where
/// a `u64` tick times 3,360 cannot overflow.
pub fn rescale_tick(tick: u64, source_ppq: u16, event: Event) -> Result<u64, Refusal> {
    let zero_ppq = Refusal::Model(ModelError::ZeroPpq);
    let ppq = u128::from(source_ppq);
    let scaled = u128::from(tick)
        .checked_mul(u128::from(PPQ))
        .ok_or(Refusal::TickOverflow { event, tick })?;
    if scaled.checked_rem(ppq).ok_or(zero_ppq)? != 0 {
        return Err(Refusal::InexactTick {
            event,
            tick,
            source_ppq,
        });
    }
    let law_tick = scaled.checked_div(ppq).ok_or(zero_ppq)?;
    u64::try_from(law_tick).map_err(|_| Refusal::TickOverflow { event, tick })
}

/// The tempo map in law ticks, with the exact sample position of every tempo
/// change.
///
/// # The conversion
///
/// Write `R` for [`SAMPLE_RATE`] and `D = PPQ × 10⁶`. Tempo change `k` sets
/// `u_k` microseconds per quarter from law tick `t_k` on, with `t_0 = 0`. One
/// tick of segment `k` lasts `u_k / PPQ` microseconds, which is
/// `u_k × R / D` samples. The exact position of tick `t`, in samples, is the
/// rational `N(t) / D`, where
///
/// ```text
/// N(t) = Σ_k  max(0, min(t, t_(k+1)) - t_k) × u_k × R
/// ```
///
/// an exact integer. The law's sample position is `floor(N(t) / D)`: **the
/// floor of the exact rational sum, never a sum of floors.** The two differ:
/// `floor(a/D) + floor(b/D)` can be less than `floor((a + b)/D)`, by up to one
/// sample per tempo change, and that loss would accumulate over a piece.
///
/// # How it stays exact
///
/// Each segment stores the whole part and the remainder of its start:
/// `start_sample_k × D + start_remainder_k = N(t_k)`, with
/// `0 <= start_remainder_k < D`. For `t` in `[t_k, t_(k+1))`,
/// `N(t) = N(t_k) + (t - t_k) × u_k × R`, so
///
/// ```text
/// floor(N(t) / D) = start_sample_k + floor((start_remainder_k + (t - t_k) × u_k × R) / D)
/// ```
///
/// because `floor((s × D + x) / D) = s + floor(x / D)` for any `x >= 0`. The
/// next segment's whole part and remainder come from the same division at
/// `t = t_(k+1)`, so the invariant holds for every segment by induction: the
/// remainder is carried across each tempo change instead of dropped.
///
/// The numerator is at most `(2⁶⁴ - 1) × (2²⁴ - 1) × 48,000 + D`, below 2¹⁰⁴,
/// so it never overflows a `u128`. A position that does not fit a `u64` is
/// refused with [`Refusal::SampleOverflow`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempoMap {
    segments: Vec<Segment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Segment {
    /// The segment's first law tick, `t_k`.
    tick: u64,
    /// `u_k`, microseconds per quarter note from `tick` on.
    us_per_quarter: u32,
    /// `floor(N(t_k) / D)`.
    start_sample: u64,
    /// `N(t_k) mod D`, carried into every position inside the segment.
    start_remainder: u64,
}

impl Segment {
    /// The exact position of `tick` (at or after this segment's start) under
    /// this segment's tempo, as `(floor(N / D), N mod D)`.
    fn position(&self, tick: u64) -> Result<(u64, u64), Refusal> {
        let overflow = Refusal::SampleOverflow { tick };
        let elapsed = tick.checked_sub(self.tick).ok_or(Refusal::Overflow)?;
        let numerator = u128::from(elapsed)
            .checked_mul(u128::from(self.us_per_quarter))
            .and_then(|n| n.checked_mul(u128::from(SAMPLE_RATE)))
            .and_then(|n| n.checked_add(u128::from(self.start_remainder)))
            .ok_or(overflow)?;
        let whole = numerator
            .checked_div(DENOMINATOR)
            .ok_or(Refusal::Overflow)?;
        let remainder = numerator
            .checked_rem(DENOMINATOR)
            .ok_or(Refusal::Overflow)?;
        let sample = u64::try_from(whole)
            .ok()
            .and_then(|w| self.start_sample.checked_add(w))
            .ok_or(overflow)?;
        let remainder = u64::try_from(remainder).map_err(|_| Refusal::Overflow)?;
        Ok((sample, remainder))
    }
}

impl TempoMap {
    /// Builds the map from `(law tick, microseconds per quarter)` pairs.
    ///
    /// The first change must be at tick 0, ticks must strictly increase, and
    /// each tempo must be in `1..=MAX_US_PER_QUARTER`; otherwise the matching
    /// `ModelError` is refused, as `IngestedScore::validate` would.
    pub fn new(changes: &[(u64, u32)]) -> Result<Self, Refusal> {
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(changes.len())
            .map_err(|_| Refusal::OutOfMemory)?;
        let mut previous: Option<Segment> = None;
        for (index, &(tick, us_per_quarter)) in changes.iter().enumerate() {
            if us_per_quarter == 0 || us_per_quarter > MAX_US_PER_QUARTER {
                return Err(Refusal::Model(ModelError::TempoOutOfRange { index }));
            }
            let (start_sample, start_remainder) = match previous {
                None if tick == 0 => (0, 0),
                None => return Err(Refusal::Model(ModelError::NoTempoAtZero)),
                Some(prev) if tick <= prev.tick => {
                    return Err(Refusal::Model(ModelError::TempoNotSorted { index }));
                }
                Some(prev) => prev.position(tick)?,
            };
            let segment = Segment {
                tick,
                us_per_quarter,
                start_sample,
                start_remainder,
            };
            segments.push(segment);
            previous = Some(segment);
        }
        if segments.is_empty() {
            return Err(Refusal::Model(ModelError::NoTempoAtZero));
        }
        Ok(TempoMap { segments })
    }

    /// The sample position of law tick `tick`: `floor(N(tick) / D)`, exact as
    /// the type documentation proves, or [`Refusal::SampleOverflow`].
    pub fn sample_at(&self, tick: u64) -> Result<u64, Refusal> {
        // The first segment starts at tick 0, so at least one segment is at or
        // before any tick.
        let starts_at_or_before = self.segments.partition_point(|s| s.tick <= tick);
        let index = starts_at_or_before
            .checked_sub(1)
            .ok_or(Refusal::Model(ModelError::NoTempoAtZero))?;
        let segment = self.segments.get(index).ok_or(Refusal::Overflow)?;
        segment.position(tick).map(|(sample, _)| sample)
    }

    /// Each tempo change as `(law tick, microseconds per quarter, sample
    /// position)`, in tick order.
    pub fn changes(&self) -> impl Iterator<Item = (u64, u32, u64)> + '_ {
        self.segments
            .iter()
            .map(|s| (s.tick, s.us_per_quarter, s.start_sample))
    }
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
mod tests {
    use super::*;
    use alloc::vec;

    const R: u128 = SAMPLE_RATE as u128;

    // --- Tuplets at PPQ 3360 -------------------------------------------------

    /// Note values from a whole note down to a 128th, in law ticks.
    const LEVELS: [(&str, u64); 8] = [
        ("whole", 13_440),
        ("half", 6_720),
        ("quarter", 3_360),
        ("eighth", 1_680),
        ("16th", 840),
        ("32nd", 420),
        ("64th", 210),
        ("128th", 105),
    ];

    #[test]
    fn three_five_and_seven_tuplets_are_exact_at_every_level_down_to_a_128th() {
        for (name, ticks) in LEVELS {
            for n in [3, 5, 7] {
                assert_eq!(ticks % n, 0, "a {name} ({ticks} ticks) into {n} parts");
            }
        }
        // The tuplet note itself, m in the time of n: ticks × m / n.
        let tuplet = |level: u64, n: u64, in_time_of: u64| {
            assert_eq!(level * in_time_of % n, 0);
            level * in_time_of / n
        };
        assert_eq!(tuplet(1_680, 3, 2), 1_120, "eighth-note triplet");
        assert_eq!(tuplet(840, 5, 4), 672, "16th-note quintuplet");
        assert_eq!(tuplet(420, 7, 4), 240, "32nd-note septuplet, 7:4");
        assert_eq!(tuplet(420, 7, 8), 480, "32nd-note septuplet, 7:8");
        assert_eq!(tuplet(105, 3, 2), 70, "128th-note triplet");
        assert_eq!(tuplet(105, 5, 4), 84, "128th-note quintuplet");
        assert_eq!(tuplet(105, 7, 4), 60, "128th-note septuplet");
    }

    /// Which equal divisions, 2 to 16, of each level are whole ticks at PPQ
    /// 3360. 3, 5 and 7 are exact everywhere; halving stops at the 128th
    /// (a 256th would be 52.5 ticks); 9, 11 and 13 are never exact.
    #[test]
    fn which_subdivisions_of_ppq_3360_are_exact() {
        let expected: [(&str, &[u64]); 8] = [
            ("whole", &[2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15, 16]),
            ("half", &[2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15, 16]),
            ("quarter", &[2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15, 16]),
            ("eighth", &[2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15, 16]),
            ("16th", &[2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15]),
            ("32nd", &[2, 3, 4, 5, 6, 7, 10, 12, 14, 15]),
            ("64th", &[2, 3, 5, 6, 7, 10, 14, 15]),
            ("128th", &[3, 5, 7, 15]),
        ];
        for ((name, ticks), (expected_name, exact)) in LEVELS.iter().zip(expected) {
            assert_eq!(*name, expected_name);
            let measured: vec::Vec<u64> = (2..=16).filter(|n| ticks % n == 0).collect();
            assert_eq!(measured, exact, "exact divisions of a {name}");
        }
        assert_eq!(u64::from(PPQ) % 32, 0);
        assert_ne!(105 % 2, 0, "a 128th cannot be halved");
    }

    /// The tuplet table at PPQ 3360, in ticks: the note, then a 3:2, a 5:4
    /// and a 7:4 tuplet note of that value. The expected values are copied
    /// from rust-knowledge wave 5, integer-time lane (compiler-measured, then
    /// re-derived in Python bigint by a separate verifier); they are not
    /// computed here.
    #[test]
    fn the_tuplet_table_matches_the_knowledge_base() {
        let table: [(&str, [u64; 4]); 8] = [
            ("whole", [13_440, 8_960, 10_752, 7_680]),
            ("half", [6_720, 4_480, 5_376, 3_840]),
            ("quarter", [3_360, 2_240, 2_688, 1_920]),
            ("eighth", [1_680, 1_120, 1_344, 960]),
            ("16th", [840, 560, 672, 480]),
            ("32nd", [420, 280, 336, 240]),
            ("64th", [210, 140, 168, 120]),
            ("128th", [105, 70, 84, 60]),
        ];
        for ((name, ticks), (expected_name, [note, three, five, seven])) in LEVELS.iter().zip(table)
        {
            assert_eq!((*name, *ticks), (expected_name, note));
            // m in the time of n is ticks × m / n; each must divide exactly.
            for (n, m, expected) in [(3, 2, three), (5, 4, five), (7, 4, seven)] {
                assert_eq!(ticks * m % n, 0, "a {name} {n}:{m} tuplet is not whole");
                assert_eq!(ticks * m / n, expected, "a {name} {n}:{m} tuplet");
            }
        }
    }

    // --- Rescaling -----------------------------------------------------------

    #[test]
    fn rescaling_is_exact_or_refused() {
        let e = Event::NoteStart(0);
        assert_eq!(rescale_tick(3_360, 3_360, e), Ok(3_360));
        assert_eq!(rescale_tick(1, 480, e), Ok(7));
        assert_eq!(rescale_tick(4, 384, e), Ok(35));
        assert_eq!(rescale_tick(2, 960, e), Ok(7));
        assert_eq!(rescale_tick(1, 7, e), Ok(480));
        assert_eq!(rescale_tick(0, 65_535, e), Ok(0));
        // 385 × 3360 / 384 = 13,475 / 4.
        assert_eq!(
            rescale_tick(385, 384, e),
            Err(Refusal::InexactTick {
                event: e,
                tick: 385,
                source_ppq: 384
            })
        );
        // 1 × 3360 / 960 = 3.5.
        assert_eq!(
            rescale_tick(1, 960, Event::Meter(2)),
            Err(Refusal::InexactTick {
                event: Event::Meter(2),
                tick: 1,
                source_ppq: 960
            })
        );
        assert_eq!(
            rescale_tick(1, 0, e),
            Err(Refusal::Model(ModelError::ZeroPpq))
        );
    }

    #[test]
    fn rescaling_refuses_at_the_u64_boundary() {
        let e = Event::Tempo(1);
        let last = u64::MAX / 3_360;
        assert_eq!(rescale_tick(last, 1, e), Ok(last * 3_360));
        assert_eq!(
            rescale_tick(last + 1, 1, e),
            Err(Refusal::TickOverflow {
                event: e,
                tick: last + 1
            })
        );
        // A source PPQ above 3360 shrinks ticks, so u64::MAX itself rescales
        // when it divides exactly: 3360 / 6720 = 1/2.
        assert_eq!(rescale_tick(u64::MAX - 1, 6_720, e), Ok((u64::MAX - 1) / 2));
    }

    // --- Tick to sample ------------------------------------------------------

    fn map(changes: &[(u64, u32)]) -> TempoMap {
        TempoMap::new(changes).unwrap()
    }

    #[test]
    fn a_quarter_at_120_bpm_is_24000_samples() {
        let m = map(&[(0, 500_000)]);
        assert_eq!(m.sample_at(0), Ok(0));
        assert_eq!(m.sample_at(3_360), Ok(24_000));
        // 1 tick = 500,000 × 48,000 / 3,360,000,000 = 50/7 samples.
        assert_eq!(m.sample_at(1), Ok(7));
        assert_eq!(m.sample_at(7), Ok(50));
    }

    /// The remainder carried across a tempo change, against a hand-computed
    /// example.
    ///
    /// Segment 1, ticks 0..3361 at 500,000 us/quarter:
    /// 3361 × 500,000 × 48,000 / 3,360,000,000 = 168,050 / 7 = 24,007 + 1/7.
    ///
    /// Segment 2, ticks 3361..6723 at 450,000 us/quarter:
    /// 3362 × 450,000 × 48,000 / 3,360,000,000 = 151,290 / 7 = 21,612 + 6/7.
    ///
    /// Exact sum: 24,007 + 21,612 + 7/7 = 45,620. A sum of floors gives
    /// 24,007 + 21,612 = 45,619, one sample early.
    #[test]
    fn the_remainder_carries_across_a_tempo_change() {
        let m = map(&[(0, 500_000), (3_361, 450_000)]);
        assert_eq!(m.sample_at(3_361), Ok(24_007));
        assert_eq!(m.sample_at(6_723), Ok(45_620));

        let d = DENOMINATOR;
        let first = 3_361u128 * 500_000 * R;
        let second = 3_362u128 * 450_000 * R;
        assert_eq!((first / d, first % d), (24_007, d / 7));
        assert_eq!((second / d, second % d), (21_612, 6 * d / 7));
        let sum_of_floors = first / d + second / d;
        assert_eq!(sum_of_floors, 45_619);
        assert_eq!((first + second) / d, 45_620);

        let changes: vec::Vec<_> = m.changes().collect();
        assert_eq!(changes, [(0, 500_000, 0), (3_361, 450_000, 24_007)]);
    }

    /// Two one-tick segments at the slowest tempo. The expected values are
    /// copied from rust-knowledge wave 5, integer-time lane (compiler-measured,
    /// then re-derived in Python bigint by a separate verifier); they are not
    /// computed here. One tick at 0xFFFFFF us/quarter is 239.67... samples,
    /// so two ticks are 479.35... and the law's position is 479. Summing the
    /// two segments' floors independently gives 478: that is the drift value.
    #[test]
    fn two_one_tick_segments_at_the_slowest_tempo_carry_to_479() {
        let m = map(&[(0, 16_777_215), (1, 16_777_215)]);
        assert_eq!(m.sample_at(2), Ok(479));
        let drift = m.sample_at(1).unwrap() * 2;
        assert_eq!(
            drift, 478,
            "the per-segment floors, which the law must not sum"
        );
    }

    /// `N(t)` computed from scratch, segment by segment, with no carry.
    fn reference(changes: &[(u64, u32)], tick: u64) -> u128 {
        let mut n = 0u128;
        for (i, &(start, tempo)) in changes.iter().enumerate() {
            let end = changes.get(i + 1).map_or(u64::MAX, |c| c.0);
            if tick > start {
                let span = tick.min(end) - start;
                n += u128::from(span) * u128::from(tempo) * R;
            }
        }
        n / DENOMINATOR
    }

    #[test]
    fn every_position_is_the_floor_of_the_exact_rational() {
        let changes = [
            (0, 333_333),
            (1_001, 499_999),
            (2_000, 1),
            (2_003, MAX_US_PER_QUARTER),
            (2_010, 428_571),
            (9_999, 600_001),
        ];
        let m = map(&changes);
        for tick in 0..=12_000u64 {
            assert_eq!(
                u128::from(m.sample_at(tick).unwrap()),
                reference(&changes, tick),
                "tick {tick}"
            );
        }
        // Far from the origin, where floors lost per segment would show most.
        for tick in (u64::from(u32::MAX)..u64::from(u32::MAX) + 5_000).step_by(7) {
            assert_eq!(
                u128::from(m.sample_at(tick).unwrap()),
                reference(&changes, tick)
            );
        }
    }

    /// At the slowest SMF tempo, 0xFFFFFF microseconds per quarter, the last
    /// tick whose sample fits a `u64` converts, and the next one is refused.
    #[test]
    fn the_overflow_boundary_at_tempo_0xffffff() {
        let slow = MAX_US_PER_QUARTER;
        assert_eq!(slow, 0xFF_FFFF);
        let per_tick = u128::from(slow) * R;
        // sample(t) <= u64::MAX  <=>  t × u × R < 2^64 × D.
        let limit = (1u128 << 64) * DENOMINATOR;
        let last = u64::try_from((limit - 1) / per_tick).unwrap();
        let m = map(&[(0, slow)]);
        let expected = u64::try_from(u128::from(last) * per_tick / DENOMINATOR).unwrap();
        assert_eq!(m.sample_at(last), Ok(expected));
        assert!(expected > u64::MAX - 240_000, "within 5 s of the end");
        assert_eq!(
            m.sample_at(last + 1),
            Err(Refusal::SampleOverflow { tick: last + 1 })
        );
        assert_eq!(
            m.sample_at(u64::MAX),
            Err(Refusal::SampleOverflow { tick: u64::MAX })
        );
        // A tempo change past the boundary cannot even start.
        assert_eq!(
            TempoMap::new(&[(0, slow), (last + 1, 500_000)]),
            Err(Refusal::SampleOverflow { tick: last + 1 })
        );
        assert!(TempoMap::new(&[(0, slow), (last, 500_000)]).is_ok());
    }

    #[test]
    fn a_tempo_map_refuses_what_validate_refuses() {
        assert_eq!(
            TempoMap::new(&[]),
            Err(Refusal::Model(ModelError::NoTempoAtZero))
        );
        assert_eq!(
            TempoMap::new(&[(1, 500_000)]),
            Err(Refusal::Model(ModelError::NoTempoAtZero))
        );
        assert_eq!(
            TempoMap::new(&[(0, 500_000), (0, 400_000)]),
            Err(Refusal::Model(ModelError::TempoNotSorted { index: 1 }))
        );
        assert_eq!(
            TempoMap::new(&[(0, 0)]),
            Err(Refusal::Model(ModelError::TempoOutOfRange { index: 0 }))
        );
        assert_eq!(
            TempoMap::new(&[(0, MAX_US_PER_QUARTER + 1)]),
            Err(Refusal::Model(ModelError::TempoOutOfRange { index: 0 }))
        );
    }
}
