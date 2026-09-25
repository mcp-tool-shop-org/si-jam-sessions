//! The canonical encoding: the one byte string the law hashes for a receipt.
//!
//! Version 1. Every integer is little-endian; there is no padding and no alignment.
//!
//! ```text
//! receipt      = magic "SJRC" · u16 version (1) · u16 schema
//!                · str score_id · str title · date fetched_on
//!                · composition · source_edition · arrangement
//!                · vec<file> files · vec<evidence> evidence · vec<str> notes
//! composition  = vec<author> authors · opt<u16> first_publication_year · vec<str> evidence
//! author       = str name · u8 role · opt<u16> death_year
//! source_edition = str statement · opt<str> publisher · opt<u16> year · u8 kind
//!                · vec<str> evidence
//! arrangement  = u8 0 · third_party  |  u8 1 · str engraver
//! third_party  = str host · str record · str typesetter · vec<str> contributors
//!                · quote page_licence · terms · opt<str> credit_ledger_id
//! quote        = str evidence · str text
//! terms        = str evidence · str text · vec<u8> restrictions
//! file         = str name · u8 media · str url · [u8; 32] sha256 · u64 bytes
//!                · opt<str> last_modified · vec<statement> in_file_licence
//! statement    = u8 field · str text
//! evidence     = str id · str url · opt<str> resolved_url · [u8; 32] sha256 · u64 bytes
//!                · vec<str> quotes
//! date         = u16 year · u8 month · u8 day
//! str          = u32 byte length · UTF-8 bytes
//! opt<T>       = u8 0  |  u8 1 · T
//! vec<T>       = u32 count · T × count
//! u8 enums     = the vocabulary tags in `receipt.rs`
//! ```
//!
//! Decoding is strict: the magic and version must match, every tag and option flag must
//! be one this version defines, every string must be UTF-8, nothing may follow the last
//! field, and the decoded receipt must pass [`Receipt::check_structure`] (which fixes the
//! order of files, evidence and restrictions). So every accepted byte string re-encodes to
//! itself, and a receipt has exactly one encoding.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{CanonicalProblem, ReceiptError};
use crate::receipt::{
    Arrangement, Author, AuthorRole, Composition, Date, EditionKind, Evidence, FileEntry, Media,
    Quote, Receipt, Restriction, SourceEdition, Statement, StatementField, Terms, ThirdParty,
    ThisProject,
};

/// The first four bytes of every canonical receipt.
pub const MAGIC: [u8; 4] = *b"SJRC";

/// The version of this encoding.
pub const CANONICAL_VERSION: u16 = 1;

