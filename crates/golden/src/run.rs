//! Computing the golden: the inputs, one run of the law through its C ABI and
//! one through its Rust API, and the two files `write-golden` writes.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use law::abi;
use law::wire::{self, ContainerFile};
use law::{CitedKind, Law, LawScore, QUANTUM_SAMPLES, TakeNote, Verdict};
use provenance::{Receipt, Tier};
use sha2::{Digest, Sha256};

use crate::Error;
use crate::take::{self, Perturbation, TakeEdit};

/// The score and its receipt, relative to the repository root.
pub const SCORE_DIR: &str = "scores/entertainer";
/// The receipt's file name inside [`SCORE_DIR`].
pub const RECEIPT_FILE: &str = "receipt.json";
/// The golden file, relative to the repository root.
pub const GOLDEN_FILE: &str = "golden/entertainer.golden";
/// Every row the law graded, one per line, relative to the repository root.
pub const ROWS_FILE: &str = "golden/entertainer.rows";
/// The version of the golden file's own layout.
pub const GOLDEN_FORMAT: u32 = 1;

/// SHA-256.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Lower-case hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // Writing to a String cannot fail.
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The files the golden is computed from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inputs {
    /// The receipt's bytes, as committed.
    pub receipt: Vec<u8>,
    /// Every file the receipt lists, by its receipt name, in name order.
    pub files: Vec<(String, Vec<u8>)>,
}

impl Inputs {
    /// Reads the receipt from [`SCORE_DIR`] under `root`, and every file it
    /// lists from beside it. The receipt, not a list here, decides the files,
    /// so every receipted file reaches the law.
    pub fn read(root: &Path) -> Result<Inputs, Error> {
        let dir = root.join(SCORE_DIR);
        let receipt = read_file(&dir.join(RECEIPT_FILE))?;
        let parsed = Receipt::from_json(&receipt)
            .map_err(|e| Error::new(format!("{SCORE_DIR}/{RECEIPT_FILE} does not load: {e:?}")))?;
        let mut files = Vec::new();
        // Receipt names are plain file names (no separators), so each is a
        // file inside `dir`.
        for f in &parsed.files {
            files.push((f.name.clone(), read_file(&dir.join(&f.name))?));
        }
        Ok(Inputs { receipt, files })
    }

    /// The container the law's ingest verb reads.
    pub fn container(&self) -> Result<Vec<u8>, Error> {
        let files: Vec<ContainerFile<'_>> = self
            .files
            .iter()
            .map(|(name, bytes)| ContainerFile { name, bytes })
            .collect();
        wire::encode_container(&self.receipt, &files)
            .map_err(|r| Error::new(format!("the container does not encode: {r}")))
    }

    /// Each input as the golden file names it: its path from the root, its
    /// size and its SHA-256. The receipt first, then the files by name.
    pub fn records(&self) -> Vec<Record> {
        let mut out = vec![Record::of(
            format!("{SCORE_DIR}/{RECEIPT_FILE}"),
            &self.receipt,
        )];
        for (name, bytes) in &self.files {
            out.push(Record::of(format!("{SCORE_DIR}/{name}"), bytes));
        }
        out
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(path).map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

/// A named byte string's size and SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub bytes: usize,
    pub sha256: [u8; 32],
}

impl Record {
    pub fn of(name: impl Into<String>, bytes: &[u8]) -> Record {
        Record {
            name: name.into(),
            bytes: bytes.len(),
            sha256: sha256(bytes),
        }
    }
}

/// One drawn note, as the golden file records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnRecord {
    pub perturbation: Perturbation,
    pub note: u32,
    pub score_onset: u64,
    pub played_onset: u64,
    pub score_pitch: u8,
    pub played_pitch: u8,
    /// The kind the law graded it, by its row's last word.
    pub verdict: &'static str,
    /// The row the law wrote for it.
    pub row: String,
}

