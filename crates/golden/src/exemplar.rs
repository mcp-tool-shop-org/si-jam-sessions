//! The exemplars' goldens: what the law commits when each arrangement of the
//! Battle Hymn plays.
//!
//! An exemplar plays with no take. The host ingests the score, steps the law
//! one quantum per 48 samples, and plays the frames the law commits (every
//! note-on of the score and every beat of its meter) from sample 0 until one
//! second past the score's last sound, where `render` stops
//! ([`TAIL_SAMPLES`]). An exemplar's golden pins exactly those frames:
//!
//! - `golden/<id>.frames`: every frame of those quanta, in the order the
//!   host's ring carries them (by onset; at one onset the beat first, then the
//!   notes in the law's frame order), one per line;
//! - `golden/<id>.golden`: the law's pins; the inputs and the container; the
//!   receipt's digest and tier; the window and the steps that commit it; the
//!   frames' bytes as the C ABI hands them to a host, and the frames file,
//!   each by size and SHA-256; and the SHA-256 of the law's snapshot of the
//!   ingested score, the exemplar's golden hash.
//!
//! The law runs twice on the same bytes, as it does for the golden
//! ([`crate::run::compute`]): through its C ABI, as a host drives it, and
//! through its Rust API. The two must agree to the byte, on the snapshot and on
//! the frames. The frames are not in the snapshot (the law hashes the score,
//! from which they are derived); the frames file and the frames' bytes are
//! pinned here instead.

use std::fmt::Write as _;
use std::path::Path;

use law::abi;
use law::wire;
use law::{Beat, FrameNote, Frames, Law, QUANTUM_SAMPLES, Voice};
use provenance::Tier;

use crate::Error;
use crate::run::{self, Difference, Inputs, Record, hex, sha256};

/// One exemplar: its id, its score's directory, and its two golden files, all
/// relative to the repository root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exemplar {
    pub id: &'static str,
    pub dir: &'static str,
    pub golden_file: &'static str,
    pub frames_file: &'static str,
}

/// The two exemplars: the Battle Hymn as glm-5.3 and as kimi-k3 arranged it.
pub const EXEMPLARS: [Exemplar; 2] = [
    Exemplar {
        id: "battle-hymn-glm-5.3",
        dir: "scores/battle-hymn-glm-5.3",
        golden_file: "golden/battle-hymn-glm-5.3.golden",
        frames_file: "golden/battle-hymn-glm-5.3.frames",
    },
    Exemplar {
        id: "battle-hymn-kimi-k3",
        dir: "scores/battle-hymn-kimi-k3",
        golden_file: "golden/battle-hymn-kimi-k3.golden",
        frames_file: "golden/battle-hymn-kimi-k3.frames",
    },
];

/// One second at the law's rate: how far past a score's last sound a render
/// runs, so its last notes ring out.
pub const TAIL_SAMPLES: u64 = 48_000;

/// An exemplar's golden, computed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExemplarGolden {
    pub exemplar: Exemplar,
    pub inputs: Vec<Record>,
    pub container: Vec<u8>,
    pub receipt_digest: [u8; 32],
    pub tier: Tier,
    pub score_notes: usize,
    /// The sample after the score's last sound: its latest note end.
    pub score_end: u64,
    /// The window's last quantum: the one holding sample `score_end +
    /// TAIL_SAMPLES - 1`, the last one a render plays. The window starts at
    /// quantum 0.
    pub last_quantum: u64,
    /// The fewest steps after which the committed horizon reaches
    /// `last_quantum`.
    pub steps: u64,
    /// The committed frames of the window.
    pub frames: Frames,
    /// The same frames as the C ABI hands them to a host (`law_frames`).
    pub frames_wire: Vec<u8>,
    /// The law's snapshot of the ingested score, before any step.
    pub snapshot: Vec<u8>,
    /// The SHA-256 of the snapshot, checked against the law's own digest.
    pub golden: [u8; 32],
}

