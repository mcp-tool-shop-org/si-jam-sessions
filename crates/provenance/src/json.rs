//! A strict reader for the JSON subset a receipt is written in.
//!
//! Accepted: objects, arrays, strings, unsigned integers that fit `u64`, `true`, `false`
//! and `null`, separated by JSON whitespace (space, tab, LF, CR).
//!
//! Refused: a byte-order mark; invalid UTF-8; raw control characters inside strings;
//! escapes other than JSON's own; lone surrogates; negative numbers, fractions, exponents
//! and leading zeros; duplicate keys; anything after the top-level value; nesting deeper
//! than [`MAX_DEPTH`]; inputs over [`MAX_INPUT`] bytes.
//!
//! This reader is how a human-readable receipt reaches the law. What the law hashes is
//! the canonical encoding (see `canonical.rs`), so two JSON files that differ only in
//! whitespace or escaping load to the same receipt and hash the same.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{JsonProblem, ReceiptError};

/// Largest accepted input, in bytes.
pub const MAX_INPUT: usize = 1 << 20;

/// Deepest accepted nesting. The receipt schema nests four levels.
pub const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Uint(u64),
    Str(String),
    Arr(Vec<Value>),
    /// Members in input order. Keys are unique.
    Obj(Vec<(String, Value)>),
}

pub(crate) fn parse(input: &[u8]) -> Result<Value, ReceiptError> {
    if input.len() > MAX_INPUT {
        return Err(err(0, JsonProblem::TooLarge));
    }
    let text =
        core::str::from_utf8(input).map_err(|e| err(e.valid_up_to(), JsonProblem::NotUtf8))?;
    if text.starts_with('\u{feff}') {
        return Err(err(0, JsonProblem::ByteOrderMark));
    }
    let mut parser = Parser {
        text,
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let value = parser.value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(err(parser.pos, JsonProblem::TrailingContent));
    }
    Ok(value)
}

