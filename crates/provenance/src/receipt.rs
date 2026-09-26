//! The receipt: what was fetched, from where, with which hashes, and what the sources say
//! about who wrote the music, which edition was followed and under what licence.
//!
//! A receipt is evidence, not a verdict. [`crate::admit`] reads it together with the file
//! bytes and decides. Loading a receipt (from JSON or from its canonical bytes) checks only
//! its structure; whether it proves enough is the predicate's job, so a receipt with a
//! missing year still loads and is then refused with a named reason.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::ReceiptError;

/// The receipt schema this crate reads and writes.
pub const RECEIPT_SCHEMA: u16 = 1;

/// Longest file or evidence name accepted, in bytes.
pub const MAX_NAME_LEN: usize = 128;

/// Declares a closed vocabulary: each variant has a stable tag for the canonical encoding
/// and a stable name for JSON. Tags ascend in declaration order, so the derived `Ord` is
/// the tag order.
macro_rules! closed_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = ($tag:literal, $text:literal), )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// Every variant, in tag order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The variant's tag in the canonical encoding.
            pub const fn tag(self) -> u8 {
                match self { $($name::$variant => $tag,)+ }
            }

            /// The variant for a canonical tag, if there is one.
            pub const fn from_tag(tag: u8) -> Option<Self> {
                match tag { $($tag => Some($name::$variant),)+ _ => None }
            }

            /// The variant's name in JSON.
            pub const fn name(self) -> &'static str {
                match self { $($name::$variant => $text,)+ }
            }

            /// The variant for a JSON name, if there is one. Matching is exact.
            pub fn from_name(text: &str) -> Option<Self> {
                match text { $($text => Some($name::$variant),)+ _ => None }
            }
        }
    };
}

closed_enum! {
    /// What an author of the composition wrote. The EU term runs from the death of the
    /// last of them, so every one of them needs a death year.
    pub enum AuthorRole {
        Composer = (0, "composer"),
        Lyricist = (1, "lyricist"),
    }
}

closed_enum! {
    /// What kind of edition the typesetting follows: what the edition adds to the work,
    /// and so which term protects what it adds.
    pub enum EditionKind {
        /// The work's first publication.
        FirstEdition = (0, "first-edition"),
        /// A reprint or facsimile of an earlier edition, with no editorial work of its own.
        Reprint = (1, "reprint"),
        /// A critical, scholarly or urtext edition: the kind a scholarly-edition term protects.
        Scholarly = (2, "scholarly"),
        /// Not established.
        Unknown = (3, "unknown"),
        /// An edition that adds a setting of its own, such as a harmonisation, an
        /// accompaniment or editing, and does not name who made it. The receipt records
        /// that author as unknown. What the edition adds is an anonymous work, whose term
        /// runs from the edition's publication.
        Anonymous = (4, "anonymous"),
    }
}

closed_enum! {
    /// A restriction found in a source's terms. Any one of them refuses the score.
    pub enum Restriction {
        AllRightsReserved = (0, "all-rights-reserved"),
        NoRedistribution = (1, "no-redistribution"),
        ShareAlike = (2, "share-alike"),
        NonCommercial = (3, "non-commercial"),
        NoDerivatives = (4, "no-derivatives"),
        /// A term that forbids or limits processing by, or training of, AI models.
        AiRestricted = (5, "ai-restricted"),
    }
}

closed_enum! {
    /// The format of a payload file, which decides how its in-file licence is read.
    pub enum Media {
        /// LilyPond source (`.ly`).
        Lilypond = (0, "lilypond"),
        /// Standard MIDI File (`.mid`).
        Smf = (1, "smf"),
    }
}

closed_enum! {
    /// Where inside a file a licence statement was read.
    pub enum StatementField {
        /// A LilyPond `license = "..."` assignment.
        LilypondLicense = (0, "lilypond-license"),
        /// A LilyPond `copyright = "..."` assignment.
        LilypondCopyright = (1, "lilypond-copyright"),
        /// The text of a LilyPond `copyright = \markup { ... }` assignment: its LilyPond
        /// string literals in order, joined by one space, whitespace collapsed.
        LilypondCopyrightMarkup = (2, "lilypond-copyright-markup"),
        /// An SMF copyright meta event (`FF 02`).
        SmfCopyright = (3, "smf-copyright"),
        /// Any other SMF text meta event whose text mentions copyright or a licence.
        SmfText = (4, "smf-text"),
    }
}