/// How many verdicts of each kind, in the snapshot's kind order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub matched: usize,
    pub early: usize,
    pub late: usize,
    pub wrong_pitch: usize,
    pub addition: usize,
    pub never_played: usize,
}

impl Counts {
    fn of(verdicts: &[Verdict]) -> Counts {
        let mut c = Counts::default();
        for v in verdicts {
            let slot = match v {
                Verdict::Cited { kind, .. } => match kind {
                    CitedKind::Match => &mut c.matched,
                    CitedKind::Early => &mut c.early,
                    CitedKind::Late => &mut c.late,
                    CitedKind::WrongPitch => &mut c.wrong_pitch,
                },
                Verdict::Addition { .. } => &mut c.addition,
                Verdict::NeverPlayed { .. } => &mut c.never_played,
            };
            *slot += 1;
        }
        c
    }
}

/// Everything the golden file records, and the rows file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Golden {
    pub inputs: Vec<Record>,
    pub container: Vec<u8>,
    pub receipt_digest: [u8; 32],
    pub tier: Tier,
    pub score_notes: usize,
    pub seed: u64,
    pub drawn: Vec<DrawnRecord>,
    /// The take, in the law's wire layout: the bytes every engine admits.
    pub take: Vec<u8>,
    pub take_notes: usize,
    pub steps: u64,
    pub counts: Counts,
    /// Every row, one per verdict, joined by line feeds.
    pub rows: String,
    pub snapshot_bytes: usize,
    /// The law's snapshot itself, which `write-golden --snapshot` writes out
    /// so its bytes can be examined outside the harness.
    pub snapshot: Vec<u8>,
    /// The SHA-256 of the law's snapshot: the golden hash.
    pub golden: [u8; 32],
}

/// The quanta the transport steps before its second snapshot: until the
/// playhead reaches the quantum holding the last sample the score or the take
/// reaches (the latest note end or take onset).
pub fn steps_for(score: &LawScore, take: &[TakeNote]) -> Result<u64, Error> {
    let overflow = || Error::new("a sample position overflowed");
    let mut last = 0u64;
    for n in score.notes() {
        last = last.max(
            n.onset_sample
                .checked_add(n.duration_samples)
                .ok_or_else(overflow)?,
        );
    }
    for t in take {
        last = last.max(t.onset_sample);
    }
    (last / u64::from(QUANTUM_SAMPLES))
        .checked_add(1)
        .ok_or_else(overflow)
}