fn err(offset: usize, problem: JsonProblem) -> ReceiptError {
    ReceiptError::Json { offset, problem }
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn unexpected(&self) -> ReceiptError {
        if self.pos >= self.bytes.len() {
            err(self.pos, JsonProblem::UnexpectedEnd)
        } else {
            err(self.pos, JsonProblem::UnexpectedByte)
        }
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), ReceiptError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ReceiptError> {
        if depth > MAX_DEPTH {
            return Err(err(self.pos, JsonProblem::TooDeep));
        }
        match self.peek() {
            None => Err(self.unexpected()),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::Str),
            Some(b'-') => Err(err(self.pos, JsonProblem::NegativeNumber)),
            Some(b'0'..=b'9') => self.number(),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(_) => Err(self.unexpected()),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, ReceiptError> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.unexpected())
        }
    }

    fn number(&mut self) -> Result<Value, ReceiptError> {
        let start = self.pos;
        let mut n: u64 = 0;
        while let Some(digit @ b'0'..=b'9') = self.peek() {
            if self.pos > start && self.bytes[start] == b'0' {
                return Err(err(start, JsonProblem::LeadingZero));
            }
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add(u64::from(digit - b'0')))
                .ok_or_else(|| err(start, JsonProblem::NumberTooLarge))?;
            self.pos += 1;
        }
        if let Some(b'.' | b'e' | b'E') = self.peek() {
            return Err(err(start, JsonProblem::NotAnInteger));
        }
        Ok(Value::Uint(n))
    }

    fn string(&mut self) -> Result<String, ReceiptError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let run = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            // `run` and `pos` both sit on ASCII bytes or at the end, so both are char
            // boundaries of the (already validated) UTF-8 text.
            out.push_str(&self.text[run..self.pos]);
            match self.peek() {
                None => return Err(self.unexpected()),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    let c = self.escape()?;
                    out.push(c);
                }
                Some(_) => return Err(err(self.pos, JsonProblem::ControlCharacter)),
            }
        }
    }

    fn escape(&mut self) -> Result<char, ReceiptError> {
        let at = self.pos;
        self.pos += 1;
        let Some(b) = self.peek() else {
            return Err(self.unexpected());
        };
        self.pos += 1;
        Ok(match b {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let unit = self.hex4()?;
                match unit {
                    0xD800..=0xDBFF => {
                        if !self.bytes[self.pos..].starts_with(b"\\u") {
                            return Err(err(at, JsonProblem::LoneSurrogate));
                        }
                        self.pos += 2;
                        let low = self.hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(err(at, JsonProblem::LoneSurrogate));
                        }
                        let scalar = 0x1_0000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                        char::from_u32(scalar).ok_or_else(|| err(at, JsonProblem::LoneSurrogate))?
                    }
                    0xDC00..=0xDFFF => return Err(err(at, JsonProblem::LoneSurrogate)),
                    _ => char::from_u32(unit).ok_or_else(|| err(at, JsonProblem::BadEscape))?,
                }
            }
            _ => return Err(err(at, JsonProblem::BadEscape)),
        })
    }

    fn hex4(&mut self) -> Result<u32, ReceiptError> {
        let mut unit = 0u32;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(b @ b'0'..=b'9') => b - b'0',
                Some(b @ b'a'..=b'f') => b - b'a' + 10,
                Some(b @ b'A'..=b'F') => b - b'A' + 10,
                Some(_) => return Err(err(self.pos, JsonProblem::BadEscape)),
                None => return Err(self.unexpected()),
            };
            unit = unit * 16 + u32::from(digit);
            self.pos += 1;
        }
        Ok(unit)
    }

    fn object(&mut self, depth: usize) -> Result<Value, ReceiptError> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Value)> = Vec::new();
        // Keys seen so far, so a duplicate costs a lookup rather than a scan.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Obj(members));
        }
        loop {
            self.skip_ws();
            let key_at = self.pos;
            if self.peek() != Some(b'"') {
                return Err(self.unexpected());
            }
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(err(key_at, JsonProblem::DuplicateKey));
            }
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Obj(members));
                }
                _ => return Err(self.unexpected()),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ReceiptError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Arr(items));
                }
                _ => return Err(self.unexpected()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn problem(input: &str) -> JsonProblem {
        match parse(input.as_bytes()) {
            Err(ReceiptError::Json { problem, .. }) => problem,
            other => panic!("{input:?} gave {other:?}"),
        }
    }

    #[test]
    fn reads_the_subset() {
        let v = parse(
            br#" { "a" : [1, 0, 18446744073709551615, true, false, null], "b": {"c": "d"} } "#,
        )
        .unwrap();
        assert_eq!(
            v,
            Value::Obj(vec![
                (
                    "a".to_string(),
                    Value::Arr(vec![
                        Value::Uint(1),
                        Value::Uint(0),
                        Value::Uint(u64::MAX),
                        Value::Bool(true),
                        Value::Bool(false),
                        Value::Null,
                    ])
                ),
                (
                    "b".to_string(),
                    Value::Obj(vec![("c".to_string(), Value::Str("d".to_string()))])
                ),
            ])
        );
        assert_eq!(parse(b"[]").unwrap(), Value::Arr(vec![]));
        assert_eq!(parse(b"{}").unwrap(), Value::Obj(vec![]));
    }

    #[test]
    fn decodes_escapes_and_keeps_utf8() {
        let v = parse(r#""q\" b\\ s\/ \b\f\n\r\t é 𝄞 ‘x’""#.as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Str("q\" b\\ s/ \u{8}\u{c}\n\r\t é \u{1d11e} ‘x’".to_string())
        );
    }

    #[test]
    fn refuses_everything_outside_the_subset() {
        assert_eq!(problem("\u{feff}{}"), JsonProblem::ByteOrderMark);
        assert_eq!(problem(""), JsonProblem::UnexpectedEnd);
        assert_eq!(problem("{} x"), JsonProblem::TrailingContent);
        assert_eq!(problem("{} {}"), JsonProblem::TrailingContent);
        assert_eq!(problem(r#"{"a":1,"a":2}"#), JsonProblem::DuplicateKey);
        assert_eq!(problem("-1"), JsonProblem::NegativeNumber);
        assert_eq!(problem("1.5"), JsonProblem::NotAnInteger);
        assert_eq!(problem("1e3"), JsonProblem::NotAnInteger);
        assert_eq!(problem("01"), JsonProblem::LeadingZero);
        assert_eq!(problem("18446744073709551616"), JsonProblem::NumberTooLarge);
        assert_eq!(problem(r#""\x""#), JsonProblem::BadEscape);
        assert_eq!(problem(r#""\u12g4""#), JsonProblem::BadEscape);
        assert_eq!(problem(r#""\ud834""#), JsonProblem::LoneSurrogate);
        assert_eq!(problem(r#""\udd1e""#), JsonProblem::LoneSurrogate);
        assert_eq!(problem(r#""\ud834A""#), JsonProblem::LoneSurrogate);
        assert_eq!(problem("\"a\u{1}b\""), JsonProblem::ControlCharacter);
        assert_eq!(problem("\"a\nb\""), JsonProblem::ControlCharacter);
        assert_eq!(problem(r#"{"a":1,}"#), JsonProblem::UnexpectedByte);
        assert_eq!(problem("[1,]"), JsonProblem::UnexpectedByte);
        assert_eq!(problem("[1 2]"), JsonProblem::UnexpectedByte);
        assert_eq!(problem("{'a':1}"), JsonProblem::UnexpectedByte);
        assert_eq!(problem("tru"), JsonProblem::UnexpectedByte);
        assert_eq!(problem("NaN"), JsonProblem::UnexpectedByte);
        assert_eq!(problem(r#"{"a":1"#), JsonProblem::UnexpectedEnd);
        assert_eq!(problem(r#""abc"#), JsonProblem::UnexpectedEnd);
        assert_eq!(problem("[\u{a0}]"), JsonProblem::UnexpectedByte);
        match parse(b"\"\xff\"") {
            Err(ReceiptError::Json {
                offset: 1,
                problem: JsonProblem::NotUtf8,
            }) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_duplicate_among_many_keys_is_found() {
        let mut text = String::from("{");
        for i in 0..30_000 {
            text.push_str(&alloc::format!("\"k{i}\":0,"));
        }
        let at = text.len();
        text.push_str("\"k0\":1}");
        assert!(text.len() < MAX_INPUT);
        assert_eq!(
            parse(text.as_bytes()),
            Err(ReceiptError::Json {
                offset: at,
                problem: JsonProblem::DuplicateKey
            })
        );
    }

    #[test]
    fn bounds_depth_and_size() {
        let deep_ok = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(deep_ok.as_bytes()).is_ok());
        let too_deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert_eq!(problem(&too_deep), JsonProblem::TooDeep);
        let big = vec![b' '; MAX_INPUT + 1];
        assert!(matches!(
            parse(&big),
            Err(ReceiptError::Json {
                problem: JsonProblem::TooLarge,
                ..
            })
        ));
    }
}
