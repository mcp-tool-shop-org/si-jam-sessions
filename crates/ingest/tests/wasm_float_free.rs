//! The crates the law links, built for wasm32 and inspected byte by byte: no floating point.
//!
//! The law links ingest, and provenance's licence predicate, into its wasm, and the law's
//! artifact test (`crates/law/tests/wasm_artifact.rs`) refuses any floating-point type or
//! instruction anywhere in the module. This test builds one small cdylib per crate. Each
//! calls what the law will call:
//! - ingest: `ingest_smf`;
//! - provenance: `Receipt::from_json`, `Receipt::from_canonical` and `admit`, which reads
//!   LilyPond and SMF files.
//!
//! The cdylibs use the workspace's release profile, lockfile and toolchain. Each module is
//! walked with the same technique as the law's test: every signature, global, local and
//! instruction. Code the calls cannot reach is removed by the linker, as it is in the law,
//! so the test sees exactly what linking the crate adds.
//!
//! The cdylibs are generated here rather than kept as crates: ingest and provenance stay
//! rlibs with no exports, so nothing they export can leak into the law's export list.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A path as TOML can hold it in a basic string.
fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

const PROBE_MANIFEST: &str = r#"[package]
name = "@NAME@"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]
path = "lib.rs"

[dependencies]
@CRATE@ = { path = "@PATH@" }

# The workspace's release profile.
[profile.release]
panic = "abort"
overflow-checks = true
codegen-units = 1

[workspace]
"#;

/// Shared by both probes: a byte slice from wasm linear memory.
const BYTES_FN: &str = r#"
fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    unsafe { core::slice::from_raw_parts(ptr, len) }
}
"#;

const INGEST_PROBE: &str = r#"
#[unsafe(no_mangle)]
pub extern "C" fn probe_ingest(ptr: *const u8, len: usize) -> u32 {
    match ingest::ingest_smf(bytes(ptr, len)) {
        Ok(score) => score.notes.len() as u32,
        Err(_) => u32::MAX,
    }
}
"#;

const PROVENANCE_PROBE: &str = r#"
#[unsafe(no_mangle)]
pub extern "C" fn probe_decode(ptr: *const u8, len: usize) -> u32 {
    match provenance::Receipt::from_canonical(bytes(ptr, len)) {
        Ok(receipt) => receipt.files.len() as u32,
        Err(_) => u32::MAX,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_admit(
    receipt_ptr: *const u8,
    receipt_len: usize,
    ly_ptr: *const u8,
    ly_len: usize,
    mid_ptr: *const u8,
    mid_len: usize,
) -> u32 {
    let Ok(receipt) = provenance::Receipt::from_json(bytes(receipt_ptr, receipt_len)) else {
        return u32::MAX;
    };
    let files = [
        provenance::Supplied { name: "a.ly", bytes: bytes(ly_ptr, ly_len) },
        provenance::Supplied { name: "a.mid", bytes: bytes(mid_ptr, mid_len) },
    ];
    match provenance::admit(&receipt, &files) {
        Ok(admitted) => u32::from(admitted.receipt_digest[0]),
        Err(_) => u32::MAX - 1,
    }
}
"#;

/// Builds a cdylib that depends on one workspace crate and holds `source`, and returns the
/// wasm module's bytes.
fn build_probe(name: &str, krate: &str, source: &str) -> Vec<u8> {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let dir = tmp.join(name);
    std::fs::create_dir_all(&dir).expect("the probe directory");
    let manifest = PROBE_MANIFEST
        .replace("@NAME@", name)
        .replace("@CRATE@", krate)
        .replace(
            "@PATH@",
            &toml_path(&workspace().join("crates").join(krate)),
        );
    std::fs::write(dir.join("Cargo.toml"), manifest).expect("the probe manifest");
    std::fs::write(dir.join("lib.rs"), [BYTES_FN, source].concat()).expect("the probe source");
    // The workspace's lockfile, so the probe builds the dependency versions the law does.
    // Cargo adds the probe itself and drops what it does not use; with `--offline` it can
    // change no locked version.
    std::fs::copy(workspace().join("Cargo.lock"), dir.join("Cargo.lock")).expect("the lockfile");
    std::fs::copy(
        workspace().join("rust-toolchain.toml"),
        dir.join("rust-toolchain.toml"),
    )
    .expect("the toolchain pin");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    // One target directory for both probes, so they share compiled dependencies.
    let target = tmp.join("wasm-probe-target");
    let status = Command::new(cargo)
        .current_dir(&dir)
        .args([
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--offline",
            "--quiet",
            "--target-dir",
        ])
        .arg(&target)
        .status()
        .expect("cargo runs");
    assert!(status.success(), "the {name} wasm build failed");
    let file = format!("{}.wasm", name.replace('-', "_"));
    let path = target.join("wasm32-unknown-unknown/release").join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The ingest probe, built once for all the tests in this file.
fn ingest_artifact() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| build_probe("ingest-wasm-probe", "ingest", INGEST_PROBE))
}

/// The provenance probe, built once for all the tests in this file.
fn provenance_artifact() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| build_probe("provenance-wasm-probe", "provenance", PROVENANCE_PROBE))
}