/// Computes `exemplar`'s golden from `inputs`, its score directory's files.
pub fn compute(inputs: &Inputs, exemplar: Exemplar) -> Result<ExemplarGolden, Error> {
    let id = exemplar.id;
    let container = inputs.container()?;
    let refused = |what: &str, r: law::Refusal| {
        Error::new(format!(
            "{id}: {what} refused with status {}: {r}",
            r.code()
        ))
    };
    let mut law = Law::ingest(&container).map_err(|r| {
        Error::new(format!(
            "{id}: the law refused the score with status {}: {r}",
            r.code()
        ))
    })?;
    let provenance = law
        .provenance()
        .cloned()
        .ok_or_else(|| Error::new(format!("{id}: an ingested score has no provenance")))?;
    let snapshot = law
        .snapshot_bytes()
        .map_err(|r| refused("the snapshot", r))?;

    let overflow = || Error::new(format!("{id}: a sample position overflowed"));
    let mut score_end = 0u64;
    for n in law.score().notes() {
        let end = n
            .onset_sample
            .checked_add(n.duration_samples)
            .ok_or_else(overflow)?;
        score_end = score_end.max(end);
    }
    let last_quantum = score_end
        .checked_add(TAIL_SAMPLES - 1)
        .ok_or_else(overflow)?
        / u64::from(QUANTUM_SAMPLES);
    let mut steps = 0u64;
    while law
        .committed_horizon()
        .map_err(|r| refused("the horizon", r))?
        .is_none_or(|h| h < last_quantum)
    {
        law.step().map_err(|r| refused("a step", r))?;
        steps += 1;
    }
    let frames = law
        .frames(0, last_quantum)
        .map_err(|r| refused("the frames", r))?;
    let frames_wire = wire::encode_frames(&frames).map_err(|r| refused("the frame encoder", r))?;
    // No take: every frame note is a score note, each played once.
    if frames.notes.iter().any(|n| n.voice != Voice::Score) {
        return Err(Error::new(format!("{id}: a frame note is not the score's")));
    }
    if frames.notes.len() != law.score().notes().len() {
        return Err(Error::new(format!(
            "{id}: the window holds {} note-ons for {} score notes",
            frames.notes.len(),
            law.score().notes().len()
        )));
    }

    let run = abi_run(&container, steps, last_quantum)?;
    if run.snapshot != snapshot {
        return Err(Error::new(format!(
            "{id}: the C ABI and the Rust API give different snapshots"
        )));
    }
    if run.frames != frames_wire {
        return Err(Error::new(format!(
            "{id}: the C ABI and the Rust API commit different frames"
        )));
    }
    let golden = sha256(&snapshot);
    if golden != run.hash {
        return Err(Error::new(format!(
            "{id}: the law's hash is not the SHA-256 of its snapshot"
        )));
    }

    Ok(ExemplarGolden {
        exemplar,
        inputs: inputs.records(),
        container,
        receipt_digest: provenance.receipt_digest,
        tier: provenance.tier,
        score_notes: law.score().notes().len(),
        score_end,
        last_quantum,
        steps,
        frames,
        frames_wire,
        snapshot,
        golden,
    })
}

/// Reads `exemplar`'s inputs from the repository at `root` and computes its
/// golden.
pub fn compute_at(root: &Path, exemplar: Exemplar) -> Result<ExemplarGolden, Error> {
    compute(&Inputs::read_dir(root, exemplar.dir)?, exemplar)
}

/// What one run through the C ABI read back.
struct AbiRun {
    hash: [u8; 32],
    snapshot: Vec<u8>,
    frames: Vec<u8>,
}

/// The container into `law_ingest`, a snapshot, `steps` calls to `law_step`,
/// and the frames of quanta `0..=last`, as a host reads them.
fn abi_run(container: &[u8], steps: u64, last: u64) -> Result<AbiRun, Error> {
    let _turn = run::TURN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    run::pass(container, abi::law_ingest, "law_ingest")?;
    let hash = run::snapshot_hash()?;
    let snapshot = run::read_out(abi::law_snapshot_ptr(), abi::law_snapshot_len());
    for _ in 0..steps {
        let status = abi::law_step();
        if status != 0 {
            return Err(run::refusal("law_step", status));
        }
    }
    let status = abi::law_frames(0, last);
    if status != 0 {
        return Err(run::refusal("law_frames", status));
    }
    let frames = run::read_out(abi::law_frames_ptr(), abi::law_frames_len());
    Ok(AbiRun {
        hash,
        snapshot,
        frames,
    })
}

/// A beat's line in the frames file.
fn beat_line(t: &mut String, b: &Beat) {
    // Writing to a String cannot fail.
    let _ = writeln!(
        t,
        "{} beat {}.{}{}",
        b.onset_sample,
        b.bar,
        b.beat,
        if b.downbeat { " downbeat" } else { "" }
    );
}

/// A note's line in the frames file.
fn note_line(t: &mut String, n: &FrameNote) {
    let id = n
        .note
        .map_or(String::from("addition"), |id| id.0.to_string());
    let _ = writeln!(
        t,
        "{} note {id} pitch {} velocity {} length {}",
        n.onset_sample, n.pitch, n.velocity, n.duration_samples
    );
}