/// Computes the golden from `inputs`, with the take drawn from `seed` and
/// changed by `edit` (only a negative control changes it).
///
/// The law runs twice on the same bytes, and the two runs must agree to the
/// byte:
/// - through its C ABI, as every JavaScript engine drives it: the container
///   into `law_ingest`, the take into `law_admit_take`, a snapshot, `steps`
///   calls to `law_step`, and a second snapshot, whose hash must equal the
///   first (stepping is not in the record);
/// - through its Rust API: [`Law::ingest`], [`Law::admit`] and
///   [`Law::snapshot_bytes`].
///
/// The golden hash is the SHA-256 of that snapshot, checked here against the
/// law's own digest.
pub fn compute(inputs: &Inputs, seed: u64, edit: TakeEdit) -> Result<Golden, Error> {
    let container = inputs.container()?;
    let mut law = Law::ingest(&container).map_err(|r| {
        Error::new(format!(
            "the law refused the score with status {}: {r}",
            r.code()
        ))
    })?;
    let provenance = law
        .provenance()
        .cloned()
        .ok_or_else(|| Error::new("an ingested score has no provenance"))?;
    let drawn = take::draw(law.score(), seed)?;
    let mut notes = take::construct(law.score(), &drawn)?;
    edit.apply(&mut notes)?;
    let take = wire::encode_take(&notes)
        .map_err(|r| Error::new(format!("the take does not encode: {r}")))?;
    let steps = steps_for(law.score(), &notes)?;

    let refused = |what: &str, r: law::Refusal| {
        Error::new(format!("{what} refused with status {}: {r}", r.code()))
    };
    law.admit(&notes).map_err(|r| refused("the take", r))?;
    let snapshot = law
        .snapshot_bytes()
        .map_err(|r| refused("the snapshot", r))?;
    let verdicts = law.verdicts().map_err(|r| refused("grading", r))?;
    let rows = law.rows().map_err(|r| refused("the rows", r))?.join("\n");

    let run = abi_run(&container, &take, steps)?;
    if run.hash_before != run.hash_after {
        return Err(Error::new(format!(
            "stepping moved the hash: {} before, {} after",
            hex(&run.hash_before),
            hex(&run.hash_after)
        )));
    }
    if run.snapshot != snapshot || run.rows != rows {
        return Err(Error::new(
            "the C ABI and the Rust API give different snapshots or rows",
        ));
    }
    let golden = sha256(&snapshot);
    if golden != run.hash_after {
        return Err(Error::new(
            "the law's hash is not the SHA-256 of its snapshot",
        ));
    }

    let mut drawn_records = Vec::new();
    for d in &drawn {
        let score_note = law
            .score()
            .note(d.note)
            .ok_or_else(|| Error::new("a drawn note is not in the score"))?;
        let played = notes
            .iter()
            .find(|t| t.cites == Some(d.note))
            .ok_or_else(|| Error::new("a drawn note is not in the take"))?;
        let (index, verdict) = verdicts
            .iter()
            .enumerate()
            .find(|(_, v)| matches!(v, Verdict::Cited { note, .. } if *note == d.note))
            .ok_or_else(|| Error::new("a drawn note has no verdict"))?;
        let row = rows
            .lines()
            .nth(index)
            .ok_or_else(|| Error::new("a drawn note has no row"))?;
        drawn_records.push(DrawnRecord {
            perturbation: d.perturbation,
            note: d.note.0,
            score_onset: score_note.onset_sample,
            played_onset: played.onset_sample,
            score_pitch: score_note.pitch,
            played_pitch: played.pitch,
            verdict: verdict.word(),
            row: row.to_owned(),
        });
    }

    Ok(Golden {
        inputs: inputs.records(),
        container,
        receipt_digest: provenance.receipt_digest,
        tier: provenance.tier,
        score_notes: law.score().notes().len(),
        seed,
        drawn: drawn_records,
        take,
        take_notes: notes.len(),
        steps,
        counts: Counts::of(&verdicts),
        rows,
        snapshot_bytes: snapshot.len(),
        snapshot,
        golden,
    })
}

/// What one run through the C ABI read back.
struct AbiRun {
    hash_before: [u8; 32],
    hash_after: [u8; 32],
    snapshot: Vec<u8>,
    rows: String,
}

/// The law's C ABI shares one state per process, so runs take turns.
static TURN: Mutex<()> = Mutex::new(());

fn abi_run(container: &[u8], take: &[u8], steps: u64) -> Result<AbiRun, Error> {
    let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    pass(container, abi::law_ingest, "law_ingest")?;
    pass(take, abi::law_admit_take, "law_admit_take")?;
    let hash_before = snapshot_hash()?;
    for _ in 0..steps {
        let status = abi::law_step();
        if status != 0 {
            return Err(refusal("law_step", status));
        }
    }
    if abi::law_steps() != steps {
        return Err(Error::new(format!(
            "law_steps reads {} after {steps} steps",
            abi::law_steps()
        )));
    }
    let hash_after = snapshot_hash()?;
    let snapshot = read_out(abi::law_snapshot_ptr(), abi::law_snapshot_len());
    let rows = String::from_utf8(read_out(abi::law_rows_ptr(), abi::law_rows_len()))
        .map_err(|_| Error::new("the rows are not UTF-8"))?;
    Ok(AbiRun {
        hash_before,
        hash_after,
        snapshot,
        rows,
    })
}

