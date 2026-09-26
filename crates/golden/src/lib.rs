//! The golden-hash harness of si-jam-sessions slice 1.
//!
//! The golden hash is the SHA-256 of the law's snapshot after it has ingested
//! the committed score (*The Entertainer*, through its receipt and the
//! licence predicate) and graded the constructed take against it. The law is
//! deterministic integer code, so the same inputs must give the same hash
//! natively on x86_64 and ARM64 and as wasm under V8, SpiderMonkey and
//! JavaScriptCore. This crate computes it, records it, and checks it:
//!
//! - [`take`]: the constructed take PHASE-0 describes, placed by
//!   [`prng::SplitMix64`] from [`take::SEED`];
//! - [`run`]: the inputs, one run of the law through its C ABI and one through
//!   its Rust API (they must agree), and the golden file's text;
//! - [`engine_js`]: the script a JavaScript engine runs to compute the same
//!   hash from the same bytes;
//! - [`exemplar`]: each exemplar's golden, what the law commits when the
//!   arrangement plays with no take: its frames and its snapshot's hash;
//! - `write-golden`: the only writer of `golden/entertainer.golden`,
//!   `golden/entertainer.rows` and each exemplar's `golden/<id>.golden` and
//!   `golden/<id>.frames`; with `--check` it writes nothing and exits 1 when
//!   regenerating would change any of them;
//! - `engine-js`: writes the engine script for a given wasm, after checking
//!   that its inputs are the golden file's.
//!
//! # Standards compliance
//!
//! The studio's six workflow standards, scored 0-3 for this pipeline
//! (write-golden, engine-js and the CI jobs that run them).
//!
//! - **PIN_PER_STEP 2.** The law version, snapshot format, PPQ, rate, Q, H,
//!   the gate, the predicate's version and cut-off years, every input's
//!   SHA-256, the seed and the PRNG are in the golden file; the pins are also
//!   in the snapshot's header, so changing one moves the golden hash (the
//!   mutation proofs change H and one take sample and watch `--check` fail).
//!   CI pins the toolchain, the lockfile (`--locked`), every action by commit
//!   SHA, and node and the two engine shells by version and by the SHA-256 of
//!   every file they run (`.github/engines/`), checked before they run.
//! - **ANDON_AUTHORITY 2.** `--check` and every engine run stop on the first
//!   difference and name it; the native run also stops when the C ABI and the
//!   Rust API disagree or when stepping moves the hash. Negative controls in
//!   the tests and in CI prove each check can fail.
//! - **NAMED_COMPENSATORS.** Nothing here is irreversible: `write-golden`
//!   rewrites two files in the working tree, which git restores, and CI
//!   publishes nothing. There is nothing to undo.
//! - **DECOMPOSE_BY_SECRETS 2.** The law takes bytes and returns a status and
//!   a hash; its wasm imports nothing (the law's own test), so the harness
//!   and the engines hand it bytes and read bytes back. No secret is read.
//! - **UNCERTAINTY_GATED_HUMANS 1.** skip: a golden either matches or does
//!   not; there is no uncertain output to route to a person. A person reviews
//!   every change to the golden files in a pull request.
//! - **EXTERNAL_VERIFIER 2.** No model grades anything: five independent
//!   executions (two native architectures, three JavaScript engines) must
//!   agree with the committed file. An engine's output is checked against
//!   sources it cannot see: the golden file, and the artifact's own SHA-256,
//!   which the engine must reproduce from the bytes it compiled. The code is
//!   reviewed by a different model family.

use std::fmt;
use std::path::{Path, PathBuf};

pub mod engine_js;
pub mod exemplar;
pub mod prng;
pub mod run;
pub mod take;

/// A failure of the harness, as a message for the person running it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// The repository root: two directories above this crate's manifest.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}
