//! The constructed take of slice 1.
//!
//! PHASE-0 fixes its shape: "three notes late by 30, 45 and 60 ms, one early by
//! 45 ms, one wrong pitch, at positions drawn by the seed". Every other score
//! note is played once, on time, at its pitch and velocity, citing itself.
//!
//! # The draw
//!
//! Five distinct score notes are drawn with [`SplitMix64`] from [`SEED`], one
//! per perturbation, in the order of [`Perturbation::DRAW_ORDER`]. For each
//! perturbation, an index is drawn uniformly from `0..n` over the score's `n`
//! notes (in the law's canonical order, so an index is a score note id). It is
//! drawn again when:
//! - that note has already been drawn, or
//! - the perturbation does not fit the note. A late note moves forward, and
//!   fits while its onset stays at most the law's last sample. An early note
//!   moves back 2,160 samples, and a take onset is unsigned, so it fits only a
//!   score onset of at least 2,160 samples. A wrong pitch always fits: one
//!   semitone up, or down from 127.
//!
//! A perturbation that finds no note in [`MAX_DRAWS`] draws is refused rather
//! than looped on.

use law::{LawScore, MAX_SAMPLE, SAMPLES_PER_MS, ScoreNoteId, TakeNote};

use crate::Error;
use crate::prng::SplitMix64;

/// The seed of the constructed take: the ASCII bytes of `si-jam-1`, read
/// big-endian. A plain, readable value, fixed before the draw was first run,
/// so the notes it picks were not chosen by hand.
pub const SEED: u64 = u64::from_be_bytes(*b"si-jam-1");

/// Draws allowed for one perturbation before the draw refuses.
pub const MAX_DRAWS: u32 = 1_000_000;

/// One of the five ways the constructed take departs from the score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perturbation {
    /// 30 ms late: +1,440 samples. Inside the ±1,920-sample gate, so on time.
    Late30,
    /// 45 ms late: +2,160 samples.
    Late45,
    /// 60 ms late: +2,880 samples.
    Late60,
    /// 45 ms early: -2,160 samples.
    Early45,
    /// The wrong pitch, on time: one semitone up, or down from 127.
    WrongPitch,
}

impl Perturbation {
    /// The order the notes are drawn in.
    pub const DRAW_ORDER: [Perturbation; 5] = [
        Perturbation::Late30,
        Perturbation::Late45,
        Perturbation::Late60,
        Perturbation::Early45,
        Perturbation::WrongPitch,
    ];

    /// The onset offset in samples: milliseconds times 48.
    pub const fn delta_samples(self) -> i64 {
        match self {
            Perturbation::Late30 => 1_440,
            Perturbation::Late45 => 2_160,
            Perturbation::Late60 => 2_880,
            Perturbation::Early45 => -2_160,
            Perturbation::WrongPitch => 0,
        }
    }

    /// The perturbation's name in the golden file.
    pub const fn label(self) -> &'static str {
        match self {
            Perturbation::Late30 => "late-30ms",
            Perturbation::Late45 => "late-45ms",
            Perturbation::Late60 => "late-60ms",
            Perturbation::Early45 => "early-45ms",
            Perturbation::WrongPitch => "wrong-pitch",
        }
    }

    /// The onset this perturbation plays a note at, or `None` when it does not
    /// fit the note (see the module documentation).
    pub fn onset(self, score_onset: u64) -> Option<u64> {
        let moved = score_onset.checked_add_signed(self.delta_samples())?;
        (moved <= MAX_SAMPLE).then_some(moved)
    }

    /// The pitch this perturbation plays a note at.
    pub const fn pitch(self, score_pitch: u8) -> u8 {
        match self {
            Perturbation::WrongPitch if score_pitch < 127 => score_pitch + 1,
            Perturbation::WrongPitch => 126,
            _ => score_pitch,
        }
    }
}

// The offsets are the milliseconds PHASE-0 names, in whole samples at 48 kHz.
const _: () = {
    let ms = SAMPLES_PER_MS as i64;
    assert!(Perturbation::Late30.delta_samples() == 30 * ms);
    assert!(Perturbation::Late45.delta_samples() == 45 * ms);
    assert!(Perturbation::Late60.delta_samples() == 60 * ms);
    assert!(Perturbation::Early45.delta_samples() == -45 * ms);
};

/// A drawn note and the perturbation it carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    pub perturbation: Perturbation,
    pub note: ScoreNoteId,
}