impl ExemplarGolden {
    /// The frames file's text: every frame of the window, in the order the
    /// host's ring carries them.
    pub fn frames_text(&self) -> String {
        let mut t = String::new();
        let mut beats = self.frames.beats.iter().peekable();
        for n in &self.frames.notes {
            while let Some(b) = beats.next_if(|b| b.onset_sample <= n.onset_sample) {
                beat_line(&mut t, b);
            }
            note_line(&mut t, n);
        }
        for b in beats {
            beat_line(&mut t, b);
        }
        t
    }

    /// The golden file's text. Every line after the comments is `key value`,
    /// and the line order is fixed.
    pub fn golden_text(&self) -> String {
        let x = self.exemplar;
        let mut t = String::new();
        let mut line = |text: String| {
            t.push_str(&text);
            t.push('\n');
        };
        line(format!(
            "# The golden of the exemplar {}: what the law commits when the score plays with",
            x.id
        ));
        line(String::from(
            "# no take, from sample 0 to one second past its last sound. Written only by",
        ));
        line(String::from(
            "# `cargo run -p golden --bin write-golden`; `write-golden --check` exits non-zero",
        ));
        line(format!(
            "# when regenerating would change this file or {}.",
            x.frames_file
        ));
        line(format!("golden-format {}", run::GOLDEN_FORMAT));
        line(format!("exemplar {}", x.id));
        line(format!("law-version {}", law::LAW_VERSION));
        line(format!("snapshot-format {}", law::SNAPSHOT_FORMAT));
        line(format!(
            "predicate-version {}",
            provenance::PREDICATE_VERSION
        ));
        line(format!("rules-year {}", provenance::RULES_YEAR));
        line(format!(
            "us-last-public-domain-publication-year {}",
            provenance::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR
        ));
        line(format!(
            "eu-last-public-domain-death-year {}",
            provenance::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR
        ));
        line(format!(
            "last-out-of-term-edition-year {}",
            provenance::LAST_OUT_OF_TERM_EDITION_YEAR
        ));
        line(format!("ppq {}", law::PPQ));
        line(format!("sample-rate {}", law::SAMPLE_RATE));
        line(format!("quantum-samples {}", law::QUANTUM_SAMPLES));
        line(format!("horizon-quanta {}", law::HORIZON_QUANTA));
        line(format!("gate-samples {}", law::GATE_SAMPLES));
        for r in &self.inputs {
            line(format!("input {} {} {}", r.name, r.bytes, hex(&r.sha256)));
        }
        line(format!(
            "container {} {}",
            self.container.len(),
            hex(&sha256(&self.container))
        ));
        line(format!("receipt-digest {}", hex(&self.receipt_digest)));
        line(format!("tier {}", run::tier_label(&self.tier)));
        line(format!("score-notes {}", self.score_notes));
        line(format!("score-end {}", self.score_end));
        line(format!("tail-samples {TAIL_SAMPLES}"));
        line(format!("window 0 {}", self.last_quantum));
        line(format!("steps {}", self.steps));
        line(format!(
            "frame-notes {} beats {}",
            self.frames.notes.len(),
            self.frames.beats.len()
        ));
        line(format!(
            "frames-wire {} {}",
            self.frames_wire.len(),
            hex(&sha256(&self.frames_wire))
        ));
        let frames = self.frames_text();
        line(format!(
            "frames {} {}",
            frames.len(),
            hex(&sha256(frames.as_bytes()))
        ));
        line(format!("snapshot {}", self.snapshot.len()));
        line(format!("golden {}", hex(&self.golden)));
        t
    }

