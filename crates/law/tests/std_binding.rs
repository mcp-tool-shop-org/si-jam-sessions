//! `std` reaches the law only unnamed.
//!
//! The law is `#![no_std]` and links std once, as `extern crate std as _;`, for
//! its wasm32 allocator and its trapping panic handler. Bound to `_`, std has no
//! name, so no path in the law can reach a clock, a file or a thread.
//!
//! That boundary is only as strong as review: a named binding
//! (`extern crate std;` or `extern crate std as x;`) inside any module gives
//! that module the whole of std, and it still compiles. This guard fails the
//! build on such a binding anywhere in the law or the score model, except inside
//! a `#[cfg(test)]` item, where test code may use std freely.
//!
//! Comments, string literals and char literals are blanked first, so text that
//! mentions `extern crate std` is not a binding.

mod common;

use common::{is_word, sources, strip};
use std::fs;
use std::path::Path;

/// True when an attribute's text, whitespace removed, is exactly `cfg(test)`.
fn is_cfg_test(attribute: &str) -> bool {
    let compact: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    compact == "#[cfg(test)]" || compact == "#![cfg(test)]"
}

/// Every named `extern crate std` in `code` (comments and strings already
/// blanked) that is outside a `#[cfg(test)]` scope, as `line: text`.
fn named_std_outside_tests(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    // Attributes seen since the last item boundary; they apply to the next item.
    let mut pending: Vec<String> = Vec::new();
    // One entry per open brace: is that scope test-only?
    let mut scopes: Vec<bool> = Vec::new();
    // An inner `#![cfg(test)]` at the top of the file makes the file test-only.
    let mut file_is_test = false;
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '#' && matches!(chars.get(i + 1), Some('[' | '!')) {
            // An attribute: #[...] or #![...], brackets may nest.
            let start = i;
            let mut depth = 0;
            while i < chars.len() {
                match chars[i] {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    '\n' => line += 1,
                    _ => {}
                }
                i += 1;
            }
            let attribute: String = chars[start..i].iter().collect();
            if attribute.starts_with("#!") {
                if scopes.is_empty() && is_cfg_test(&attribute) {
                    file_is_test = true;
                }
            } else {
                pending.push(attribute);
            }
        } else if c == '{' {
            let inherited = scopes.last().copied().unwrap_or(file_is_test);
            scopes.push(inherited || pending.iter().any(|a| is_cfg_test(a)));
            pending.clear();
            i += 1;
        } else if c == '}' {
            scopes.pop();
            pending.clear();
            i += 1;
        } else if c == ';' {
            pending.clear();
            i += 1;
        } else if c == 'e'
            && !(i > 0 && is_word(chars[i - 1]))
            && chars[i..].starts_with(&['e', 'x', 't', 'e', 'r', 'n'])
            && !chars.get(i + 6).is_some_and(|&a| is_word(a))
        {
            // `extern`, then maybe `crate std`: read the item up to its `;`.
            let end = (i..chars.len())
                .find(|&k| chars[k] == ';')
                .unwrap_or(chars.len());
            let item: String = chars[i..end].iter().collect();
            let words: Vec<&str> = item.split_whitespace().collect();
            let is_std = words.len() >= 3 && words[1] == "crate" && words[2] == "std";
            let unnamed = words.len() == 5 && words[3] == "as" && words[4] == "_";
            if is_std && !unnamed {
                let in_test = scopes.last().copied().unwrap_or(file_is_test)
                    || pending.iter().any(|a| is_cfg_test(a));
                if !in_test {
                    let text = lines.get(line - 1).map_or("", |l| l.trim());
                    found.push(format!("{line}: {text}"));
                }
            }
            i += 6;
        } else {
            i += 1;
        }
    }
    found
}

#[test]
fn std_is_bound_only_unnamed_outside_tests() {
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
        for hit in named_std_outside_tests(&strip(&src)) {
            found.push(format!("{}:{hit}", file.display()));
        }
    }
    assert!(
        found.is_empty(),
        "std is bound by name outside #[cfg(test)]:\n{}",
        found.join("\n")
    );
}

/// The scanner itself, on sources it must flag and sources it must not.
#[test]
fn the_scanner_finds_what_it_should() {
    for bad in [
        "extern crate std;",
        "extern crate std as s;",
        "mod a { extern crate std; }",
        "pub mod a { mod b { extern crate  std ; } }",
        "#[cfg(not(test))] mod m { extern crate std; }",
        "#[cfg(test)] fn f() {} extern crate std;",
        "#[cfg(test)] mod t {} mod u { extern crate std; }",
        "#[cfg(test)] use x; extern crate std;",
    ] {
        assert_eq!(
            named_std_outside_tests(&strip(bad)).len(),
            1,
            "should flag: {bad}"
        );
    }
    for good in [
        "extern crate std as _;",
        "extern crate alloc;",
        "#[cfg(test)] mod tests { extern crate std; }",
        "#[cfg(test)]\n#[allow(\n    clippy::unwrap_used,\n)]\nmod tests {\n    extern crate std;\n}",
        "pub mod a { #[cfg(test)] mod t { fn g() { if x { } } extern crate std; } }",
        "#[cfg(test)] extern crate std;",
        "#![cfg(test)]\nextern crate std;",
        "// extern crate std;\nlet s = \"extern crate std;\"; let n = 1;",
        "extern \"C\" fn f() {}",
        "let external = 1; let externals = 2;",
    ] {
        assert_eq!(
            named_std_outside_tests(&strip(good)),
            Vec::<String>::new(),
            "should pass: {good}"
        );
    }
}
