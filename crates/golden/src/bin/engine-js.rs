//! `engine-js`: writes the script a JavaScript engine runs to compute the
//! golden hash from a given build of the law's wasm.
//!
//! ```text
//! cargo run -p golden --bin engine-js -- --wasm <law.wasm> --out <file.js> [--negative-control]
//! ```
//!
//! It computes the container, the take and the step count from the committed
//! inputs, and refuses unless they are the ones the committed golden file
//! records: a stale golden file is `write-golden`'s to fix, not this tool's.
//! The golden hash it embeds is read from the committed file, never
//! recomputed, so an engine is compared with the file itself.
//!
//! With `--negative-control`, the take's last note is one sample late
//! ([`golden::take::TakeEdit::LastNoteOneSampleLater`]). That script must
//! fail: CI runs it to show each engine's check can go red.
//!
//! Exit status: 0 when the script was written; 2 on any failure.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use golden::Error;
use golden::engine_js::{self, Role, Script};
use golden::run::{self, GoldenFile, Inputs, hex, sha256};
use golden::take::{SEED, TakeEdit};

struct Args {
    wasm: PathBuf,
    out: PathBuf,
    role: Role,
}

fn args() -> Result<Args, Error> {
    let mut wasm = None;
    let mut out = None;
    let mut role = Role::Check;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wasm" => wasm = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--negative-control" => role = Role::NegativeControl,
            other => return Err(Error::new(format!("unknown argument {other}"))),
        }
    }
    match (wasm, out) {
        (Some(wasm), Some(out)) => Ok(Args { wasm, out, role }),
        _ => Err(Error::new(
            "usage: engine-js --wasm <law.wasm> --out <file.js> [--negative-control]",
        )),
    }
}

fn run() -> Result<(), Error> {
    let args = args()?;
    let root = golden::repo_root();
    let committed = GoldenFile::read(&root)?;
    let edit = match args.role {
        Role::Check => TakeEdit::None,
        Role::NegativeControl => TakeEdit::LastNoteOneSampleLater,
    };
    let golden = run::compute(&Inputs::read(&root)?, SEED, edit)?;
    let regenerated = GoldenFile::parse(&golden.golden_text())?;

    // The engine must get the golden's own inputs. Only a negative control
    // runs a different take.
    let mut keys = vec![
        "law-version",
        "snapshot-format",
        "input",
        "container",
        "seed",
        "steps",
    ];
    if args.role == Role::Check {
        keys.push("take");
    }
    for key in keys {
        if committed.all(key) != regenerated.all(key) {
            return Err(Error::new(format!(
                "the committed golden file's `{key}` is not what the inputs give now; run \
                 `cargo run -p golden --bin write-golden -- --check`"
            )));
        }
    }
    let steps: u64 = committed
        .one("steps")?
        .parse()
        .map_err(|_| Error::new("the golden file's steps is not a number"))?;

    let wasm =
        fs::read(&args.wasm).map_err(|e| Error::new(format!("{}: {e}", args.wasm.display())))?;
    let script = engine_js::render(&Script {
        role: args.role,
        wasm: &wasm,
        container: &golden.container,
        take: &golden.take,
        steps,
        law_version: law::LAW_VERSION,
        golden: committed.golden()?,
    });
    fs::write(&args.out, &script)
        .map_err(|e| Error::new(format!("{}: {e}", args.out.display())))?;
    println!(
        "engine-js: wrote {} ({} bytes, sha256 {})",
        args.out.display(),
        script.len(),
        hex(&sha256(script.as_bytes()))
    );
    println!(
        "engine-js: it embeds {} ({} bytes, sha256 {})",
        args.wasm.display(),
        wasm.len(),
        hex(&sha256(&wasm))
    );
    match args.role {
        Role::Check => println!(
            "engine-js: it must print golden {}",
            hex(&committed.golden()?)
        ),
        Role::NegativeControl => {
            println!("engine-js: a negative control, one take onset one sample late: it must fail")
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("engine-js: {e}");
            ExitCode::from(2)
        }
    }
}