    /// The two files `write-golden` writes for the exemplar, with their paths
    /// from the root.
    pub fn files(&self) -> [(&'static str, String); 2] {
        [
            (self.exemplar.golden_file, self.golden_text()),
            (self.exemplar.frames_file, self.frames_text()),
        ]
    }

    /// Compares the committed files under `root` with these. Empty when
    /// regenerating would change neither.
    pub fn check(&self, root: &Path) -> Vec<Difference> {
        run::check_files(root, self.files())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_root;
    use crate::run::GoldenFile;

    fn goldens() -> Vec<ExemplarGolden> {
        let root = repo_root();
        EXEMPLARS
            .iter()
            .map(|x| compute_at(&root, *x).unwrap())
            .collect()
    }

    #[test]
    fn the_committed_exemplar_goldens_are_current() {
        let root = repo_root();
        for g in goldens() {
            let differences = g.check(&root);
            assert!(
                differences.is_empty(),
                "{}: run `cargo run -p golden --bin write-golden`: {differences:#?}",
                g.exemplar.id
            );
        }
    }

    /// The window holds every score note once, as the score holds it, and the
    /// beats of the meter up to the window's end; the frames file lists them
    /// all, a beat before the notes at its onset.
    #[test]
    fn the_frames_are_the_score_and_its_beats() {
        for g in goldens() {
            let id = g.exemplar.id;
            let law = Law::ingest(&g.container).unwrap();
            let score = law.score().notes();
            assert_eq!(g.frames.notes.len(), score.len(), "{id}");
            let mut seen = vec![false; score.len()];
            for f in &g.frames.notes {
                let n = f.note.expect("a score frame cites its note").0 as usize;
                assert!(!seen[n], "{id}: note {n} twice");
                seen[n] = true;
                let s = &score[n];
                assert_eq!(
                    (f.onset_sample, f.pitch, f.velocity, f.duration_samples),
                    (s.onset_sample, s.pitch, s.velocity, s.duration_samples),
                    "{id}: note {n}"
                );
            }
            let end = (g.last_quantum + 1) * u64::from(QUANTUM_SAMPLES);
            assert!(g.score_end + TAIL_SAMPLES <= end, "{id}");
            assert!(g.score_end + TAIL_SAMPLES > end - u64::from(QUANTUM_SAMPLES));
            let first = g.frames.beats.first().expect("a first beat");
            assert_eq!((first.onset_sample, first.bar, first.beat), (0, 0, 0));
            assert!(first.downbeat);
            let last = g.frames.beats.last().unwrap();
            assert!(last.onset_sample < end, "{id}");
            assert!(g.frames.beats.len() > 300, "{id}: every bar has its beats");
            let text = g.frames_text();
            assert_eq!(
                text.lines().count(),
                g.frames.notes.len() + g.frames.beats.len()
            );
            assert!(text.starts_with("0 beat 0.0 downbeat\n"), "{id}");
            let onsets: Vec<u64> = text
                .lines()
                .map(|l| l.split(' ').next().unwrap().parse().unwrap())
                .collect();
            assert!(onsets.windows(2).all(|w| w[0] <= w[1]), "{id}: by onset");
        }
    }

    /// The fewest steps commit the window: one step fewer leaves its last
    /// quantum uncommitted, and the law refuses to read it.
    #[test]
    fn the_steps_are_the_fewest_that_commit_the_window() {
        for g in goldens() {
            let mut law = Law::ingest(&g.container).unwrap();
            for _ in 1..g.steps {
                law.step().unwrap();
            }
            assert!(law.frames(0, g.last_quantum).is_err(), "{}", g.exemplar.id);
            law.step().unwrap();
            assert_eq!(law.frames(0, g.last_quantum).unwrap(), g.frames);
        }
    }

    /// The golden file reads back, and its hash is the snapshot's SHA-256.
    #[test]
    fn the_exemplar_golden_file_reads_back() {
        for g in goldens() {
            let file = GoldenFile::parse(&g.golden_text()).unwrap();
            assert_eq!(file.golden().unwrap(), g.golden);
            assert_eq!(file.one("exemplar").unwrap(), g.exemplar.id);
            assert_eq!(file.one("tier").unwrap(), "own-engraving");
            assert_eq!(file.one("predicate-version").unwrap(), "3");
            assert_eq!(file.all("input").len(), 3);
            assert_eq!(g.golden, sha256(&g.snapshot));
        }
    }

    /// Changing one committed frame, or one byte of a score file (which the
    /// receipt refuses), is seen.
    #[test]
    fn a_changed_frame_or_score_file_fails() {
        let root = repo_root();
        let g = compute_at(&root, EXEMPLARS[0]).unwrap();
        let text = g.frames_text();
        let moved = text.replacen(" velocity ", " velocity 1", 1);
        let d = run::difference(g.exemplar.frames_file, &text, &moved).unwrap();
        assert_eq!(d.count, 1);

        let mut inputs = Inputs::read_dir(&root, EXEMPLARS[0].dir).unwrap();
        let last = inputs.files[1].1.len() - 1;
        inputs.files[1].1[last] ^= 1;
        let e = compute(&inputs, EXEMPLARS[0]).unwrap_err();
        assert!(e.to_string().contains("refused the score"), "{e}");
    }

    /// The two arrangements are two scores: different notes, frames and hashes.
    #[test]
    fn the_two_exemplars_differ() {
        let g = goldens();
        assert_ne!(g[0].golden, g[1].golden);
        assert_ne!(g[0].frames_wire, g[1].frames_wire);
        assert_ne!(g[0].score_notes, g[1].score_notes);
    }
}
