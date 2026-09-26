//! The release wasm law, inspected byte by byte.
//!
//! The test builds the artifact with the command CI uses,
//! `cargo build -p law --target wasm32-unknown-unknown --release --locked`,
//! then reads the module with a parser written here (no dependency), and
//! asserts three things:
//!
//! 1. The import section is empty: the law asks the host for nothing, so it
//!    cannot reach a clock, a file, a device or the network.
//! 2. The exports are exactly the C ABI in `src/abi.rs`, and linear memory.
//! 3. No floating-point value type or instruction appears anywhere: not in a
//!    signature, a local, a global, or a function body. This covers the code
//!    std and the dependencies link in, not just the law's own source.
//!
//! CI also checks (1) with the JavaScript engine's own parser, on the artifact
//! its build step produced.

use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
}

/// Builds the release artifact once for all the tests in this file.
fn artifact() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let target = target_dir();
        let status = Command::new(cargo)
            .current_dir(workspace())
            .args([
                "build",
                "-p",
                "law",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "--locked",
                "--quiet",
                "--target-dir",
            ])
            .arg(&target)
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the wasm build failed");
        let path = target.join("wasm32-unknown-unknown/release/law.wasm");
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    })
}

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

#[test]
fn the_release_wasm_imports_nothing() {
    let imports: Vec<&[u8]> = sections(artifact())
        .into_iter()
        .filter(|(id, _)| *id == 2)
        .map(|(_, body)| body)
        .collect();
    for body in &imports {
        let count = Reader::new(body).u32();
        assert_eq!(count, 0, "the import section has {count} entries");
    }
    assert!(imports.len() <= 1);
}

#[test]
fn the_release_wasm_exports_the_c_abi() {
    let mut names = BTreeSet::new();
    for (id, body) in sections(artifact()) {
        if id != 7 {
            continue;
        }
        let mut r = Reader::new(body);
        for _ in 0..r.u32() {
            let name = r.name();
            let _kind = r.byte();
            let _index = r.u32();
            assert!(names.insert(name), "an export is named twice");
        }
    }
    let expected: BTreeSet<String> = [
        "memory",
        "law_version",
        "law_alloc",
        "law_free",
        "law_ingest",
        "law_load_score",
        "law_admit_take",
        "law_live_note",
        "law_live_note_off",
        "law_step",
        "law_steps",
        "law_horizon",
        "law_snapshot",
        "law_snapshot_ptr",
        "law_snapshot_len",
        "law_hash_ptr",
        "law_rows_ptr",
        "law_rows_len",
        "law_refusal_ptr",
        "law_refusal_len",
        "law_frames",
        "law_frames_ptr",
        "law_frames_len",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(names, expected);
}

/// The float opcodes of the wasm 1.0 instruction set: loads, stores,
/// constants, comparisons, arithmetic and conversions that take or give an
/// f32 or f64.
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

/// Walks one function body instruction by instruction and returns the float
/// opcodes it finds. Any opcode this walker does not know fails the test, so a
/// new instruction cannot slip past it.
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

#[test]
fn the_release_wasm_has_no_floating_point() {
    let mut found = Vec::new();
    let mut bodies = 0;
    for (id, body) in sections(artifact()) {
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
            // Globals: the type of each.
            6 => {
                for _ in 0..r.u32() {
                    let t = r.byte();
                    if is_float(t) {
                        found.push(format!("a global of type 0x{t:02X}"));
                    }
                    let _mutable = r.byte();
                    // The initialiser: one constant instruction, then end.
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
    assert!(bodies > 0, "the module has function bodies");
    assert!(found.is_empty(), "floating point in the law: {found:?}");
}

/// The walker itself: a body with one of each shape it must skip, and one with
/// a float, so a walker that stopped decoding would fail here.
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
    // i32.trunc_sat_f64_s through the 0xFC prefix.
    let saturating = [0x00, 0xFC, 0x02, 0x0B];
    assert_eq!(
        float_instructions(&saturating),
        ["opcode 0xFC 2 (trunc_sat)"]
    );
}
