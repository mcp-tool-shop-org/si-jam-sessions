//! Provenance: the receipt a score arrives with, and the licence predicate that admits it
//! or refuses it before anything is ingested.
//!
//! A sibling project once published training data built from files it had no licence to
//! ship. This crate is why that cannot happen here, so every rule in it fails closed: what
//! the receipt does not show, the predicate does not assume.
//!
//! # The receipt
//!
//! [`Receipt`] records what was fetched, from where, and with which SHA-256; the licence
//! the host page states and the one each file states about itself; who wrote the work and
//! when they died; when it was first published; and which edition the typesetting follows.
//! On disk it is JSON in a strict subset ([`Receipt::from_json`]). What the law hashes is
//! its canonical encoding ([`Receipt::to_canonical`], [`Receipt::digest`]): little-endian,
//! length-prefixed, versioned, with exactly one encoding per receipt. The layout is
//! documented in `canonical.rs` and decoded strictly by [`Receipt::from_canonical`].
//!
//! # The predicate
//!
//! [`admit`] takes a receipt and the file bytes and returns an [`Admitted`] score in one
//! [`Tier`], or a [`Refusal`] that names its reason. Its checks, in order, are listed on
//! [`admit`]. The rules that are dates are constants of the law version below, because the
//! law reads no clock.
//!
//! # Normalisation
//!
//! Licence texts are compared after [`normalise`]: ends trimmed, inner ASCII whitespace
//! runs collapsed to one space, ASCII lower-cased, nothing else. A text that does not
//! normalise refuses.
//!
//! - **Restriction phrases** (AI use, all rights reserved, no redistribution,
//!   non-commercial, no derivatives, share-alike) are looked for in the words of every
//!   licence text the predicate reads, so separators do not matter (`CC BY NC` reads as
//!   `cc-by-nc`). A phrase must be whole words, except a few stems that also find their
//!   inflections (`noncommercially`, `neural nets`); no stem refuses plain text. One found
//!   anywhere refuses by its class. The rule is documented in `licence.rs`.
//! - **A statement** read from a file must equal the page's licence exactly.
//! - **A LilyPond copyright markup**, which is prose, must hold the page's licence as a
//!   whole phrase and affirm it: no negating, limiting, lapsing or hedging word and no
//!   question mark. The word list is closed and documented in `licence.rs`.
//! - **An evidence quote** must hold the page licence or the terms verbatim and as whole
//!   words, and a quote that holds either text must not negate it by the same rule.

#![no_std]

extern crate alloc;

mod canonical;
mod error;
mod infile;
mod json;
mod licence;
mod predicate;
mod receipt;
mod schema;

use alloc::vec::Vec;

pub use canonical::{CANONICAL_VERSION, MAGIC};
pub use error::{CanonicalProblem, JsonProblem, ReceiptError};
pub use infile::{Unreadable, smf_marker, statements};
pub use json::{MAX_DEPTH, MAX_INPUT};
pub use licence::{LicenceRefusal, normalise};
pub use predicate::{Admitted, Refusal, Supplied, Tier, admit};
pub use receipt::{
    Arrangement, Author, AuthorRole, Composition, Date, EditionKind, Evidence, FileEntry,
    MAX_NAME_LEN, Media, Quote, RECEIPT_SCHEMA, Receipt, Restriction, SourceEdition, Statement,
    StatementField, Terms, ThirdParty, ThisProject, is_plain_name,
};

/// The calendar year the date rules below are written for.
///
/// These constants belong to the law version. They move each January, and moving them is
/// a law-version bump in the same commit, because a changed cut-off can change which
/// scores are admitted.
pub const RULES_YEAR: u16 = 2026;

/// United States: a work first published in this year or earlier is in the public domain
/// in [`RULES_YEAR`] (95 years from publication, to the end of the calendar year).
pub const US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR: u16 = RULES_YEAR - 96;

/// European Union: a work whose every author died in this year or earlier is in the
/// public domain in [`RULES_YEAR`] (70 years after the last author's death, to the end of
/// the calendar year).
pub const EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR: u16 = RULES_YEAR - 71;

/// A scholarly edition published in this year or earlier is out of its term in
/// [`RULES_YEAR`] (German UrhG §70: 25 years from publication, to the end of the calendar
/// year). A more recent edition is refused unless the receipt shows otherwise, and in this
/// version nothing else on a receipt can.
pub const LAST_OUT_OF_TERM_EDITION_YEAR: u16 = RULES_YEAR - 26;

/// The version of this predicate: its rules, its admitted-licence list, its refusal
/// phrases, its in-file readers and the canonical encoding. The law folds it into its own
/// version. Any change to what is admitted or refused, or to the refusal a file is given,
/// bumps it.
///
/// Version 2 differs from version 1:
/// - It refuses more. An own engraving's files may state no licence. A copyright markup
///   or an evidence quote that holds the licence but negates it does not affirm it, and
///   an evidence quote must hold its text as whole words. AI wording refuses in every
///   licence text read, the other classes find more wording than version 1's lists, and
///   every restriction phrase is looked for in the page and terms quotes and in MIDI text
///   events.
/// - It names some refusals differently. An in-file statement that holds a restriction
///   phrase is refused by that restriction, AI wording is named before any other
///   restriction, and the SMF track-count checks refuse by their own names.
/// - It matches restriction phrases on a text's words, so separators do not matter
///   (`CC BY NC`, `cc-by-nc`), where version 1 matched substrings of the text as written.
///   A phrase must be whole words, except a few stems that find their inflections
///   (`noncommercially`, `neural networks`). So a phrase that starts inside a word
///   (`no derivatives` in `piano derivatives`), or runs on into a plain word
///   (`no-derivation`, `cc-by-sarah`), no longer refuses.
///
/// The matching rule was settled in four steps before version 2 shipped: whole words for
/// every class; then stems for every class but AI wording; then a text's words, with AI
/// wording's longer phrases as stems too; and then stems only where no plain word runs on
/// from them.
pub const PREDICATE_VERSION: u32 = 2;

impl Receipt {
    /// Loads a receipt from JSON in the strict subset (see `json.rs`), refusing unknown or
    /// missing keys, values of the wrong type or outside their vocabulary, and receipts
    /// that break [`Receipt::check_structure`].
    pub fn from_json(bytes: &[u8]) -> Result<Receipt, ReceiptError> {
        schema::receipt(&json::parse(bytes)?)
    }

    /// The canonical encoding. Only a receipt that passes [`Receipt::check_structure`] has
    /// one; [`admit`] refuses any other, so an unstructured receipt is never committed.
    pub fn to_canonical(&self) -> Vec<u8> {
        canonical::encode(self)
    }

    /// Decodes a canonical encoding strictly. Accepted bytes re-encode to themselves.
    pub fn from_canonical(bytes: &[u8]) -> Result<Receipt, ReceiptError> {
        canonical::decode(bytes)
    }

    /// SHA-256 of the canonical encoding: the receipt's identity in the law.
    pub fn digest(&self) -> [u8; 32] {
        predicate::sha256(&self.to_canonical())
    }
}

#[cfg(test)]
mod tests;