/// Copies `bytes` into a buffer from `law_alloc`, calls `verb` on it, and
/// frees it, as a JavaScript host does.
fn pass(
    bytes: &[u8],
    verb: unsafe extern "C" fn(*const u8, u32) -> u32,
    what: &str,
) -> Result<(), Error> {
    let len = u32::try_from(bytes.len()).map_err(|_| Error::new("an input past 4 GiB"))?;
    let ptr = abi::law_alloc(len);
    if ptr.is_null() {
        return Err(Error::new(format!("law_alloc({len}) returned null")));
    }
    // SAFETY: `law_alloc` returned a buffer of `len` writable bytes, and
    // `bytes` has exactly `len` bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    // SAFETY: `ptr` points to `len` bytes this thread wrote, and nothing
    // writes them during the call.
    let status = unsafe { verb(ptr, len) };
    // SAFETY: `ptr` came from `law_alloc(len)` and is freed once, here.
    let freed = unsafe { abi::law_free(ptr, len) };
    if status != 0 {
        return Err(refusal(what, status));
    }
    if freed != 0 {
        return Err(refusal("law_free", freed));
    }
    Ok(())
}

fn snapshot_hash() -> Result<[u8; 32], Error> {
    let status = abi::law_snapshot();
    if status != 0 {
        return Err(refusal("law_snapshot", status));
    }
    read_out(abi::law_hash_ptr(), 32)
        .try_into()
        .map_err(|_| Error::new("the hash is not 32 bytes"))
}

/// Copies `len` bytes out of the law's buffer at `ptr`.
fn read_out(ptr: *const u8, len: u32) -> Vec<u8> {
    let Ok(len) = usize::try_from(len) else {
        return Vec::new();
    };
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: the pointer and length come from the law's getters, which point
    // into its live buffers until the next call that loads, admits or
    // snapshots; the bytes are copied out before any such call.
    unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
}

fn refusal(what: &str, status: u32) -> Error {
    let reason = read_out(abi::law_refusal_ptr(), abi::law_refusal_len());
    Error::new(format!(
        "{what} refused with status {status}: {}",
        String::from_utf8_lossy(&reason)
    ))
}

fn tier_label(tier: &Tier) -> String {
    match tier {
        Tier::PublicDomain => String::from("public-domain"),
        Tier::OwnEngraving => String::from("own-engraving"),
        Tier::CcBy40 { credit_ledger_id } => format!("cc-by-4.0 {credit_ledger_id}"),
    }
}

impl Golden {
    /// The golden file's text. Every line after the comments is `key value`,
    /// and the line order is fixed.
    pub fn golden_text(&self) -> String {
        let mut t = String::new();
        let mut line = |text: String| {
            t.push_str(&text);
            t.push('\n');
        };
        line(String::from(
            "# The golden hash of si-jam-sessions slice 1: The Entertainer, graded by the law",
        ));
        line(String::from(
            "# against the constructed take. Written only by `cargo run -p golden --bin write-golden`;",
        ));
        line(String::from(
            "# `write-golden --check` exits non-zero when regenerating would change this file or",
        ));
        line(format!(
            "# {ROWS_FILE}. The golden hash is the SHA-256 of the law's snapshot."
        ));
        line(format!("golden-format {GOLDEN_FORMAT}"));
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
        line(format!("tier {}", tier_label(&self.tier)));
        line(format!("score-notes {}", self.score_notes));
        line(String::from("prng splitmix64"));
        line(format!("seed 0x{:016x}", self.seed));
        for d in &self.drawn {
            let change = match d.perturbation {
                Perturbation::WrongPitch => {
                    format!("pitch {} played {}", d.score_pitch, d.played_pitch)
                }
                p => format!(
                    "onset {} played {} delta {:+}",
                    d.score_onset,
                    d.played_onset,
                    p.delta_samples()
                ),
            };
            line(format!(
                "drawn {} note {} {change} verdict {}",
                d.perturbation.label(),
                d.note,
                d.verdict
            ));
        }
        for d in &self.drawn {
            line(format!("row {} {}", d.perturbation.label(), d.row));
        }
        line(format!(
            "take {} {} notes {}",
            self.take.len(),
            hex(&sha256(&self.take)),
            self.take_notes
        ));
        line(format!("steps {}", self.steps));
        let c = self.counts;
        line(format!(
            "verdicts match {} early {} late {} wrong-pitch {} addition {} never-played {}",
            c.matched, c.early, c.late, c.wrong_pitch, c.addition, c.never_played
        ));
        let rows = self.rows_text();
        line(format!(
            "rows {} {}",
            rows.len(),
            hex(&sha256(rows.as_bytes()))
        ));
        line(format!("snapshot {}", self.snapshot_bytes));
        line(format!("golden {}", hex(&self.golden)));
        t
    }