// The wasm reader and the float walker below follow crates/law/tests/wasm_artifact.rs.

/// A cursor over a byte slice, reading the wasm binary format.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn byte(&mut self) -> u8 {
        let b = self.bytes[self.pos];
        self.pos += 1;
        b
    }

    fn bytes(&mut self, n: usize) -> &'a [u8] {
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        s
    }

    /// An unsigned LEB128 of up to 32 bits.
    fn u32(&mut self) -> u32 {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let b = self.byte();
            result |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            assert!(shift < 35, "a u32 LEB128 longer than five bytes");
        }
        u32::try_from(result).expect("a u32")
    }

    fn usize(&mut self) -> usize {
        usize::try_from(self.u32()).expect("fits usize")
    }

    /// A signed LEB128 (i32, i64 or s33): skipped, its value unused here.
    fn skip_signed(&mut self) {
        while self.byte() & 0x80 != 0 {}
    }

    fn name(&mut self) -> String {
        let len = self.usize();
        String::from_utf8(self.bytes(len).to_vec()).expect("a UTF-8 name")
    }
}

/// The module's sections as `(id, contents)`, after checking the header.
fn sections(module: &[u8]) -> Vec<(u8, &[u8])> {
    let mut r = Reader::new(module);
    assert_eq!(r.bytes(4), b"\0asm", "the wasm magic");
    assert_eq!(r.bytes(4), [1, 0, 0, 0], "wasm binary version 1");
    let mut out = Vec::new();
    while !r.done() {
        let id = r.byte();
        let len = r.usize();
        out.push((id, r.bytes(len)));
    }
    out
}

const F32: u8 = 0x7D;
const F64: u8 = 0x7C;

fn is_float(valtype: u8) -> bool {
    valtype == F32 || valtype == F64
}

/// The float opcodes of the wasm 1.0 instruction set: loads, stores, constants,
/// comparisons, arithmetic and conversions that take or give an f32 or f64.
fn is_float_opcode(op: u8) -> bool {
    matches!(op,
        0x2A | 0x2B | 0x38 | 0x39 // f32/f64 load and store
        | 0x43 | 0x44            // f32/f64 const
        | 0x5B..=0x66            // f32/f64 comparisons
        | 0x8B..=0xA6            // f32/f64 arithmetic
        | 0xA8..=0xAB            // i32.trunc_f32/f64
        | 0xAE..=0xBF            // i64.trunc, converts, demote, promote, reinterprets
    )
}

/// Walks one function body instruction by instruction and returns the float opcodes it
/// finds. Any opcode this walker does not know fails the test, so a new instruction
/// cannot slip past it.
fn float_instructions(body: &[u8]) -> Vec<String> {
    let mut r = Reader::new(body);
    let mut found = Vec::new();
    for _ in 0..r.u32() {
        let _count = r.u32();
        let valtype = r.byte();
        if is_float(valtype) {
            found.push(format!("a local of type 0x{valtype:02X}"));
        }
    }
    while !r.done() {
        let op = r.byte();
        if is_float_opcode(op) {
            found.push(format!("opcode 0x{op:02X}"));
        }
        match op {
            0x00 | 0x01 | 0x05 | 0x0B | 0x0F | 0x1A | 0x1B | 0xD1 => {}
            0x45..=0xC4 => {}
            0x02..=0x04 => {
                // Block type: 0x40, a value type, or an s33 type index.
                let b = r.bytes[r.pos];
                if b == 0x40 || (0x6F..=0x7F).contains(&b) {
                    if is_float(b) {
                        found.push(format!("a block of type 0x{b:02X}"));
                    }
                    r.pos += 1;
                } else {
                    r.skip_signed();
                }
            }
            0x0C | 0x0D | 0x10 | 0x20..=0x26 | 0xD2 => {
                r.u32();
            }
            0x0E => {
                for _ in 0..=r.u32() {
                    r.u32();
                }
            }
            0x11 => {
                r.u32();
                r.u32();
            }
            0x1C => {
                for _ in 0..r.u32() {
                    let t = r.byte();
                    if is_float(t) {
                        found.push(format!("a select of type 0x{t:02X}"));
                    }
                }
            }
            0x28..=0x3E => {
                r.u32();
                r.u32();
            }
            0x3F | 0x40 | 0xD0 => {
                r.byte();
            }
            0x41 | 0x42 => r.skip_signed(),
            0x43 => {
                r.bytes(4);
            }
            0x44 => {
                r.bytes(8);
            }
            0xFC => match r.u32() {
                sub @ 0..=7 => found.push(format!("opcode 0xFC {sub} (trunc_sat)")),
                8 => {
                    r.u32();
                    r.byte();
                }
                9 | 13 | 15..=17 => {
                    r.u32();
                }
                10 => {
                    r.byte();
                    r.byte();
                }
                11 => {
                    r.byte();
                }
                12 | 14 => {
                    r.u32();
                    r.u32();
                }
                sub => panic!("unknown opcode 0xFC {sub}"),
            },
            other => panic!("unknown opcode 0x{other:02X}"),
        }
    }
    found
}

