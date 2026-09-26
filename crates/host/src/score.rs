//! The pieces the host plays, and the scores it hands the law for them.
//!
//! - The two exemplars, *Battle Hymn of the Republic* as glm-5.3 and as
//!   kimi-k3 arranged it, play with no take: the score alone, as their goldens
//!   pin it ([`golden::exemplar`]). The glm-5.3 arrangement is the default.
//! - *The Entertainer* plays with PHASE-0's constructed take, built exactly as
//!   the golden builds it: the same receipt and files, the same seed, the same
//!   draw.

use std::path::{Path, PathBuf};

use golden::exemplar::{EXEMPLARS, Exemplar, TAIL_SAMPLES};
use golden::run::{Inputs, SCORE_DIR};
use golden::take::{self, Drawn, SEED};
use law::{Law, LawScore, SAMPLE_RATE, wire};

/// A piece the host can play, by the name `--piece` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name {
    /// *Battle Hymn of the Republic*, arranged by glm-5.3: the default.
    BattleHymnGlm53,
    /// *Battle Hymn of the Republic*, arranged by kimi-k3.
    BattleHymnKimiK3,
    /// *The Entertainer*, with PHASE-0's constructed take.
    Entertainer,
}

impl Name {
    /// Every piece, the default first.
    pub const ALL: [Name; 3] = [
        Name::BattleHymnGlm53,
        Name::BattleHymnKimiK3,
        Name::Entertainer,
    ];

    /// The piece a command plays when `--piece` names none.
    pub const DEFAULT: Name = Name::BattleHymnGlm53;

    /// The name `--piece` takes: the score's directory under `scores/`.
    pub fn id(self) -> &'static str {
        match self {
            Name::Entertainer => "entertainer",
            exemplar => exemplar.exemplar().map_or("", |x| x.id),
        }
    }

    /// The piece `--piece` names, if any.
    pub fn parse(id: &str) -> Option<Name> {
        Name::ALL.into_iter().find(|n| n.id() == id)
    }

    /// The piece as a report names it.
    pub fn title(self) -> &'static str {
        match self {
            Name::BattleHymnGlm53 => "Battle Hymn of the Republic, arranged by glm-5.3",
            Name::BattleHymnKimiK3 => "Battle Hymn of the Republic, arranged by kimi-k3",
            Name::Entertainer => "The Entertainer",
        }
    }

    /// The exemplar the piece is, if it is one.
    pub fn exemplar(self) -> Option<Exemplar> {
        match self {
            Name::BattleHymnGlm53 => Some(EXEMPLARS[0]),
            Name::BattleHymnKimiK3 => Some(EXEMPLARS[1]),
            Name::Entertainer => None,
        }
    }
}

/// A score, and the take that plays against it if the piece has one, ready
/// for the law's C ABI.
pub struct Piece {
    pub name: Name,
    /// The receipt and every file it receipts, in the law's container layout:
    /// what the ingest verb reads.
    pub container: Vec<u8>,
    /// The score in law form, read with the law's Rust API for the draw and
    /// for the listening guide.
    pub score: LawScore,
    /// The constructed take, in the law's take layout: *The Entertainer*'s.
    /// An exemplar plays with none.
    pub take: Option<Vec<u8>>,
    /// The five notes the seed drew and what it did to each; none without a
    /// take.
    pub drawn: Vec<Drawn>,
    /// The sample after the last one any note of the score or the take sounds
    /// on, before its release.
    pub end: u64,
}

impl Piece {
    /// The piece `name`, from the repository at `root`.
    pub fn load(root: &Path, name: Name) -> Result<Piece, String> {
        let Some(exemplar) = name.exemplar() else {
            return Piece::entertainer(root);
        };
        let inputs = Inputs::read_dir(root, exemplar.dir).map_err(|e| e.to_string())?;
        let container = inputs.container().map_err(|e| e.to_string())?;
        let law = Law::ingest(&container).map_err(|r| r.to_string())?;
        let score = law.score().clone();
        let end = score
            .notes()
            .iter()
            .map(|n| n.onset_sample.saturating_add(n.duration_samples))
            .max()
            .unwrap_or(0);
        Ok(Piece {
            name,
            container,
            score,
            take: None,
            drawn: Vec::new(),
            end,
        })
    }

    /// The score and its constructed take, from the repository at `root`.
    pub fn entertainer(root: &Path) -> Result<Piece, String> {
        let inputs = Inputs::read(root).map_err(|e| e.to_string())?;
        let container = inputs.container().map_err(|e| e.to_string())?;
        let law = Law::ingest(&container).map_err(|r| r.to_string())?;
        let score = law.score().clone();
        let drawn = take::draw(&score, SEED).map_err(|e| e.to_string())?;
        let notes = take::construct(&score, &drawn).map_err(|e| e.to_string())?;
        let take = wire::encode_take(&notes).map_err(|r| r.to_string())?;
        let mut end = 0u64;
        for n in score.notes() {
            end = end.max(n.onset_sample.saturating_add(n.duration_samples));
        }
        for t in &notes {
            let length = t
                .cites
                .and_then(|id| score.note(id))
                .map_or(0, |n| n.duration_samples);
            end = end.max(t.onset_sample.saturating_add(length));
        }
        Ok(Piece {
            name: Name::Entertainer,
            container,
            score,
            take: Some(take),
            drawn: drawn.to_vec(),
            end,
        })
    }

