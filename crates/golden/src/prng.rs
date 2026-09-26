//! SplitMix64, the pseudorandom generator that places the constructed take's
//! perturbations.
//!
//! The generator is Steele, Lea and Flood's SplitMix ("Fast splittable
//! pseudorandom number generators", OOPSLA 2014) with the 64-bit finaliser of
//! Vigna's reference `splitmix64.c`: add the golden-ratio increment, then two
//! xor-shift-multiply rounds and a final xor-shift.
//!
//! Why this one:
//! - its whole state is one `u64`, and every step is wrapping integer
//!   arithmetic, so it gives the same numbers on every host and needs no
//!   floating point;
//! - it has published test vectors, which the tests below check;
//! - the draw needs only to be reproducible from a seed and not picked by
//!   hand. It is not a cryptographic generator and does not need to be.

/// SplitMix64's state and its step.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform integer in `0..n`, or `None` when `n` is 0.
    ///
    /// Rejection sampling, as in OpenBSD's `arc4random_uniform`: an output
    /// below `2^64 mod n` is drawn again, so the outputs kept span a whole
    /// number of copies of `0..n` and their remainder mod `n` is unbiased. A
    /// plain `x % n` would favour the low values.
    pub fn below(&mut self, n: u64) -> Option<u64> {
        if n == 0 {
            return None;
        }
        // 2^64 mod n, computed in u64: (2^64 - n) mod n.
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.next_u64();
            if x >= threshold {
                return Some(x % n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published vectors for seed 1234567 (as in Rosetta Code's
    /// "Pseudo-random numbers/Splitmix64"), which a Python big-integer
    /// implementation of the reference reproduces.
    #[test]
    fn the_published_vectors() {
        let mut g = SplitMix64::new(1_234_567);
        let got: Vec<u64> = (0..5).map(|_| g.next_u64()).collect();
        assert_eq!(
            got,
            [
                6_457_827_717_110_365_317,
                3_203_168_211_198_807_973,
                9_817_491_932_198_370_423,
                4_593_380_528_125_082_431,
                16_408_922_859_458_223_821,
            ]
        );
    }

    /// The first outputs for the take's seed, from the same Python
    /// implementation.
    #[test]
    fn the_take_seed_starts_where_the_reference_does() {
        let mut g = SplitMix64::new(crate::take::SEED);
        let got: Vec<u64> = (0..3).map(|_| g.next_u64()).collect();
        assert_eq!(
            got,
            [
                13_915_708_071_972_804_707,
                4_955_225_999_451_175_660,
                12_693_192_435_307_978_464,
            ]
        );
    }

    #[test]
    fn below_stays_in_range_and_refuses_zero() {
        let mut g = SplitMix64::new(7);
        assert_eq!(g.below(0), None);
        for _ in 0..1_000 {
            assert_eq!(g.below(1), Some(0));
            assert!(g.below(2_621).is_some_and(|x| x < 2_621));
            assert!(g.below(u64::MAX).is_some_and(|x| x < u64::MAX));
        }
    }

    /// The threshold is 2^64 mod n, checked against u128 arithmetic.
    #[test]
    fn the_rejection_threshold_is_two_to_the_64_mod_n() {
        for n in [1u64, 2, 3, 7, 2_621, 1 << 40, u64::MAX - 1, u64::MAX] {
            let wide = (1u128 << 64) % u128::from(n);
            assert_eq!(u128::from(n.wrapping_neg() % n), wide, "n = {n}");
        }
        assert_eq!((1u128 << 64) % 2_621, 1_536);
    }

    /// Below the threshold a draw is redrawn: with n = 2^63 + 1 the threshold
    /// is 2^63 - 1, so about half of all outputs are rejected, and every value
    /// kept is still in range.
    #[test]
    fn rejected_draws_are_redrawn() {
        let n = (1u64 << 63) + 1;
        assert_eq!(n.wrapping_neg() % n, (1u64 << 63) - 1);
        let mut kept = SplitMix64::new(99);
        let mut raw = SplitMix64::new(99);
        let mut redrawn = 0;
        for _ in 0..200 {
            let value = kept.below(n).unwrap();
            assert!(value < n);
            loop {
                let x = raw.next_u64();
                if x >= (1u64 << 63) - 1 {
                    assert_eq!(value, x % n);
                    break;
                }
                redrawn += 1;
            }
        }
        assert!(redrawn > 50, "only {redrawn} redraws");
    }
}