    /// The rows file's text: every row, one per line, in verdict order.
    pub fn rows_text(&self) -> String {
        let mut t = self.rows.clone();
        t.push('\n');
        t
    }

    /// The two files `write-golden` writes, with their paths from the root.
    pub fn files(&self) -> [(&'static str, String); 2] {
        [
            (GOLDEN_FILE, self.golden_text()),
            (ROWS_FILE, self.rows_text()),
        ]
    }
}

/// A committed golden file, read back: its `key value` lines in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenFile {
    lines: Vec<(String, String)>,
}

impl GoldenFile {
    pub fn parse(text: &str) -> Result<GoldenFile, Error> {
        let mut lines = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once(' ')
                .ok_or_else(|| Error::new(format!("golden file line {} has no value", n + 1)))?;
            lines.push((key.to_owned(), value.to_owned()));
        }
        Ok(GoldenFile { lines })
    }

    pub fn read(root: &Path) -> Result<GoldenFile, Error> {
        let path = root.join(GOLDEN_FILE);
        let text = fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
        GoldenFile::parse(&text)
    }

    /// Every value of `key`, in file order.
    pub fn all(&self, key: &str) -> Vec<&str> {
        self.lines
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// The one value of `key`.
    pub fn one(&self, key: &str) -> Result<&str, Error> {
        match self.all(key).as_slice() {
            [value] => Ok(*value),
            [] => Err(Error::new(format!("the golden file has no `{key}` line"))),
            _ => Err(Error::new(format!(
                "the golden file has more than one `{key}` line"
            ))),
        }
    }

    /// The golden hash.
    pub fn golden(&self) -> Result<[u8; 32], Error> {
        let value = self.one("golden")?;
        let bytes = unhex(value).ok_or_else(|| Error::new("the golden line is not hex"))?;
        bytes
            .try_into()
            .map_err(|_| Error::new("the golden hash is not 32 bytes"))
    }
}

/// Bytes from lower-case hexadecimal, or `None`.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    text.as_bytes()
        .chunks(2)
        .map(|pair| Some(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect()
}

/// One way a committed file differs from what regenerating it gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub path: &'static str,
    /// Differing lines as (line number from 1, committed, regenerated); a side
    /// is `None` past its end. At most [`Difference::SHOWN`] are kept.
    pub lines: Vec<(usize, Option<String>, Option<String>)>,
    /// How many lines differ in all.
    pub count: usize,
    /// The committed file could not be read.
    pub missing: bool,
}

impl Difference {
    pub const SHOWN: usize = 12;
}

/// Compares a committed text with its regenerated text, line by line.
pub fn difference(path: &'static str, committed: &str, regenerated: &str) -> Option<Difference> {
    if committed == regenerated {
        return None;
    }
    let a: Vec<&str> = committed.split('\n').collect();
    let b: Vec<&str> = regenerated.split('\n').collect();
    let mut lines = Vec::new();
    let mut count = 0;
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i), b.get(i));
        if x != y {
            count += 1;
            if lines.len() < Difference::SHOWN {
                lines.push((
                    i + 1,
                    x.map(|s| (*s).to_owned()),
                    y.map(|s| (*s).to_owned()),
                ));
            }
        }
    }
    Some(Difference {
        path,
        lines,
        count,
        missing: false,
    })
}

