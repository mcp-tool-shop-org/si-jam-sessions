//! No `f32` or `f64` in the law's source, nor in the score model it embeds.
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

use std::fs;
use std::path::{Path, PathBuf};

/// Blanks every comment, the contents of every string literal (raw strings
/// included) and every char literal, keeping line numbers. Nested block
/// comments are handled; a `'` that does not close a char literal is a
/// lifetime and is kept.
fn strip(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r'
            && !(i > 0 && is_word(chars[i - 1]) && chars[i - 1] != 'b')
            && matches!(next, Some('"' | '#'))
        {
            // A raw string: r"..." or r#"..."#, with no escapes.
            let mut hashes = 0;
            let mut j = i + 1;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            let body = j + 1;
            let end = (body..chars.len())
                .find(|&k| chars[k] == '"' && (1..=hashes).all(|h| chars.get(k + h) == Some(&'#')));
            let stop = end.map_or(chars.len(), |k| k + 1 + hashes);
            for &ch in &chars[i..stop] {
                out.push(blank(ch));
            }
            i = stop;
        } else if c == '"' {
            // A string literal: its contents are blanked, escapes included.
            out.push(c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            if i < chars.len() {
                out.push('"');
                i += 1;
            }
        } else if c == '\'' {
            // 'x' or an escape like '\n' or '\u{b1}' is a char literal;
            // anything else ('a, '_, 'static) is a lifetime.
            let close = if next == Some('\\') {
                (i + 2..chars.len()).find(|&j| chars[j] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match close {
                Some(end) => {
                    out.push_str("' '");
                    for _ in i + 3..=end {
                        out.push(' ');
                    }
                    i = end + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

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

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_law_has_no_float() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    sources(&crates.join("law").join("src"), &mut files);
    sources(&crates.join("score-model").join("src"), &mut files);
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
