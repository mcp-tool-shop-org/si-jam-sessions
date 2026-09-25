//! The receipt's JSON shape. Every key is required, unknown keys are refused, and optional
//! values are written as `null`, so a receipt always says out loud what it does not know.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::ReceiptError;
use crate::json::Value;
use crate::receipt::{
    Arrangement, Author, AuthorRole, Composition, Date, EditionKind, Evidence, FileEntry, Media,
    Quote, RECEIPT_SCHEMA, Receipt, Restriction, SourceEdition, Statement, StatementField, Terms,
    ThirdParty, ThisProject,
};

pub(crate) fn receipt(value: &Value) -> Result<Receipt, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "receipt")?;
    let schema = o.u16("schema")?;
    if schema != RECEIPT_SCHEMA {
        return Err(ReceiptError::UnsupportedSchema(schema));
    }
    let receipt = Receipt {
        schema,
        score_id: o.string("score_id")?,
        title: o.string("title")?,
        fetched_on: o.date("fetched_on")?,
        composition: composition(o.get("composition")?)?,
        source_edition: source_edition(o.get("source_edition")?)?,
        arrangement: arrangement(o.get("arrangement")?)?,
        files: o.list("files", file)?,
        evidence: o.list("evidence", evidence)?,
        notes: o.strings("notes")?,
    };
    o.finish()?;
    receipt.check_structure()?;
    Ok(receipt)
}

fn composition(value: &Value) -> Result<Composition, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "composition")?;
    let c = Composition {
        authors: o.list("authors", author)?,
        first_publication_year: o.opt_u16("first_publication_year")?,
        evidence: o.strings("evidence")?,
    };
    o.finish()?;
    Ok(c)
}

fn author(value: &Value) -> Result<Author, ReceiptError> {
    let mut o = Obj::new(value, "composition", "authors[]")?;
    let a = Author {
        name: o.string("name")?,
        role: o.vocab("role", AuthorRole::from_name)?,
        death_year: o.opt_u16("death_year")?,
    };
    o.finish()?;
    Ok(a)
}

fn source_edition(value: &Value) -> Result<SourceEdition, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "source_edition")?;
    let s = SourceEdition {
        statement: o.string("statement")?,
        publisher: o.opt_string("publisher")?,
        year: o.opt_u16("year")?,
        kind: o.vocab("kind", EditionKind::from_name)?,
        evidence: o.strings("evidence")?,
    };
    o.finish()?;
    Ok(s)
}

fn arrangement(value: &Value) -> Result<Arrangement, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "arrangement")?;
    let kind = o.string("kind")?;
    let a = match kind.as_str() {
        "third-party" => Arrangement::ThirdParty(Box::new(ThirdParty {
            host: o.string("host")?,
            record: o.string("record")?,
            typesetter: o.string("typesetter")?,
            contributors: o.strings("contributors")?,
            page_licence: quote(o.get("page_licence")?)?,
            terms: terms(o.get("terms")?)?,
            credit_ledger_id: o.opt_string("credit_ledger_id")?,
        })),
        "this-project" => Arrangement::ThisProject(ThisProject {
            engraver: o.string("engraver")?,
        }),
        _ => {
            return Err(ReceiptError::BadValue {
                object: "arrangement",
                key: "kind",
            });
        }
    };
    o.finish()?;
    Ok(a)
}

fn quote(value: &Value) -> Result<Quote, ReceiptError> {
    let mut o = Obj::new(value, "arrangement", "page_licence")?;
    let q = Quote {
        evidence: o.string("evidence")?,
        text: o.string("text")?,
    };
    o.finish()?;
    Ok(q)
}

fn terms(value: &Value) -> Result<Terms, ReceiptError> {
    let mut o = Obj::new(value, "arrangement", "terms")?;
    let t = Terms {
        evidence: o.string("evidence")?,
        text: o.string("text")?,
        restrictions: o.list("restrictions", |v| {
            vocab_value(v, "terms", "restrictions", Restriction::from_name)
        })?,
    };
    o.finish()?;
    Ok(t)
}