/// A receipt for one score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// Always [`RECEIPT_SCHEMA`].
    pub schema: u16,
    /// The score's id in this repository, e.g. `entertainer`.
    pub score_id: String,
    /// The work's title.
    pub title: String,
    /// The day every file and every piece of evidence was fetched.
    pub fetched_on: Date,
    pub composition: Composition,
    pub source_edition: SourceEdition,
    pub arrangement: Arrangement,
    /// The payload: the files the law may be handed. Sorted by name, names unique.
    pub files: Vec<FileEntry>,
    /// Fetched documents that support the claims. Sorted by id, ids unique.
    pub evidence: Vec<Evidence>,
    /// Findings for a human reader. The predicate does not read them.
    pub notes: Vec<String>,
}

/// A calendar date. The law reads no clock; this is a recorded fact, not a comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

/// The musical work, independent of any edition or typesetting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    /// Every author of the work, in order of credit. The EU term runs from the death of
    /// the last named author, or, when an author is unknown, from the work's publication.
    pub authors: Vec<Author>,
    /// The year the work as a whole was first published: for a work whose parts appeared
    /// at different times, the year its last part did.
    pub first_publication_year: Option<u16>,
    /// Ids of the evidence that supports the years.
    pub evidence: Vec<String>,
}

/// One author of the work, named or unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Author {
    /// The author's name, or `None` for an author the sources do not identify: the author
    /// of an anonymous work. A claimant is never recorded here; claimants go in the
    /// receipt's notes. A pseudonym that leaves no doubt who the author is names them.
    pub name: Option<String>,
    pub role: AuthorRole,
    /// The year a named author died. An unknown author has none.
    pub death_year: Option<u16>,
}

/// The edition the typesetting follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEdition {
    /// The source as the typesetting states it, verbatim.
    pub statement: String,
    pub publisher: Option<String>,
    pub year: Option<u16>,
    pub kind: EditionKind,
    /// Ids of the evidence that supports the publisher and year.
    pub evidence: Vec<String>,
}

/// Who set the notes in the payload files, and on what terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arrangement {
    /// A typesetting published by someone else, with a host page and terms.
    ThirdParty(Box<ThirdParty>),
    /// A typesetting made by this project, under the product licence.
    ThisProject(ThisProject),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThirdParty {
    /// The site that publishes the typesetting, e.g. `Mutopia Project`.
    pub host: String,
    /// The host's id for this typesetting, e.g. `Mutopia-2016/11/25-263`.
    pub record: String,
    /// The typesetter or maintainer the host credits.
    pub typesetter: String,
    /// Others the host or the file credits for the typesetting.
    pub contributors: Vec<String>,
    /// The licence the host page states for the typesetting.
    pub page_licence: Quote,
    /// The host's terms for that licence.
    pub terms: Terms,
    /// Required in the CC-BY-4.0 tier, forbidden in the public-domain tier.
    pub credit_ledger_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThisProject {
    pub engraver: String,
}

/// A verbatim quote, and the evidence it was read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub evidence: String,
    pub text: String,
}

/// The host's terms: a verbatim quote, its evidence, and every restriction found in them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terms {
    pub evidence: String,
    pub text: String,
    /// Sorted by tag, no repeats. Empty means the terms were read and none was found.
    pub restrictions: Vec<Restriction>,
}

/// One payload file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// A plain file name: `[A-Za-z0-9._-]`, not starting with a dot.
    pub name: String,
    pub media: Media,
    pub url: String,
    pub sha256: [u8; 32],
    pub bytes: u64,
    /// The HTTP `Last-Modified` header at fetch time, verbatim.
    pub last_modified: Option<String>,
    /// Every licence statement the file itself carries, verbatim, in file order. The
    /// predicate reads the file again and refuses if this list is not exactly what it finds.
    pub in_file_licence: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    pub field: StatementField,
    pub text: String,
}

/// A fetched document that supports a claim. It is hashed, not committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    /// A short id the rest of the receipt refers to. Same alphabet as file names.
    pub id: String,
    pub url: String,
    /// Where the request ended after redirects, if that differs from `url`.
    pub resolved_url: Option<String>,
    pub sha256: [u8; 32],
    pub bytes: u64,
    /// Short verbatim quotes from the document.
    pub quotes: Vec<String>,
}

