//! `engine-js`: writes the script a JavaScript engine runs to compute the
//! golden hash from a given build of the law's wasm.
//!
//! ```text
//! cargo run -p golden --bin engine-js -- --wasm <law.wasm> --out <file.js> [--negative-control]
//! cargo run -p golden --bin engine-js -- --wasm <law.wasm> --out <file.js> --exemplar <id>
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
//! With `--exemplar <id>`, the script checks one of the exemplar scores
//! (currently `battle-hymn-glm-5.3` or `battle-hymn-kimi-k3`) with no take.
//!
//! Exit status: 0 when the script was written; 2 on any failure.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use golden::Error;
use golden::engine_js::{self, ExemplarScript, Role, Script};
use golden::exemplar::{self, EXEMPLARS};
use golden::run::{self, GoldenFile, Inputs, hex, sha256};
use golden::take::{SEED, TakeEdit};

enum Mode {
    Golden { role: Role },
    Exemplar { id: String },
}

struct Args {
    wasm: PathBuf,
    out: PathBuf,
    mode: Mode,
}

fn args() -> Result<Args, Error> {
    let mut wasm = None;
    let mut out = None;
    let mut role = Role::Check;
    let mut exemplar = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wasm" => wasm = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--negative-control" => role = Role::NegativeControl,
            "--exemplar" => exemplar = args.next(),
            other => return Err(Error::new(format!("unknown argument {other}"))),
        }
    }
    let mode = match exemplar {
        Some(id) => {
            if role != Role::Check {
                return Err(Error::new(
                    "--exemplar and --negative-control are mutually exclusive",
                ));
            }
            Mode::Exemplar { id }
        }
        None => Mode::Golden { role },
    };
    match (wasm, out) {
        (Some(wasm), Some(out)) => Ok(Args { wasm, out, mode }),
        _ => Err(Error::new(
            "usage: engine-js --wasm <law.wasm> --out <file.js> [--negative-control | --exemplar <id>]",
        )),
    }
}

fn run() -> Result<(), Error> {
    let args = args()?;
    let root = golden::repo_root();
    let wasm =
        fs::read(&args.wasm).map_err(|e| Error::new(format!("{}: {e}", args.wasm.display())))?;

    match args.mode {
        Mode::Golden { role } => run_golden(&args, &root, &wasm, role),
        Mode::Exemplar { ref id } => run_exemplar(&args, &root, &wasm, id),
    }
}

fn run_golden(args: &Args, root: &std::path::Path, wasm: &[u8], role: Role) -> Result<(), Error> {
    let committed = GoldenFile::read(root)?;
    let edit = match role {
        Role::Check => TakeEdit::None,
        Role::NegativeControl => TakeEdit::LastNoteOneSampleLater,
        Role::Exemplar => unreachable!(),
    };
    let golden = run::compute(&Inputs::read(root)?, SEED, edit)?;
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
    if role == Role::Check {
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

    let script = engine_js::render(&Script {
        role,
        wasm,
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
        hex(&sha256(wasm))
    );
    match role {
        Role::Check => println!(
            "engine-js: it must print golden {}",
            hex(&committed.golden()?)
        ),
        Role::NegativeControl => {
            println!("engine-js: a negative control, one take onset one sample late: it must fail")
        }
        Role::Exemplar => unreachable!(),
    }
    Ok(())
}

fn run_exemplar(args: &Args, root: &std::path::Path, wasm: &[u8], id: &str) -> Result<(), Error> {
    let exemplar = EXEMPLARS.iter().find(|x| x.id == id).ok_or_else(|| {
        Error::new(format!(
            "unknown exemplar `{id}`; known: battle-hymn-glm-5.3, battle-hymn-kimi-k3"
        ))
    })?;
    let inputs = Inputs::read_dir(root, exemplar.dir)?;
    let computed = exemplar::compute(&inputs, *exemplar)?;
    let committed_text = fs::read_to_string(root.join(exemplar.golden_file))
        .map_err(|e| Error::new(format!("{}: {e}", exemplar.golden_file)))?;
    let committed = GoldenFile::parse(&committed_text)?;
    let regenerated = GoldenFile::parse(&computed.golden_text())?;

    for key in [
        "law-version",
        "snapshot-format",
        "input",
        "container",
        "score-end",
        "tail-samples",
        "steps",
        "window",
    ] {
        if committed.all(key) != regenerated.all(key) {
            return Err(Error::new(format!(
                "the committed exemplar golden file's `{key}` is not what the inputs give now; run \
                 `cargo run -p golden --bin write-golden -- --check`"
            )));
        }
    }
    let steps: u64 = committed
        .one("steps")?
        .parse()
        .map_err(|_| Error::new("the exemplar golden file's steps is not a number"))?;

    let script = engine_js::render_exemplar(&ExemplarScript {
        wasm,
        container: &computed.container,
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
        hex(&sha256(wasm))
    );
    println!(
        "engine-js: exemplar `{id}` must print golden {}",
        hex(&committed.golden()?)
    );
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
