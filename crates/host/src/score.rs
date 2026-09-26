//! The committed score, *The Entertainer*, and PHASE-0's constructed take of
//! it, built exactly as the golden builds them: the same receipt and files,
//! the same seed, the same draw.

use std::path::{Path, PathBuf};

use golden::run::{Inputs, SCORE_DIR};
use golden::take::{self, Drawn, SEED};
use law::{Law, LawScore, SAMPLE_RATE, wire};

/// The score and the take, ready for the law's C ABI.
pub struct Piece {
    /// The receipt and every file it receipts, in the law's container layout:
    /// what the ingest verb reads.
    pub container: Vec<u8>,
    /// The score in law form, read with the law's Rust API for the draw and
    /// for the listening guide.
    pub score: LawScore,
    /// The constructed take, in the law's take layout.
    pub take: Vec<u8>,
    /// The five notes the seed drew and what it did to each.
    pub drawn: [Drawn; 5],
    /// The sample after the last one any note of the score or the take sounds
    /// on, before its release.
    pub end: u64,
}

impl Piece {
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
            container,
            score,
            take,
            drawn,
            end,
        })
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
        assert_eq!(piece.container, golden.container);
        assert_eq!(piece.take, golden.take);
        assert_eq!(piece.score.notes().len(), 2_621);
        let score_end = piece
            .score
            .notes()
            .iter()
            .map(|n| n.onset_sample + n.duration_samples)
            .max()
            .unwrap();
        assert!(piece.end >= score_end);
    }

    #[test]
    fn a_sample_reads_as_a_clock() {
        assert_eq!(clock(0), "0:00.000");
        assert_eq!(clock(48), "0:00.001");
        assert_eq!(clock(48_000 * 83 + 480), "1:23.010");
        assert_eq!(clock(11_682_155), "4:03.378");
    }
}
