//! `write-golden`: the only writer of the golden file.
//!
//! ```text
//! cargo run -p golden --bin write-golden             write golden/entertainer.golden,
//!                                                    golden/entertainer.rows, and each
//!                                                    exemplar's golden/<id>.golden and
//!                                                    golden/<id>.frames
//! cargo run -p golden --bin write-golden -- --check  write nothing; exit 1 when
//!                                                    regenerating would change any
//! ... [--check] --snapshot <path>                    also write the law's snapshot,
//!                                                    the bytes the golden hashes, to
//!                                                    <path>, outside the golden files
//! ```
//!
//! The snapshot `--snapshot` writes is the golden's, *The Entertainer* graded
//! against the constructed take; an exemplar's is in its golden file by size
//! and SHA-256.
//!
//! Exit status: 0 when the files were written, or when `--check` finds them
//! current; 1 when `--check` finds a difference; 2 when the harness itself
//! fails (a bad argument, an unreadable input, a refusal by the law, or the
//! law's C ABI and Rust API disagreeing).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use golden::exemplar::{self, EXEMPLARS, ExemplarGolden};
use golden::run::{self, Difference, GOLDEN_FILE, Inputs, ROWS_FILE, hex};
use golden::take::{SEED, TakeEdit};

/// `--check`, and where `--snapshot` writes the snapshot, if anywhere.
fn arguments() -> Option<(bool, Option<PathBuf>)> {
    let mut check = false;
    let mut snapshot = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" if !check => check = true,
            "--snapshot" if snapshot.is_none() => snapshot = Some(PathBuf::from(args.next()?)),
            _ => return None,
        }
    }
    Some((check, snapshot))
}

fn main() -> ExitCode {
    let Some((check, snapshot)) = arguments() else {
        eprintln!("usage: write-golden [--check] [--snapshot <path>]");
        return ExitCode::from(2);
    };
    let root = golden::repo_root();
    let golden = match Inputs::read(&root).and_then(|i| run::compute(&i, SEED, TakeEdit::None)) {
        Ok(golden) => golden,
        Err(e) => {
            eprintln!("write-golden: {e}");
            return ExitCode::from(2);
        }
    };
    let exemplars: Vec<ExemplarGolden> = match EXEMPLARS
        .iter()
        .map(|x| exemplar::compute_at(&root, *x))
        .collect()
    {
        Ok(exemplars) => exemplars,
        Err(e) => {
            eprintln!("write-golden: {e}");
            return ExitCode::from(2);
        }
    };

    if let Some(path) = snapshot {
        if let Err(e) = fs::write(&path, &golden.snapshot) {
            eprintln!("write-golden: {}: {e}", path.display());
            return ExitCode::from(2);
        }
        println!(
            "write-golden: wrote the snapshot ({} bytes, sha256 {}) to {}",
            golden.snapshot.len(),
            hex(&run::sha256(&golden.snapshot)),
            path.display()
        );
    }

    if check {
        let mut differences = run::check(&root, &golden);
        if differences.is_empty() {
            println!(
                "write-golden --check: {GOLDEN_FILE} and {ROWS_FILE} are current; golden {}",
                hex(&golden.golden)
            );
        }
        for x in &exemplars {
            let found = x.check(&root);
            if found.is_empty() {
                println!(
                    "write-golden --check: {} and {} are current; {}",
                    x.exemplar.golden_file,
                    x.exemplar.frames_file,
                    hashes(x)
                );
            }
            differences.extend(found);
        }
        if differences.is_empty() {
            return ExitCode::SUCCESS;
        }
        report(&differences);
        println!(
            "write-golden --check: run `cargo run -p golden --bin write-golden` and review the change"
        );
        return ExitCode::from(1);
    }

    let mut files: Vec<(&str, String)> = golden.files().into();
    for x in &exemplars {
        files.extend(x.files());
    }
    for (path, text) in files {
        let full = root.join(path);
        if let Some(dir) = full.parent()
            && let Err(e) = fs::create_dir_all(dir)
        {
            eprintln!("write-golden: {}: {e}", dir.display());
            return ExitCode::from(2);
        }
        if let Err(e) = fs::write(&full, text) {
            eprintln!("write-golden: {}: {e}", full.display());
            return ExitCode::from(2);
        }
    }
    println!(
        "write-golden: wrote {GOLDEN_FILE} and {ROWS_FILE}; golden {}",
        hex(&golden.golden)
    );
    for x in &exemplars {
        println!(
            "write-golden: wrote {} and {}; {}",
            x.exemplar.golden_file,
            x.exemplar.frames_file,
            hashes(x)
        );
    }
    ExitCode::SUCCESS
}

/// An exemplar's golden hash and the SHA-256 of its frames' bytes.
fn hashes(x: &ExemplarGolden) -> String {
    format!(
        "golden {}, frames-wire {}",
        hex(&x.golden),
        hex(&run::sha256(&x.frames_wire))
    )
}

/// Prints each difference `--check` found, with the lines it names.
fn report(differences: &[Difference]) {
    for d in differences {
        if d.missing {
            println!("write-golden --check: {} is missing", d.path);
            continue;
        }
        println!(
            "write-golden --check: regenerating would change {} ({} lines differ)",
            d.path, d.count
        );
        for (line, committed, now) in &d.lines {
            println!("  line {line}");
            println!(
                "    committed: {}",
                committed.as_deref().unwrap_or("(none)")
            );
            println!("    now:       {}", now.as_deref().unwrap_or("(none)"));
        }
        if d.count > d.lines.len() {
            println!("  and {} more", d.count - d.lines.len());
        }
    }
}