/// Draws the five perturbed notes (see the module documentation).
pub fn draw(score: &LawScore, seed: u64) -> Result<[Drawn; 5], Error> {
    let notes = score.notes();
    let n = u64::try_from(notes.len()).map_err(|_| Error::new("too many score notes"))?;
    let mut rng = SplitMix64::new(seed);
    let mut drawn: Vec<Drawn> = Vec::with_capacity(5);
    for perturbation in Perturbation::DRAW_ORDER {
        let mut found = None;
        for _ in 0..MAX_DRAWS {
            let index = rng
                .below(n)
                .ok_or_else(|| Error::new("the score has no notes to draw"))?;
            let id =
                ScoreNoteId(u32::try_from(index).map_err(|_| Error::new("a note index past u32"))?);
            if drawn.iter().any(|d| d.note == id) {
                continue;
            }
            let note = score
                .note(id)
                .ok_or_else(|| Error::new("a drawn note is missing"))?;
            if perturbation.onset(note.onset_sample).is_some() {
                found = Some(Drawn {
                    perturbation,
                    note: id,
                });
                break;
            }
        }
        let drawn_note = found.ok_or_else(|| {
            Error::new(format!(
                "no note fits {} in {MAX_DRAWS} draws",
                perturbation.label()
            ))
        })?;
        drawn.push(drawn_note);
    }
    drawn
        .try_into()
        .map_err(|_| Error::new("the draw did not make five notes"))
}

/// The constructed take: every score note played once and citing itself, at
/// its velocity, on time and at its pitch, except the five drawn notes, which
/// carry their perturbation. The notes are in the take's total order
/// ([`TakeNote::key`]), which is how the law admits a batch.
pub fn construct(score: &LawScore, drawn: &[Drawn; 5]) -> Result<Vec<TakeNote>, Error> {
    let mut take = Vec::with_capacity(score.notes().len());
    for (index, note) in score.notes().iter().enumerate() {
        let id =
            ScoreNoteId(u32::try_from(index).map_err(|_| Error::new("a note index past u32"))?);
        let perturbation = drawn.iter().find(|d| d.note == id).map(|d| d.perturbation);
        let (onset_sample, pitch) = match perturbation {
            None => (note.onset_sample, note.pitch),
            Some(p) => (
                p.onset(note.onset_sample).ok_or_else(|| {
                    Error::new(format!("{} does not fit note {index}", p.label()))
                })?,
                p.pitch(note.pitch),
            ),
        };
        take.push(TakeNote {
            onset_sample,
            pitch,
            velocity: note.velocity,
            cites: Some(id),
        });
    }
    take.sort_by_key(TakeNote::key);
    Ok(take)
}

/// A deliberate change to the take, for the negative controls: the check
/// must fail when the take is not the constructed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeEdit {
    /// The constructed take, unchanged.
    None,
    /// The last note of the take, in its total order, one sample later. It
    /// stays the last note, so the take is still admissible and the only
    /// change is one onset by one sample.
    LastNoteOneSampleLater,
}

