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
//!   `cc-by-nc`). One found anywhere refuses by its class. The standard-phrase classes
//!   match the names their restrictions are known by, and no stem of theirs refuses plain
//!   text. The AI class fails closed on its topic: a text that names AI, training, models,
//!   mining, datasets, generative or neural systems, or an AI product, through the listed
//!   vocabulary is refused, whatever else it says. The vocabulary is closed, and Known
//!   limits below lists what it leaves out. The rules are documented in `licence.rs`.
//! - **A statement** read from a file must equal the page's licence exactly.
//! - **A LilyPond copyright markup**, which is prose, must hold the page's licence as a
//!   whole phrase and affirm it: no negating, limiting, prohibiting, lapsing or hedging
//!   word and no question mark. The word list is closed and documented in `licence.rs`.
//!   So a markup that prohibits anything is refused, even when the thing it prohibits is
//!   named outside the AI vocabulary.
//! - **An evidence quote** must hold the page licence or the terms verbatim and as whole
//!   words, and a quote that holds either text must not negate it by the same rule.
//!
//! # Known limits
//!
//! - **AI products whose names are ordinary words or people's names** are left out of the
//!   AI vocabulary: claude, gemini, llama, bard, grok and mistral. Claude Debussy's scores
//!   belong to this corpus. A text that prohibits one of them ("Claude use is prohibited")
//!   is still refused by the negation rule, but not named AI-restricted; one that limits
//!   it without a prohibiting or negating word passes.
//! - **Automated-access terms** (scraping, crawling) are not a refusal class in version
//!   2. No phrase names them and no receipt restriction records them. A text that forbids
//!   them with a prohibiting or negating word ("scraping is prohibited", "no scraping") is
//!   refused by the negation rule, not by a class of its own; one without ("scraping
//!   requires written permission") names nothing the predicate refuses.

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
/// A version number is frozen when it reaches main.
/// Before that it may be refined, but one number never names two different goldens, and every pushed refinement is recorded here.
///
/// Version 2 differs from version 1:
/// - It refuses more. An own engraving's files may state no licence. A copyright markup
///   or an evidence quote that holds the licence but negates, limits or prohibits anything
///   does not affirm it, and an evidence quote must hold its text as whole words. The AI
///   class refuses any licence text read that names its topic, the other classes find
///   more wording than version 1's lists, and every restriction phrase is looked for in
///   the page and terms quotes and in MIDI text events.
/// - It names some refusals differently. An in-file statement that holds a restriction
///   phrase is refused by that restriction, AI wording is named before any other
///   restriction, and the SMF track-count checks refuse by their own names.
/// - It matches restriction phrases on a text's words, so separators do not matter
///   (`CC BY NC`, `cc-by-nc`), where version 1 matched substrings of the text as written.
///   A phrase must be whole words, except a few stems that find their inflections
///   (`noncommercially`, `neural networks`). So a phrase that starts inside a word
///   (`no derivatives` in `piano derivatives`), or runs on into a plain word
///   (`no-derivation`, `cc-by-sarah`), no longer refuses. The AI class fails closed on its
///   topic, so plain text that names it is refused ("ear training", "model trains").
///
/// Version 2's pushed refinements, one line each. Under every one of them the Entertainer
/// receipt's digest is unchanged (`e23ba2e9…`), and the Entertainer is admitted as public
/// domain. A commit that only edits this record refines nothing.
/// - `fead502`: whole-word matching.
/// - `390336b`: stems matched from the start of a word.
/// - `f4038cc`: phrases matched on a text's words, so separators do not matter
///   (`CC BY ND`), and AI wording beyond its short tokens matched as stems.
/// - `6780f16`: a phrase is a stem only where no plain word runs on from it; the other
///   phrases are whole words, with their needed forms listed.
/// - `dede20b`: the AI class fails closed on its topic. A text that names AI, training,
///   models, mining, datasets, generative or neural systems is refused, plain text
///   included.
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