/// Every float type or instruction in the module, and the number of function bodies.
fn floats_in(module: &[u8]) -> (Vec<String>, usize) {
    let mut found = Vec::new();
    let mut bodies = 0;
    for (id, body) in sections(module) {
        let mut r = Reader::new(body);
        match id {
            // Types: every parameter and result of every signature.
            1 => {
                for _ in 0..r.u32() {
                    assert_eq!(r.byte(), 0x60, "a function type");
                    for _ in 0..2 {
                        for _ in 0..r.u32() {
                            let t = r.byte();
                            if is_float(t) {
                                found.push(format!("a signature with 0x{t:02X}"));
                            }
                        }
                    }
                }
            }
            // Globals: the type of each, and its initialiser.
            6 => {
                for _ in 0..r.u32() {
                    let t = r.byte();
                    if is_float(t) {
                        found.push(format!("a global of type 0x{t:02X}"));
                    }
                    let _mutable = r.byte();
                    match r.byte() {
                        0x41 | 0x42 => r.skip_signed(),
                        0x23 | 0xD2 => {
                            r.u32();
                        }
                        0xD0 => {
                            r.byte();
                        }
                        op @ (0x43 | 0x44) => {
                            found.push(format!("a global initialised by opcode 0x{op:02X}"));
                            r.bytes(if op == 0x43 { 4 } else { 8 });
                        }
                        op => panic!("unknown constant opcode 0x{op:02X}"),
                    }
                    assert_eq!(r.byte(), 0x0B, "a global's initialiser ends");
                }
            }
            // Code: every local and every instruction of every body.
            10 => {
                for _ in 0..r.u32() {
                    let len = r.usize();
                    found.extend(float_instructions(r.bytes(len)));
                    bodies += 1;
                }
            }
            _ => {}
        }
    }
    (found, bodies)
}

/// The names in a module's export section.
fn exports(module: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for (id, body) in sections(module) {
        if id != 7 {
            continue;
        }
        let mut r = Reader::new(body);
        for _ in 0..r.u32() {
            names.push(r.name());
            let _kind = r.byte();
            let _index = r.u32();
        }
    }
    names
}

#[test]
fn the_probes_export_what_the_law_will_call() {
    let ingest = exports(ingest_artifact());
    assert!(ingest.iter().any(|n| n == "probe_ingest"), "{ingest:?}");
    let provenance = exports(provenance_artifact());
    for export in ["probe_decode", "probe_admit"] {
        assert!(
            provenance.iter().any(|n| n == export),
            "{export} in {provenance:?}"
        );
    }
}

#[test]
fn ingest_links_no_floating_point() {
    let (found, bodies) = floats_in(ingest_artifact());
    assert!(bodies > 0, "the module has function bodies");
    assert!(
        found.is_empty(),
        "floating point linked by ingest ({} occurrences): {found:?}",
        found.len()
    );
}

#[test]
fn the_licence_predicate_links_no_floating_point() {
    let (found, bodies) = floats_in(provenance_artifact());
    assert!(bodies > 0, "the module has function bodies");
    assert!(
        found.is_empty(),
        "floating point linked by provenance ({} occurrences): {found:?}",
        found.len()
    );
}

/// The walker itself: a body with one of each shape it must skip, and ones with a float,
/// so a walker that stopped decoding would fail here.
#[test]
fn the_instruction_walker_finds_what_it_should() {
    // No locals; i32.const 5; i64.const -1; block (empty) end; drop; end.
    let clean = [0x00, 0x41, 0x05, 0x42, 0x7F, 0x02, 0x40, 0x0B, 0x1A, 0x0B];
    assert_eq!(float_instructions(&clean), Vec::<String>::new());
    // One f64 local; f64.const 1.0; drop; end.
    let float = [
        0x01, 0x01, 0x7C, 0x44, 0, 0, 0, 0, 0, 0, 0xF0, 0x3F, 0x1A, 0x0B,
    ];
    assert_eq!(
        float_instructions(&float),
        ["a local of type 0x7C", "opcode 0x44"]
    );
    // f32.convert_i32_u; i32.trunc_sat_f32_u: the shape midly's capacity estimate had.
    let estimate = [0x00, 0x20, 0x00, 0xB3, 0xFC, 0x01, 0x1A, 0x0B];
    assert_eq!(
        float_instructions(&estimate),
        ["opcode 0xB3", "opcode 0xFC 1 (trunc_sat)"]
    );
}