impl Receipt {
    /// The structural rules every loaded receipt satisfies, whichever form it came from.
    /// They are what makes the canonical encoding unique: a receipt that fails them is
    /// never encoded or admitted.
    pub fn check_structure(&self) -> Result<(), ReceiptError> {
        if self.schema != RECEIPT_SCHEMA {
            return Err(ReceiptError::UnsupportedSchema(self.schema));
        }
        if !self.fetched_on.is_valid() {
            return Err(ReceiptError::InvalidDate);
        }
        if self
            .composition
            .authors
            .iter()
            .any(|a| a.name.is_none() && a.death_year.is_some())
        {
            return Err(ReceiptError::AnonymousAuthorDeathYear);
        }
        for pair in self.files.windows(2) {
            if pair[0].name >= pair[1].name {
                return Err(ReceiptError::FilesNotSorted);
            }
        }
        for file in &self.files {
            if !is_plain_name(&file.name) {
                return Err(ReceiptError::BadName);
            }
        }
        for pair in self.evidence.windows(2) {
            if pair[0].id >= pair[1].id {
                return Err(ReceiptError::EvidenceNotSorted);
            }
        }
        for evidence in &self.evidence {
            if !is_plain_name(&evidence.id) {
                return Err(ReceiptError::BadName);
            }
        }
        if let Arrangement::ThirdParty(third) = &self.arrangement {
            for pair in third.terms.restrictions.windows(2) {
                if pair[0] >= pair[1] {
                    return Err(ReceiptError::RestrictionsNotSorted);
                }
            }
        }
        Ok(())
    }

    /// The evidence with this id, if the receipt records it.
    pub fn evidence(&self, id: &str) -> Option<&Evidence> {
        self.evidence
            .binary_search_by(|e| e.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.evidence[i])
    }

    /// The payload file with this name, if the receipt lists it.
    pub fn file(&self, name: &str) -> Option<&FileEntry> {
        self.files
            .binary_search_by(|f| f.name.as_str().cmp(name))
            .ok()
            .map(|i| &self.files[i])
    }
}

impl Date {
    /// True for a real day of the Gregorian calendar.
    pub fn is_valid(&self) -> bool {
        let leap = (self.year.is_multiple_of(4) && !self.year.is_multiple_of(100))
            || self.year.is_multiple_of(400);
        let days = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return false,
        };
        self.day >= 1 && self.day <= days
    }
}

/// A plain file or evidence name: 1 to [`MAX_NAME_LEN`] bytes of `[A-Za-z0-9._-]`, not
/// starting with a dot. No separators, so a name can never be a path.
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags_ascend<T: Copy + Ord + core::fmt::Debug>(all: &[T], tag: impl Fn(T) -> u8) {
        for (i, pair) in all.windows(2).enumerate() {
            assert!(pair[0] < pair[1], "declaration order at {i}");
            assert!(tag(pair[0]) < tag(pair[1]), "tag order at {i}");
        }
    }

    #[test]
    fn every_vocabulary_round_trips_and_orders_by_tag() {
        tags_ascend(AuthorRole::ALL, AuthorRole::tag);
        tags_ascend(EditionKind::ALL, EditionKind::tag);
        tags_ascend(Restriction::ALL, Restriction::tag);
        tags_ascend(Media::ALL, Media::tag);
        tags_ascend(StatementField::ALL, StatementField::tag);
        for &v in Restriction::ALL {
            assert_eq!(Restriction::from_tag(v.tag()), Some(v));
            assert_eq!(Restriction::from_name(v.name()), Some(v));
        }
        for &v in StatementField::ALL {
            assert_eq!(StatementField::from_tag(v.tag()), Some(v));
            assert_eq!(StatementField::from_name(v.name()), Some(v));
        }
        assert_eq!(Restriction::from_tag(6), None);
        assert_eq!(Restriction::from_name("Share-Alike"), None);
    }

    #[test]
    fn dates_follow_the_calendar() {
        let d = |year, month, day| Date { year, month, day };
        assert!(d(2026, 9, 25).is_valid());
        assert!(d(2024, 2, 29).is_valid());
        assert!(d(2000, 2, 29).is_valid());
        assert!(!d(1900, 2, 29).is_valid());
        assert!(!d(2026, 2, 29).is_valid());
        assert!(!d(2026, 4, 31).is_valid());
        assert!(!d(2026, 13, 1).is_valid());
        assert!(!d(2026, 1, 0).is_valid());
    }

    #[test]
    fn names_cannot_be_paths() {
        assert!(is_plain_name("entertainer.mid"));
        assert!(is_plain_name("piece-page"));
        for bad in ["", ".hidden", "a/b", "a\\b", "..", "c:x", "a b", "é.ly"] {
            assert!(!is_plain_name(bad), "{bad:?}");
        }
        assert!(!is_plain_name(&"a".repeat(MAX_NAME_LEN + 1)));
    }
}