    /// The sample a command plays to: one second past [`Piece::end`], so the
    /// last notes ring out.
    pub fn stop(&self) -> u64 {
        self.end.saturating_add(TAIL_SAMPLES)
    }
}

/// The repository root: the nearest ancestor of the current directory that
/// holds the committed score, or else the one this crate was built in.
pub fn root() -> PathBuf {
    let receipt = Path::new(SCORE_DIR).join("receipt.json");
    if let Ok(here) = std::env::current_dir() {
        for dir in here.ancestors() {
            if dir.join(&receipt).is_file() {
                return dir.to_path_buf();
            }
        }
    }
    golden::repo_root()
}

/// A law sample as minutes, seconds and milliseconds, for a person to find
/// by ear: `m:ss.mmm`.
pub fn clock(sample: u64) -> String {
    let rate = u64::from(SAMPLE_RATE);
    let ms = sample.saturating_mul(1_000) / rate;
    format!("{}:{:02}.{:03}", ms / 60_000, ms / 1_000 % 60, ms % 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_piece_is_the_golden_take() {
        let root = golden::repo_root();
        let piece = Piece::entertainer(&root).unwrap();
        // The golden runs the law's C ABI too, which one caller holds at a time.
        let _abi = crate::bridge::Law::acquire();
        let golden = golden::run::compute(
            &Inputs::read(&root).unwrap(),
            SEED,
            golden::take::TakeEdit::None,
        )
        .unwrap();
        assert_eq!(piece.name, Name::Entertainer);
        assert_eq!(piece.container, golden.container);
        assert_eq!(piece.take.as_deref(), Some(golden.take.as_slice()));
        assert_eq!(piece.drawn.len(), 5);
        assert_eq!(piece.score.notes().len(), 2_621);
        let score_end = piece
            .score
            .notes()
            .iter()
            .map(|n| n.onset_sample + n.duration_samples)
            .max()
            .unwrap();
        assert!(piece.end >= score_end);
        assert_eq!(
            Piece::load(&root, Name::Entertainer).unwrap().take,
            piece.take
        );
    }

    /// `--piece` takes three names, each a score directory; the glm-5.3
    /// arrangement of the Battle Hymn is the default, and the exemplars are
    /// the goldens' two.
    #[test]
    fn the_pieces_are_named_and_the_battle_hymn_is_the_default() {
        assert_eq!(Name::DEFAULT, Name::BattleHymnGlm53);
        assert_eq!(Name::ALL[0], Name::DEFAULT);
        let ids: Vec<&str> = Name::ALL.iter().map(|n| n.id()).collect();
        assert_eq!(
            ids,
            ["battle-hymn-glm-5.3", "battle-hymn-kimi-k3", "entertainer"]
        );
        for n in Name::ALL {
            assert_eq!(Name::parse(n.id()), Some(n));
            assert!(
                golden::repo_root()
                    .join("scores")
                    .join(n.id())
                    .join("receipt.json")
                    .is_file(),
                "{}",
                n.id()
            );
        }
        for bad in ["", "battle-hymn", "Entertainer", "glm-5.3", "entertainer "] {
            assert_eq!(Name::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Name::BattleHymnGlm53.exemplar(), Some(EXEMPLARS[0]));
        assert_eq!(Name::BattleHymnKimiK3.exemplar(), Some(EXEMPLARS[1]));
        assert_eq!(Name::Entertainer.exemplar(), None);
    }

    /// An exemplar is its golden's score, with no take: the same container,
    /// every note, and the same end.
    #[test]
    fn an_exemplar_piece_is_its_goldens_score_with_no_take() {
        let root = golden::repo_root();
        for (name, notes) in [
            (Name::BattleHymnGlm53, 1_720),
            (Name::BattleHymnKimiK3, 1_924),
        ] {
            let piece = Piece::load(&root, name).unwrap();
            let _abi = crate::bridge::Law::acquire();
            let golden = golden::exemplar::compute_at(&root, name.exemplar().unwrap()).unwrap();
            assert_eq!(piece.name, name);
            assert_eq!(piece.container, golden.container);
            assert_eq!(piece.take, None);
            assert!(piece.drawn.is_empty());
            assert_eq!(piece.score.notes().len(), notes);
            assert_eq!(piece.end, golden.score_end);
            assert_eq!(piece.stop(), golden.score_end + TAIL_SAMPLES);
        }
    }

    #[test]
    fn a_sample_reads_as_a_clock() {
        assert_eq!(clock(0), "0:00.000");
        assert_eq!(clock(48), "0:00.001");
        assert_eq!(clock(48_000 * 83 + 480), "1:23.010");
        assert_eq!(clock(11_682_155), "4:03.378");
    }
}