fn file(value: &Value) -> Result<FileEntry, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "files[]")?;
    let f = FileEntry {
        name: o.string("name")?,
        media: o.vocab("media", Media::from_name)?,
        url: o.string("url")?,
        sha256: o.sha256("sha256")?,
        bytes: o.u64("bytes")?,
        last_modified: o.opt_string("last_modified")?,
        in_file_licence: o.list("in_file_licence", statement)?,
    };
    o.finish()?;
    Ok(f)
}

fn statement(value: &Value) -> Result<Statement, ReceiptError> {
    let mut o = Obj::new(value, "files[]", "in_file_licence[]")?;
    let s = Statement {
        field: o.vocab("field", StatementField::from_name)?,
        text: o.string("text")?,
    };
    o.finish()?;
    Ok(s)
}

fn evidence(value: &Value) -> Result<Evidence, ReceiptError> {
    let mut o = Obj::new(value, "receipt", "evidence[]")?;
    let e = Evidence {
        id: o.string("id")?,
        url: o.string("url")?,
        resolved_url: o.opt_string("resolved_url")?,
        sha256: o.sha256("sha256")?,
        bytes: o.u64("bytes")?,
        quotes: o.strings("quotes")?,
    };
    o.finish()?;
    Ok(e)
}

fn vocab_value<T>(
    value: &Value,
    object: &'static str,
    key: &'static str,
    from_name: impl Fn(&str) -> Option<T>,
) -> Result<T, ReceiptError> {
    match value {
        Value::Str(s) => from_name(s).ok_or(ReceiptError::BadValue { object, key }),
        _ => Err(ReceiptError::BadValue { object, key }),
    }
}

/// Reads one JSON object, remembering which keys were taken so that [`Obj::finish`] can
/// refuse the rest.
struct Obj<'a> {
    name: &'static str,
    members: &'a [(String, Value)],
    taken: Vec<bool>,
}

impl<'a> Obj<'a> {
    fn new(
        value: &'a Value,
        parent: &'static str,
        name: &'static str,
    ) -> Result<Self, ReceiptError> {
        match value {
            Value::Obj(members) => Ok(Obj {
                name,
                members,
                taken: alloc::vec![false; members.len()],
            }),
            _ => Err(ReceiptError::BadValue {
                object: parent,
                key: name,
            }),
        }
    }

    fn bad(&self, key: &'static str) -> ReceiptError {
        ReceiptError::BadValue {
            object: self.name,
            key,
        }
    }