pub(crate) fn encode(r: &Receipt) -> Vec<u8> {
    let mut w = Writer { out: Vec::new() };
    w.out.extend_from_slice(&MAGIC);
    w.u16(CANONICAL_VERSION);
    w.u16(r.schema);
    w.str(&r.score_id);
    w.str(&r.title);
    w.date(r.fetched_on);

    let c = &r.composition;
    w.vec(&c.authors, |w, a| {
        w.str(&a.name);
        w.u8(a.role.tag());
        w.opt_u16(a.death_year);
    });
    w.opt_u16(c.first_publication_year);
    w.strs(&c.evidence);

    let s = &r.source_edition;
    w.str(&s.statement);
    w.opt_str(s.publisher.as_deref());
    w.opt_u16(s.year);
    w.u8(s.kind.tag());
    w.strs(&s.evidence);

    match &r.arrangement {
        Arrangement::ThirdParty(t) => {
            w.u8(0);
            w.str(&t.host);
            w.str(&t.record);
            w.str(&t.typesetter);
            w.strs(&t.contributors);
            w.str(&t.page_licence.evidence);
            w.str(&t.page_licence.text);
            w.str(&t.terms.evidence);
            w.str(&t.terms.text);
            w.vec(&t.terms.restrictions, |w, x| w.u8(x.tag()));
            w.opt_str(t.credit_ledger_id.as_deref());
        }
        Arrangement::ThisProject(p) => {
            w.u8(1);
            w.str(&p.engraver);
        }
    }

    w.vec(&r.files, |w, f| {
        w.str(&f.name);
        w.u8(f.media.tag());
        w.str(&f.url);
        w.out.extend_from_slice(&f.sha256);
        w.u64(f.bytes);
        w.opt_str(f.last_modified.as_deref());
        w.vec(&f.in_file_licence, |w, st| {
            w.u8(st.field.tag());
            w.str(&st.text);
        });
    });
    w.vec(&r.evidence, |w, e| {
        w.str(&e.id);
        w.str(&e.url);
        w.opt_str(e.resolved_url.as_deref());
        w.out.extend_from_slice(&e.sha256);
        w.u64(e.bytes);
        w.strs(&e.quotes);
    });
    w.strs(&r.notes);
    w.out
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Receipt, ReceiptError> {
    let mut r = Reader { bytes, pos: 0 };
    if r.take(4)? != MAGIC {
        return Err(r.fail_at(0, CanonicalProblem::BadMagic));
    }
    if r.u16()? != CANONICAL_VERSION {
        return Err(r.fail_at(4, CanonicalProblem::UnsupportedVersion));
    }
    let schema = r.u16()?;
    let score_id = r.str()?;
    let title = r.str()?;
    let fetched_on = r.date()?;

    let composition = Composition {
        authors: r.vec(|r| {
            Ok(Author {
                name: r.str()?,
                role: r.tag(AuthorRole::from_tag)?,
                death_year: r.opt_u16()?,
            })
        })?,
        first_publication_year: r.opt_u16()?,
        evidence: r.strs()?,
    };

    let source_edition = SourceEdition {
        statement: r.str()?,
        publisher: r.opt_str()?,
        year: r.opt_u16()?,
        kind: r.tag(EditionKind::from_tag)?,
        evidence: r.strs()?,
    };

    let arrangement = match r.u8()? {
        0 => Arrangement::ThirdParty(Box::new(ThirdParty {
            host: r.str()?,
            record: r.str()?,
            typesetter: r.str()?,
            contributors: r.strs()?,
            page_licence: Quote {
                evidence: r.str()?,
                text: r.str()?,
            },
            terms: Terms {
                evidence: r.str()?,
                text: r.str()?,
                restrictions: r.vec(|r| r.tag(Restriction::from_tag))?,
            },
            credit_ledger_id: r.opt_str()?,
        })),
        1 => Arrangement::ThisProject(ThisProject { engraver: r.str()? }),
        _ => return Err(r.fail_at(r.pos - 1, CanonicalProblem::BadTag)),
    };

    let files = r.vec(|r| {
        Ok(FileEntry {
            name: r.str()?,
            media: r.tag(Media::from_tag)?,
            url: r.str()?,
            sha256: r.hash()?,
            bytes: r.u64()?,
            last_modified: r.opt_str()?,
            in_file_licence: r.vec(|r| {
                Ok(Statement {
                    field: r.tag(StatementField::from_tag)?,
                    text: r.str()?,
                })
            })?,
        })
    })?;
    let evidence = r.vec(|r| {
        Ok(Evidence {
            id: r.str()?,
            url: r.str()?,
            resolved_url: r.opt_str()?,
            sha256: r.hash()?,
            bytes: r.u64()?,
            quotes: r.strs()?,
        })
    })?;
    let notes = r.strs()?;

    if r.pos != bytes.len() {
        return Err(r.fail_at(r.pos, CanonicalProblem::TrailingBytes));
    }
    let receipt = Receipt {
        schema,
        score_id,
        title,
        fetched_on,
        composition,
        source_edition,
        arrangement,
        files,
        evidence,
        notes,
    };
    receipt.check_structure()?;
    Ok(receipt)
}

struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        // A receipt is loaded from at most `json::MAX_INPUT` bytes, far below u32::MAX.
        // Saturating keeps this total; a saturated length could never decode.
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.out.extend_from_slice(s.as_bytes());
    }
    fn opt_str(&mut self, s: Option<&str>) {
        match s {
            None => self.u8(0),
            Some(s) => {
                self.u8(1);
                self.str(s);
            }
        }
    }
    fn opt_u16(&mut self, v: Option<u16>) {
        match v {
            None => self.u8(0),
            Some(v) => {
                self.u8(1);
                self.u16(v);
            }
        }
    }
    fn date(&mut self, d: Date) {
        self.u16(d.year);
        self.u8(d.month);
        self.u8(d.day);
    }
    fn vec<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Writer, &T)) {
        self.len(items.len());
        for item in items {
            each(self, item);
        }
    }
    fn strs(&mut self, items: &[String]) {
        self.vec(items, |w, s| w.str(s));
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn fail_at(&self, offset: usize, problem: CanonicalProblem) -> ReceiptError {
        ReceiptError::Canonical { offset, problem }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ReceiptError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| self.fail_at(self.pos, CanonicalProblem::UnexpectedEnd))?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, ReceiptError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ReceiptError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, ReceiptError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Result<u64, ReceiptError> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    fn hash(&mut self) -> Result<[u8; 32], ReceiptError> {
        let b = self.take(32)?;
        let mut a = [0u8; 32];
        a.copy_from_slice(b);
        Ok(a)
    }
    fn tag<T>(&mut self, from_tag: impl Fn(u8) -> Option<T>) -> Result<T, ReceiptError> {
        let at = self.pos;
        let tag = self.u8()?;
        from_tag(tag).ok_or_else(|| self.fail_at(at, CanonicalProblem::BadTag))
    }
    fn flag(&mut self) -> Result<bool, ReceiptError> {
        let at = self.pos;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.fail_at(at, CanonicalProblem::BadFlag)),
        }
    }
    fn str(&mut self) -> Result<String, ReceiptError> {
        let len = self.u32()? as usize;
        let at = self.pos;
        let raw = self.take(len)?;
        let text =
            core::str::from_utf8(raw).map_err(|_| self.fail_at(at, CanonicalProblem::NotUtf8))?;
        Ok(String::from(text))
    }
    fn opt_str(&mut self) -> Result<Option<String>, ReceiptError> {
        if self.flag()? {
            Ok(Some(self.str()?))
        } else {
            Ok(None)
        }
    }
    fn opt_u16(&mut self) -> Result<Option<u16>, ReceiptError> {
        if self.flag()? {
            Ok(Some(self.u16()?))
        } else {
            Ok(None)
        }
    }
    fn date(&mut self) -> Result<Date, ReceiptError> {
        Ok(Date {
            year: self.u16()?,
            month: self.u8()?,
            day: self.u8()?,
        })
    }
    fn vec<T>(
        &mut self,
        mut each: impl FnMut(&mut Reader<'a>) -> Result<T, ReceiptError>,
    ) -> Result<Vec<T>, ReceiptError> {
        let at = self.pos;
        let count = self.u32()? as usize;
        // Every element takes at least one byte, so a count beyond the remaining bytes is
        // a lie; refusing it early also keeps a hostile count from driving the loop.
        if count > self.bytes.len() - self.pos {
            return Err(self.fail_at(at, CanonicalProblem::UnexpectedEnd));
        }
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(each(self)?);
        }
        Ok(items)
    }
    fn strs(&mut self) -> Result<Vec<String>, ReceiptError> {
        self.vec(|r| r.str())
    }
}