impl TakeEdit {
    pub fn apply(self, take: &mut [TakeNote]) -> Result<(), Error> {
        match self {
            TakeEdit::None => Ok(()),
            TakeEdit::LastNoteOneSampleLater => {
                let last = take
                    .last_mut()
                    .ok_or_else(|| Error::new("an empty take has no last note"))?;
                last.onset_sample = last
                    .onset_sample
                    .checked_add(1)
                    .filter(|&s| s <= MAX_SAMPLE)
                    .ok_or_else(|| Error::new("the last onset cannot move later"))?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use law::Law;
    use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};

    /// A score at 120 BPM (24,000 samples a quarter) with a note at each of
    /// `quarters`, pitches `first_pitch`, `first_pitch + 1`, ...
    fn ingested(quarters: &[u64], first_pitch: u8) -> IngestedScore {
        let notes = quarters
            .iter()
            .enumerate()
            .map(|(i, &q)| IngestedNote {
                start_tick: q * 3_360,
                pitch: first_pitch + u8::try_from(i).unwrap(),
                track: 1,
                channel: 0,
                end_tick: q * 3_360 + 1_680,
                velocity: 80,
            })
            .collect();
        IngestedScore {
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
        }
    }

    fn score(quarters: &[u64], first_pitch: u8) -> LawScore {
        Law::load(&ingested(quarters, first_pitch))
            .unwrap()
            .score()
            .clone()
    }

    fn forty() -> Vec<u64> {
        (0..40).collect()
    }

    #[test]
    fn the_seed_is_si_jam_1_in_ascii() {
        assert_eq!(SEED, 0x7369_2D6A_616D_2D31);
        assert_eq!(&SEED.to_be_bytes(), b"si-jam-1");
    }

    #[test]
    fn the_offsets_are_the_phase_0_milliseconds() {
        let ms: Vec<i64> = Perturbation::DRAW_ORDER
            .iter()
            .map(|p| p.delta_samples() / 48)
            .collect();
        assert_eq!(ms, [30, 45, 60, -45, 0]);
        assert_eq!(Perturbation::WrongPitch.pitch(60), 61);
        assert_eq!(Perturbation::WrongPitch.pitch(126), 127);
        assert_eq!(Perturbation::WrongPitch.pitch(127), 126);
        assert_eq!(Perturbation::Late60.pitch(127), 127);
    }

    #[test]
    fn an_early_note_needs_a_score_onset_of_at_least_2160() {
        assert_eq!(Perturbation::Early45.onset(2_159), None);
        assert_eq!(Perturbation::Early45.onset(2_160), Some(0));
        assert_eq!(Perturbation::Late60.onset(0), Some(2_880));
        assert_eq!(
            Perturbation::Late30.onset(MAX_SAMPLE - 1_440),
            Some(MAX_SAMPLE)
        );
        assert_eq!(Perturbation::Late30.onset(MAX_SAMPLE - 1_439), None);
        assert_eq!(Perturbation::WrongPitch.onset(0), Some(0));
    }

    #[test]
    fn five_distinct_notes_are_drawn_and_each_fits() {
        let s = score(&forty(), 40);
        let drawn = draw(&s, SEED).unwrap();
        let mut ids: Vec<u32> = drawn.iter().map(|d| d.note.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 5, "distinct: {drawn:?}");
        let order: Vec<Perturbation> = drawn.iter().map(|d| d.perturbation).collect();
        assert_eq!(order, Perturbation::DRAW_ORDER);
        for d in &drawn {
            let onset = s.note(d.note).unwrap().onset_sample;
            assert!(d.perturbation.onset(onset).is_some());
        }
        assert_eq!(draw(&s, SEED).unwrap(), drawn, "the draw is reproducible");
        assert_ne!(draw(&s, SEED + 1).unwrap(), drawn, "the seed decides it");
    }

    #[test]
    fn the_early_note_is_redrawn_until_it_fits() {
        // Only note 5 starts at or after 2,160 samples. The late notes are
        // drawn first: when they leave note 5 free, the early draw lands on
        // it whatever the seed; when one of them took it, nothing else fits,
        // and the draw refuses after MAX_DRAWS instead of looping.
        let s = score(&[0, 0, 0, 0, 0, 1], 60);
        let (mut landed, mut refused) = (0, 0);
        for seed in 0..16 {
            match draw(&s, seed) {
                Ok(drawn) => {
                    assert_eq!(drawn[3].perturbation, Perturbation::Early45);
                    assert_eq!(drawn[3].note, ScoreNoteId(5), "seed {seed}");
                    landed += 1;
                }
                Err(e) => {
                    assert_eq!(e.to_string(), "no note fits early-45ms in 1000000 draws");
                    refused += 1;
                }
            }
        }
        assert!(
            landed > 0 && refused > 0,
            "{landed} landed, {refused} refused"
        );
        // Without note 5 nothing ever fits.
        let s = score(&[0, 0, 0, 0, 0], 60);
        assert_eq!(
            draw(&s, SEED).unwrap_err().to_string(),
            "no note fits early-45ms in 1000000 draws"
        );
    }

    #[test]
    fn the_take_plays_every_note_once_and_moves_only_the_drawn_five() {
        let ingested = ingested(&forty(), 40);
        let mut law = Law::load(&ingested).unwrap();
        let s = law.score().clone();
        let drawn = draw(&s, SEED).unwrap();
        let take = construct(&s, &drawn).unwrap();
        assert_eq!(take.len(), 40);
        assert!(
            take.windows(2).all(|w| w[0].key() < w[1].key()),
            "total order"
        );
        let mut changed = 0;
        for t in &take {
            let id = t.cites.unwrap();
            let n = s.note(id).unwrap();
            assert_eq!(t.velocity, n.velocity);
            match drawn.iter().find(|d| d.note == id) {
                None => assert_eq!((t.onset_sample, t.pitch), (n.onset_sample, n.pitch)),
                Some(d) => {
                    changed += 1;
                    let delta = i64::try_from(t.onset_sample).unwrap()
                        - i64::try_from(n.onset_sample).unwrap();
                    assert_eq!(delta, d.perturbation.delta_samples());
                    assert_eq!(t.pitch, d.perturbation.pitch(n.pitch));
                }
            }
        }
        assert_eq!(changed, 5);
        law.admit(&take).unwrap();
    }

    #[test]
    fn the_edit_moves_only_the_last_note_by_one_sample() {
        let s = score(&forty(), 40);
        let take = construct(&s, &draw(&s, SEED).unwrap()).unwrap();
        let mut edited = take.clone();
        TakeEdit::LastNoteOneSampleLater.apply(&mut edited).unwrap();
        let last = take.len() - 1;
        assert_eq!(edited[..last], take[..last]);
        assert_eq!(edited[last].onset_sample, take[last].onset_sample + 1);
        assert!(edited.windows(2).all(|w| w[0].key() < w[1].key()));
        let mut unchanged = take.clone();
        TakeEdit::None.apply(&mut unchanged).unwrap();
        assert_eq!(unchanged, take);
    }
}