    fn get(&mut self, key: &'static str) -> Result<&'a Value, ReceiptError> {
        let members = self.members;
        match members.iter().position(|(k, _)| k == key) {
            Some(i) => {
                self.taken[i] = true;
                Ok(&members[i].1)
            }
            None => Err(ReceiptError::MissingKey {
                object: self.name,
                key,
            }),
        }
    }

    fn string(&mut self, key: &'static str) -> Result<String, ReceiptError> {
        match self.get(key)? {
            Value::Str(s) => Ok(s.clone()),
            _ => Err(self.bad(key)),
        }
    }

    fn opt_string(&mut self, key: &'static str) -> Result<Option<String>, ReceiptError> {
        match self.get(key)? {
            Value::Null => Ok(None),
            Value::Str(s) => Ok(Some(s.clone())),
            _ => Err(self.bad(key)),
        }
    }

    fn u64(&mut self, key: &'static str) -> Result<u64, ReceiptError> {
        match self.get(key)? {
            Value::Uint(n) => Ok(*n),
            _ => Err(self.bad(key)),
        }
    }

    fn u16(&mut self, key: &'static str) -> Result<u16, ReceiptError> {
        let n = self.u64(key)?;
        u16::try_from(n).map_err(|_| self.bad(key))
    }

    fn opt_u16(&mut self, key: &'static str) -> Result<Option<u16>, ReceiptError> {
        match self.get(key)? {
            Value::Null => Ok(None),
            Value::Uint(n) => u16::try_from(*n).map(Some).map_err(|_| self.bad(key)),
            _ => Err(self.bad(key)),
        }
    }

    fn vocab<T>(
        &mut self,
        key: &'static str,
        from_name: impl Fn(&str) -> Option<T>,
    ) -> Result<T, ReceiptError> {
        let value = self.get(key)?;
        vocab_value(value, self.name, key, from_name)
    }

    fn list<T>(
        &mut self,
        key: &'static str,
        item: impl Fn(&Value) -> Result<T, ReceiptError>,
    ) -> Result<Vec<T>, ReceiptError> {
        match self.get(key)? {
            Value::Arr(items) => items.iter().map(item).collect(),
            _ => Err(self.bad(key)),
        }
    }

    fn strings(&mut self, key: &'static str) -> Result<Vec<String>, ReceiptError> {
        let name = self.name;
        self.list(key, |v| match v {
            Value::Str(s) => Ok(s.clone()),
            _ => Err(ReceiptError::BadValue { object: name, key }),
        })
    }

    /// Exactly 64 lowercase hex digits.
    fn sha256(&mut self, key: &'static str) -> Result<[u8; 32], ReceiptError> {
        let text = self.string(key)?;
        parse_sha256(&text).ok_or(self.bad(key))
    }

    /// Exactly `YYYY-MM-DD`. Calendar validity is checked with the rest of the structure.
    fn date(&mut self, key: &'static str) -> Result<Date, ReceiptError> {
        let text = self.string(key)?;
        parse_date(&text).ok_or(self.bad(key))
    }

    fn finish(self) -> Result<(), ReceiptError> {
        match self.taken.iter().position(|taken| !taken) {
            Some(i) => Err(ReceiptError::UnknownKey {
                object: self.name,
                key: self.members[i].0.clone(),
            }),
            None => Ok(()),
        }
    }
}

pub(crate) fn parse_sha256(text: &str) -> Option<[u8; 32]> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    let mut out = [0u8; 32];
    for (byte, &[hi, lo]) in out.iter_mut().zip(pairs) {
        *byte = (lower_hex(hi)? << 4) | lower_hex(lo)?;
    }
    Some(out)
}

fn lower_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

fn parse_date(text: &str) -> Option<Date> {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |range: core::ops::Range<usize>| -> Option<u16> {
        let mut n: u16 = 0;
        for &d in &b[range] {
            if !d.is_ascii_digit() {
                return None;
            }
            n = n * 10 + u16::from(d - b'0');
        }
        Some(n)
    };
    Some(Date {
        year: digits(0..4)?,
        month: u8::try_from(digits(5..7)?).ok()?,
        day: u8::try_from(digits(8..10)?).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_lowercase_hex_only() {
        let hex = "3a07fcf4f47b6000ced3e6579ef74bebb3c15af82d542a3aa42271f797544f7d";
        let bytes = parse_sha256(hex).unwrap();
        assert_eq!(bytes[0], 0x3a);
        assert_eq!(bytes[31], 0x7d);
        assert_eq!(parse_sha256(&hex.to_ascii_uppercase()), None);
        assert_eq!(parse_sha256(&hex[..62]), None);
        assert_eq!(parse_sha256(&alloc::format!("{hex}00")), None);
        assert_eq!(parse_sha256(&hex.replace('a', "g")), None);
    }

    #[test]
    fn dates_are_iso_days_only() {
        assert_eq!(
            parse_date("2026-09-25"),
            Some(Date {
                year: 2026,
                month: 9,
                day: 25
            })
        );
        for bad in [
            "2026-9-25",
            "2026/09/25",
            "26-09-25",
            "2026-09-25T00",
            "２０２６-09-25",
        ] {
            assert_eq!(parse_date(bad), None, "{bad}");
        }
    }
}
