//! `write-golden`: the only writer of the golden file.
//!
//! ```text
//! cargo run -p golden --bin write-golden             write golden/entertainer.golden
//!                                                    and golden/entertainer.rows
//! cargo run -p golden --bin write-golden -- --check  write nothing; exit 1 when
//!                                                    regenerating would change either
//! ```
//!
//! Exit status: 0 when the files were written, or when `--check` finds them
//! current; 1 when `--check` finds a difference; 2 when the harness itself
//! fails (a bad argument, an unreadable input, a refusal by the law, or the
//! law's C ABI and Rust API disagreeing).

use std::fs;
use std::process::ExitCode;

use golden::run::{self, GOLDEN_FILE, Inputs, ROWS_FILE, hex};
use golden::take::{SEED, TakeEdit};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = match args.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            eprintln!("usage: write-golden [--check]");
            return ExitCode::from(2);
        }
    };
    let root = golden::repo_root();
    let golden = match Inputs::read(&root).and_then(|i| run::compute(&i, SEED, TakeEdit::None)) {
        Ok(golden) => golden,
        Err(e) => {
            eprintln!("write-golden: {e}");
            return ExitCode::from(2);
        }
    };

    if check {
        let differences = run::check(&root, &golden);
        if differences.is_empty() {
            println!(
                "write-golden --check: {GOLDEN_FILE} and {ROWS_FILE} are current; golden {}",
                hex(&golden.golden)
            );
            return ExitCode::SUCCESS;
        }
        for d in &differences {
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
        println!(
            "write-golden --check: run `cargo run -p golden --bin write-golden` and review the change"
        );
        return ExitCode::from(1);
    }

    for (path, text) in golden.files() {
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
    ExitCode::SUCCESS
}
