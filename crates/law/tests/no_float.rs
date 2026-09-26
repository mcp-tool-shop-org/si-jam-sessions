//! No `f32` or `f64` in the law's source, nor in the crates it embeds: the
//! score model, the SMF reader (`ingest`) and the licence predicate
//! (`provenance`).
//!
//! Comments, string literals and char literals are blanked first: text is not
//! a number the code computes with. The rest is scanned, and the scan fails on:
//! - the type names `f32` and `f64`, also as a literal suffix (`1f64`);
//! - a decimal literal with a fraction (`1.5`, `1.`);
//! - a decimal literal with an exponent (`1e3`).
//!
//! Two more guards stand behind this one: Clippy's `float_arithmetic` is
//! denied in the crate, and `tests/wasm_artifact.rs` checks the built binary
//! for any float type or instruction.

mod common;

use common::{is_word, sources, strip};
use std::fs;
use std::path::Path;

/// Every float in `code` (comments already stripped), as `line: text`.
fn floats(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        for i in 0..chars.len() {
            let before = if i == 0 { None } else { Some(chars[i - 1]) };
            // f32 / f64, alone or as a suffix; not inside a longer name.
            if chars[i] == 'f'
                && matches!(chars.get(i + 1..i + 3), Some(['3', '2']) | Some(['6', '4']))
                && !before.is_some_and(|b| b.is_ascii_alphabetic() || b == '_')
                && !chars.get(i + 3).is_some_and(|&a| is_word(a))
            {
                found.push(format!("{}: {}", n + 1, line.trim()));
                continue;
            }
            // A number literal starts at a digit that ends no word or path.
            let starts =
                chars[i].is_ascii_digit() && !before.is_some_and(|b| is_word(b) || b == '.');
            if !starts {
                continue;
            }
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '_') {
                j += 1;
            }
            let fraction = chars.get(j) == Some(&'.')
                && match chars.get(j + 1) {
                    None => true,
                    Some(&a) => a.is_ascii_digit() || !(is_word(a) || a == '.'),
                };
            let exponent = matches!(chars.get(j), Some('e' | 'E'))
                && match chars.get(j + 1) {
                    Some(&a) if a.is_ascii_digit() => true,
                    Some('+' | '-') => chars.get(j + 2).is_some_and(|a| a.is_ascii_digit()),
                    _ => false,
                };
            if fraction || exponent {
                found.push(format!("{}: {}", n + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn the_law_has_no_float() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    sources(&crates.join("law").join("src"), &mut files);
    sources(&crates.join("score-model").join("src"), &mut files);
    sources(&crates.join("ingest").join("src"), &mut files);
    sources(&crates.join("provenance").join("src"), &mut files);
    files.sort();
    assert!(
        files.len() >= 10,
        "found only {} source files: {files:?}",
        files.len()
    );
    let mut found = Vec::new();
    for file in &files {
        let src = fs::read_to_string(file).expect("a UTF-8 source file");
        for hit in floats(&strip(&src)) {
            found.push(format!("{}:{hit}", file.display()));
        }
    }
    assert!(found.is_empty(), "floats in the law:\n{}", found.join("\n"));
}

/// The scanner itself, on lines it must flag and lines it must not.
#[test]
fn the_scanner_finds_what_it_should() {
    for bad in [
        "let x: f64 = 0;",
        "let x = 1.5;",
        "let x = 1.;",
        "let x = (1.);",
        "let x = 2e3;",
        "let x = 2E-3;",
        "let x = 7f32;",
        "let x = y as f32;",
        "let x = 1_000.0;",
        "let c = '\"'; let d: f64 = 0;",
    ] {
        assert_eq!(floats(&strip(bad)).len(), 1, "should flag: {bad}");
    }
    for good in [
        "let x = t.0.1;",
        "for i in 0..5 {}",
        "let x = 1.max(2);",
        "let x = 0x1e3;",
        "let buf64 = 0;",
        "let x = y.checked_add(1);",
        "let x = 3; // a comment with f64 and 1.5",
        "/* f32 /* nested 2e3 */ 1.5 */ let a = 1;",
        "fn f<'a>(x: &'a u8) -> char { '\\u{b1}' }",
        "let s = \"a // not a comment\"; let n = 1;",
        "let row = \"(+45.0 ms) vs gate \\u{b1}1920\";",
        "let raw = r#\"a \"quoted\" 1.5\"#; let n = 2;",
        "let bytes = b\"1.5e3\";",
    ] {
        assert_eq!(
            floats(&strip(good)),
            Vec::<String>::new(),
            "should pass: {good}"
        );
    }
}
