//! Source-scan helpers shared by the law's source guards (`no_float.rs` and
//! `std_binding.rs`). Each integration test is its own crate, so a guard
//! declares `mod common;` to use them.

// Each guard uses a subset of the helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// Blanks every comment, the contents of every string literal (raw strings
/// included) and every char literal, keeping line numbers. Nested block
/// comments are handled; a `'` that does not close a char literal is a
/// lifetime and is kept.
pub fn strip(src: &str) -> String {
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

pub fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a source directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}