/// Regenerates the golden from the committed inputs and compares it with the
/// committed files. Empty when regenerating would change nothing.
pub fn check(root: &Path, golden: &Golden) -> Vec<Difference> {
    let mut out = Vec::new();
    for (path, text) in golden.files() {
        match fs::read(root.join(path)) {
            Err(_) => out.push(Difference {
                path,
                lines: Vec::new(),
                count: 0,
                missing: true,
            }),
            Ok(bytes) => {
                let committed = String::from_utf8_lossy(&bytes);
                out.extend(difference(path, &committed, &text));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_root;
    use crate::take::SEED;

    fn golden() -> Golden {
        compute(&Inputs::read(&repo_root()).unwrap(), SEED, TakeEdit::None).unwrap()
    }

    /// The constructed take played through the live verbs instead of admitted
    /// whole: through the C ABI, one note at a time, in reverse order, each a
    /// note-on with no citation and a note-off after the length of the score
    /// note it plays, once the transport has committed every quantum the take
    /// reaches. The law cites every note as the constructed take does, each
    /// score note once, so the rows are the committed rows and the snapshot's
    /// SHA-256 is the committed golden. The lengths are not hashed.
    #[test]
    fn the_constructed_take_played_live_is_the_golden() {
        let root = repo_root();
        let g = golden();
        let committed = GoldenFile::read(&root).unwrap().golden().unwrap();
        assert_eq!(g.golden, committed);
        let notes = wire::decode_take(&g.take).unwrap();
        let score = Law::ingest(&g.container).unwrap().score().clone();
        let length = |n: &TakeNote| {
            n.cites
                .and_then(|id| score.note(id))
                .map_or(1, |s| s.duration_samples)
        };
        let last = notes
            .iter()
            .map(|n| (n.onset_sample + length(n)) / u64::from(QUANTUM_SAMPLES))
            .max()
            .unwrap();

        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        pass(&g.container, abi::law_ingest, "law_ingest").unwrap();
        let steps = last + 1 - u64::from(law::HORIZON_QUANTA);
        for _ in 0..steps {
            assert_eq!(abi::law_step(), 0);
        }
        assert_eq!(
            abi::law_horizon(),
            last,
            "the last note-off's quantum, exactly"
        );
        for n in notes.iter().rev() {
            let status = abi::law_live_note(
                i64::try_from(n.onset_sample).unwrap(),
                u32::from(n.pitch),
                u32::from(n.velocity),
            );
            if status != 0 {
                panic!("{}", refusal("law_live_note", status));
            }
            let status = abi::law_live_note_off(
                u32::from(n.pitch),
                i64::try_from(n.onset_sample + length(n)).unwrap(),
            );
            if status != 0 {
                panic!("{}", refusal("law_live_note_off", status));
            }
        }
        assert_eq!(snapshot_hash().unwrap(), committed);
        let rows = read_out(abi::law_rows_ptr(), abi::law_rows_len());
        assert_eq!(rows, g.rows.as_bytes());
        assert_eq!(
            fs::read(root.join(ROWS_FILE)).unwrap(),
            [rows, b"\n".to_vec()].concat()
        );
    }

    /// The expected verdicts of PHASE-0's constructed take, graded by the law
    /// from the committed inputs, and the rows that state them in digits.
    #[test]
    fn the_constructed_take_grades_as_phase_0_says() {
        let g = golden();
        assert_eq!(g.tier, Tier::PublicDomain);
        assert_eq!(g.score_notes, 2621);
        assert_eq!(g.take_notes, 2621);
        let expected = [
            (Perturbation::Late30, "match", 1_440),
            (Perturbation::Late45, "late", 2_160),
            (Perturbation::Late60, "late", 2_880),
            (Perturbation::Early45, "early", -2_160),
            (Perturbation::WrongPitch, "wrong pitch", 0),
        ];
        assert_eq!(g.drawn.len(), 5);
        let mut notes: Vec<u32> = g.drawn.iter().map(|d| d.note).collect();
        notes.sort_unstable();
        notes.dedup();
        assert_eq!(notes.len(), 5, "five distinct notes");
        for (d, (perturbation, verdict, delta)) in g.drawn.iter().zip(expected) {
            assert_eq!(d.perturbation, perturbation);
            assert_eq!(d.verdict, verdict, "{d:?}");
            let played = i64::try_from(d.played_onset).unwrap();
            let score = i64::try_from(d.score_onset).unwrap();
            assert_eq!(played - score, delta);
            let (sign, ms) = match delta {
                1_440 => ('+', "30.0"),
                2_160 => ('+', "45.0"),
                2_880 => ('+', "60.0"),
                -2_160 => ('-', "45.0"),
                _ => ('+', "0.0"),
            };
            let samples = delta.unsigned_abs();
            let pitches = format!("pitch {} vs {}", d.played_pitch, d.score_pitch);
            assert_eq!(
                d.row,
                format!(
                    "note {}: onset {sign}{samples} samples ({sign}{ms} ms) vs gate \u{b1}1920, \
                     {pitches}: {verdict}",
                    d.note
                )
            );
            if perturbation == Perturbation::WrongPitch {
                assert_ne!(d.played_pitch, d.score_pitch);
                let up = d.score_pitch < 127 && d.played_pitch == d.score_pitch + 1;
                let down = d.score_pitch == 127 && d.played_pitch == 126;
                assert!(up || down, "{d:?}");
            } else {
                assert_eq!(d.played_pitch, d.score_pitch);
            }
        }
        // The early note had room to move back.
        assert!(g.drawn[3].score_onset >= 2_160);
        // Every other note matches; nothing is added or left unplayed.
        assert_eq!(
            g.counts,
            Counts {
                matched: 2_617,
                early: 1,
                late: 2,
                wrong_pitch: 1,
                addition: 0,
                never_played: 0,
            }
        );
        let drawn: Vec<u32> = g.drawn.iter().map(|d| d.note).collect();
        let mut rows = 0;
        for (index, row) in g.rows.lines().enumerate() {
            rows += 1;
            let note = u32::try_from(index).unwrap();
            if drawn.contains(&note) {
                continue;
            }
            assert!(
                row.starts_with(&format!(
                    "note {note}: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch "
                )),
                "{row}"
            );
            assert!(row.ends_with(": match"), "{row}");
        }
        assert_eq!(rows, 2621);
    }

    /// The snapshot `write-golden --snapshot` writes out is the one the golden
    /// hashes, and its header holds the law version at byte 12 and the
    /// predicate version at byte 36, little-endian.
    #[test]
    fn the_snapshot_is_what_the_golden_hashes() {
        let g = golden();
        assert_eq!(sha256(&g.snapshot), g.golden);
        assert_eq!(g.snapshot.len(), g.snapshot_bytes);
        assert_eq!(&g.snapshot[..8], b"SIJAMLAW");
        assert_eq!(g.snapshot[12..16], law::LAW_VERSION.to_le_bytes());
        assert_eq!(
            g.snapshot[36..40],
            provenance::PREDICATE_VERSION.to_le_bytes()
        );
    }

    /// Law version 4 changes nothing version 3 computed: its snapshot of the
    /// constructed take, with the law-version word (bytes 12 to 15) written back
    /// to 3, is version 3's snapshot byte for byte, so it hashes to version 3's
    /// golden.
    #[test]
    fn the_version_4_snapshot_is_version_3_but_for_its_version_word() {
        const VERSION_3_GOLDEN: &str =
            "145c7af9f964a47f2e21566dd253ff5de6cc5cd65c6f2cbf5f04afe745c03b40";
        let g = golden();
        let mut bytes = g.snapshot.clone();
        assert_eq!(bytes[12..16], [4, 0, 0, 0]);
        bytes[12] = 3;
        assert_eq!(hex(&sha256(&bytes)), VERSION_3_GOLDEN);
        assert_ne!(hex(&g.golden), VERSION_3_GOLDEN);
    }

    #[test]
    fn the_committed_golden_is_current() {
        let root = repo_root();
        let differences = check(&root, &golden());
        assert!(
            differences.is_empty(),
            "run `cargo run -p golden --bin write-golden`: {differences:#?}"
        );
    }

    /// A one-sample change to one take note changes the take's bytes, the
    /// golden hash and the committed files' check.
    #[test]
    fn a_one_sample_change_to_the_take_moves_the_golden_and_fails_the_check() {
        let inputs = Inputs::read(&repo_root()).unwrap();
        let good = compute(&inputs, SEED, TakeEdit::None).unwrap();
        let moved = compute(&inputs, SEED, TakeEdit::LastNoteOneSampleLater).unwrap();
        assert_ne!(moved.take, good.take);
        assert_eq!(moved.take.len(), good.take.len());
        let changed = moved
            .take
            .iter()
            .zip(&good.take)
            .filter(|(a, b)| a != b)
            .count();
        assert!((1..=8).contains(&changed), "one u64 onset changed");
        assert_ne!(moved.golden, good.golden);
        let differences = check(&repo_root(), &moved);
        assert!(!differences.is_empty());
        let golden_file = differences
            .iter()
            .find(|d| d.path == GOLDEN_FILE)
            .expect("the golden file differs");
        let changed_keys: Vec<&str> = golden_file
            .lines
            .iter()
            .filter_map(|(_, _, now)| now.as_deref())
            .filter_map(|line| line.split(' ').next())
            .collect();
        for key in ["take", "rows", "golden"] {
            assert!(changed_keys.contains(&key), "{key} not in {changed_keys:?}");
        }
    }

    #[test]
    fn the_seed_decides_the_draw_and_the_golden() {
        let inputs = Inputs::read(&repo_root()).unwrap();
        let a = compute(&inputs, SEED, TakeEdit::None).unwrap();
        let b = compute(&inputs, SEED ^ 1, TakeEdit::None).unwrap();
        assert_ne!(a.drawn, b.drawn);
        assert_ne!(a.golden, b.golden);
        assert_eq!(a.counts, b.counts, "any seed makes the same shape of take");
    }

    #[test]
    fn the_golden_file_reads_back() {
        let g = golden();
        let file = GoldenFile::parse(&g.golden_text()).unwrap();
        assert_eq!(file.golden().unwrap(), g.golden);
        assert_eq!(file.one("steps").unwrap(), g.steps.to_string());
        assert_eq!(file.all("input").len(), 3);
        assert_eq!(file.all("drawn").len(), 5);
        assert!(file.one("input").is_err(), "three input lines, not one");
        assert!(file.one("absent").is_err());
        assert_eq!(unhex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("0G"), None);
    }

    #[test]
    fn a_difference_names_its_lines() {
        assert_eq!(difference(GOLDEN_FILE, "a\nb\n", "a\nb\n"), None);
        // A changed line, and a line added at the end: after the last line
        // feed, the regenerated text has one more line than the committed one.
        let d = difference(GOLDEN_FILE, "a\nb\nc\n", "a\nB\nc\nd\n").unwrap();
        assert_eq!(d.count, 3);
        assert_eq!(
            d.lines,
            [
                (2, Some("b".into()), Some("B".into())),
                (4, Some(String::new()), Some("d".into())),
                (5, None, Some(String::new())),
            ]
        );
        assert!(!d.missing);
    }
}
