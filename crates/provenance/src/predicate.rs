//! The licence predicate. See the crate documentation for the rules and their order.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use sha2::{Digest, Sha256};

use crate::error::ReceiptError;
use crate::infile::{self, Unreadable};
use crate::licence::{self, AdmittedClass, LicenceRefusal, OWN_ENGRAVING_LICENCE};
use crate::receipt::{
    Arrangement, AuthorRole, EditionKind, FileEntry, Receipt, Statement, StatementField, ThirdParty,
};
use crate::{
    EU_LAST_PUBLIC_DOMAIN_ANONYMOUS_PUBLICATION_YEAR, EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR,
    LAST_OUT_OF_TERM_EDITION_YEAR, US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR,
};

/// A file handed to the predicate: its name in the receipt and its bytes.
#[derive(Clone, Copy, Debug)]
pub struct Supplied<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
}

/// An admitted score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted {
    pub tier: Tier,
    /// SHA-256 of the receipt's canonical encoding, for the law to commit.
    pub receipt_digest: [u8; 32],
}

/// The tier a score is admitted into. Tiers never mix: a public-domain score carries no
/// credit, and a CC-BY-4.0 score always does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Composition and typesetting both public domain.
    PublicDomain,
    /// A public-domain composition typeset by this project, under the product licence.
    OwnEngraving,
    /// A public-domain composition in a CC-BY-4.0 typesetting. Every row built from it
    /// carries this credit-ledger id.
    CcBy40 { credit_ledger_id: String },
}

/// Why a score is refused. Every refusal names its reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The receipt breaks a structural rule (see [`Receipt::check_structure`]).
    Receipt(ReceiptError),

    // Check 1: the files.
    /// The receipt lists a file that was not supplied.
    MissingFile {
        name: String,
    },
    /// A supplied file is not in the receipt.
    UnexpectedFile {
        name: String,
    },
    /// Two supplied files have the same name.
    DuplicateFile {
        name: String,
    },
    SizeMismatch {
        name: String,
    },
    HashMismatch {
        name: String,
    },

    // Check 2: the composition.
    NoAuthors,
    MissingDeathYear {
        author: String,
    },
    MissingFirstPublicationYear,
    /// A claim with no evidence id, or an evidence id the receipt does not record.
    Unevidenced {
        what: &'static str,
    },
    /// First published after the US cut-off.
    NotPublicDomainUs {
        first_publication_year: u16,
    },
    /// An author died after the EU cut-off.
    NotPublicDomainEu {
        author: String,
        death_year: u16,
    },
    /// An author of the work is unknown, and the work was first published after the EU
    /// cut-off for an anonymous work. `role` is the first unknown author's.
    AnonymousNotPublicDomainEu {
        role: AuthorRole,
        first_publication_year: u16,
    },

    // Check 3: the arrangement.
    MissingTypesetter,
    MissingEngraver,
    /// A quoted licence or terms text that is not among its evidence's recorded quotes.
    QuoteNotInEvidence {
        what: &'static str,
    },
    /// An evidence quote that holds the licence or terms text also negates, limits,
    /// prohibits or conditions it (see `licence::negates`), so the evidence does not
    /// affirm it.
    QuoteNegated {
        what: &'static str,
    },
    /// The licence or the terms refuse the score; see [`LicenceRefusal`].
    Licence(LicenceRefusal),
    MissingCreditLedgerId,
    /// A credit-ledger id on a public-domain typesetting: the tiers would mix.
    UnexpectedCreditLedgerId,

    // Check 4: the source edition.
    MissingEditionPublisher,
    MissingEditionYear,
    /// The edition is dated before the work was first published.
    EditionBeforeFirstPublication {
        edition_year: u16,
    },
    /// A scholarly edition still inside its term.
    ScholarlyEditionInTerm {
        edition_year: u16,
    },
    /// An edition too recent for its date alone to show it is out of term, and nothing
    /// else on the receipt shows it.
    EditionTermNotShown {
        edition_year: u16,
    },
    /// An anonymous edition published after the US cut-off.
    AnonymousEditionNotPublicDomainUs {
        edition_year: u16,
    },
    /// An anonymous edition published after the EU cut-off for an anonymous work.
    AnonymousEditionNotPublicDomainEu {
        edition_year: u16,
    },

    // Check 5: the in-file licence.
    /// A file could not be read for licence statements.
    Unreadable {
        name: String,
        why: Unreadable,
    },
    /// The receipt's record of a file's statements differs from what the file says.
    InFileMisrecorded {
        name: String,
    },
    /// A statement in a file disagrees with the host page's licence.
    InFileLicenceMismatch {
        name: String,
    },
    /// No file states a licence equal to the host page's.
    NoInFileLicence,
    /// A file of this project's own engraving states a licence other than
    /// [`crate::OWN_ENGRAVING_LICENCE`], `CC0 1.0`, the licence of the project's own
    /// engravings.
    OwnEngravingStatesLicence {
        name: String,
    },
}

/// Admits or refuses a score. Pure: no clock, no I/O, no global state.
///
/// Checks run in this order and the first failure is returned:
///
/// 0. the receipt's structure;
/// 1. the files: every listed file supplied once, nothing else supplied, sizes and
///    SHA-256 equal to the receipt;
/// 2. the composition: authors, a death year for every named author, the
///    first-publication year and their evidence present; then public domain by this law
///    version's cut-offs: when an author is unknown, the EU rule for an anonymous work
///    (first published at or before [`crate::EU_LAST_PUBLIC_DOMAIN_ANONYMOUS_PUBLICATION_YEAR`]);
///    the US rule (first published at or before
///    [`crate::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR`]); and the EU rule for every named
///    author (dead at or before [`crate::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR`]);
/// 3. the arrangement: a named typesetter; the page licence and the terms each held, as
///    whole words, by a quote of their evidence; no restriction declared in the terms and
///    no restriction phrase in those quotes; no quote holding either text that also
///    negates it; an admitted licence; and the credit-ledger id present exactly when the
///    tier needs one. Or an engraving by this project;
/// 4. the source edition: publisher, year and evidence present, the year not before first
///    publication; for an anonymous edition, the year at or before both jurisdictions'
///    cut-offs for an anonymous work, the EU's checked first; and the edition shown to be
///    out of any scholarly-edition term;
/// 5. the in-file licence: every file read again, and its statements equal to the
///    receipt's record of them. For a third-party typesetting, a statement that names a
///    restriction is refused by that restriction's name; every other statement must agree
///    with the host page's licence (equal to it, or for a markup, holding it as a whole
///    phrase without negating it); and at least one file must state it outright. For this
///    project's own engraving, which has no host page, a file may state
///    [`crate::OWN_ENGRAVING_LICENCE`] and nothing else.
///
/// The EU rule for an anonymous work runs before the US rule, where the named path's EU
/// rule runs after it. Both publication rules read one year, and the anonymous EU cut-off
/// (1955 in rules year 2026) is later than the US one (1930), so after the US rule it
/// could never refuse anything, and a change that deleted it would go unseen. Checked
/// first, each rule refuses years of its own: 1931 to 1955 by the US rule, 1956 on by the
/// EU rule. A receipt with no unknown author meets its checks in version 2's order.
pub fn admit(receipt: &Receipt, supplied: &[Supplied<'_>]) -> Result<Admitted, Refusal> {
    receipt.check_structure().map_err(Refusal::Receipt)?;
    let files = check_files(receipt, supplied)?;
    check_composition(receipt)?;
    let tier = check_arrangement(receipt)?;
    check_edition(receipt)?;
    match &receipt.arrangement {
        Arrangement::ThirdParty(third) => check_in_file(receipt, third, &files)?,
        Arrangement::ThisProject(_) => check_own_files(receipt, &files)?,
    }
    Ok(Admitted {
        tier,
        receipt_digest: receipt.digest(),
    })
}

/// The supplied files by name, once check 1 has passed.
type Files<'a> = BTreeMap<&'a str, &'a [u8]>;

fn check_files<'a>(receipt: &Receipt, supplied: &[Supplied<'a>]) -> Result<Files<'a>, Refusal> {
    let mut files = Files::new();
    for s in supplied {
        if files.insert(s.name, s.bytes).is_some() {
            return Err(Refusal::DuplicateFile {
                name: String::from(s.name),
            });
        }
        if receipt.file(s.name).is_none() {
            return Err(Refusal::UnexpectedFile {
                name: String::from(s.name),
            });
        }
    }
    for f in &receipt.files {
        let name = || f.name.clone();
        let Some(bytes) = files.get(f.name.as_str()) else {
            return Err(Refusal::MissingFile { name: name() });
        };
        if bytes.len() as u64 != f.bytes {
            return Err(Refusal::SizeMismatch { name: name() });
        }
        if sha256(bytes) != f.sha256 {
            return Err(Refusal::HashMismatch { name: name() });
        }
    }
    Ok(files)
}

fn check_composition(receipt: &Receipt) -> Result<(), Refusal> {
    let c = &receipt.composition;
    if c.authors.is_empty() {
        return Err(Refusal::NoAuthors);
    }
    // A named author needs a death year. An unknown author has none (the structure check
    // refuses one that does); its term runs from the work's publication instead.
    for a in &c.authors {
        if let Some(name) = &a.name
            && a.death_year.is_none()
        {
            return Err(Refusal::MissingDeathYear {
                author: name.clone(),
            });
        }
    }
    let Some(first) = c.first_publication_year else {
        return Err(Refusal::MissingFirstPublicationYear);
    };
    evidenced(receipt, &c.evidence, "composition")?;
    // An anonymous work's EU term runs 70 years from its publication (Directive
    // 2006/116/EC, Art. 1(3)). It is checked before the US rule, for the reason on
    // [`admit`].
    if let Some(a) = c.authors.iter().find(|a| a.name.is_none())
        && first > EU_LAST_PUBLIC_DOMAIN_ANONYMOUS_PUBLICATION_YEAR
    {
        return Err(Refusal::AnonymousNotPublicDomainEu {
            role: a.role,
            first_publication_year: first,
        });
    }
    if first > US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR {
        return Err(Refusal::NotPublicDomainUs {
            first_publication_year: first,
        });
    }
    for a in &c.authors {
        // An unknown author's EU term was checked above.
        let Some(name) = &a.name else {
            continue;
        };
        let death_year = a.death_year.unwrap_or(u16::MAX);
        if death_year > EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR {
            return Err(Refusal::NotPublicDomainEu {
                author: name.clone(),
                death_year,
            });
        }
    }
    Ok(())
}

fn check_arrangement(receipt: &Receipt) -> Result<Tier, Refusal> {
    let third = match &receipt.arrangement {
        Arrangement::ThisProject(p) => {
            return if p.engraver.trim().is_empty() {
                Err(Refusal::MissingEngraver)
            } else {
                Ok(Tier::OwnEngraving)
            };
        }
        Arrangement::ThirdParty(third) => third,
    };
    if third.typesetter.trim().is_empty() {
        return Err(Refusal::MissingTypesetter);
    }
    let Some(page) = licence::normalise(&third.page_licence.text) else {
        return Err(Refusal::Licence(LicenceRefusal::Unknown));
    };
    let page_quotes = quoted(
        receipt,
        &third.page_licence.evidence,
        &third.page_licence.text,
        "page licence",
    )?;
    let terms_quotes = quoted(receipt, &third.terms.evidence, &third.terms.text, "terms")?;
    // Restrictions are sorted by tag, so the first one is also the lowest tag: the reason
    // named does not depend on the order the receipt's author found them in.
    if let Some(&first) = third.terms.restrictions.first() {
        return Err(Refusal::Licence(first.into()));
    }
    // A restriction phrase in a quote that holds the licence or the terms refuses by name,
    // whatever the page licence says.
    for quote in page_quotes.iter().chain(&terms_quotes) {
        if let Some(class) = licence::restriction_in(quote) {
            return Err(Refusal::Licence(class));
        }
    }
    // Evidence that holds the text but denies it does not affirm it.
    if page_quotes.iter().any(|q| licence::negates(q)) {
        return Err(Refusal::QuoteNegated {
            what: "page licence",
        });
    }
    if terms_quotes.iter().any(|q| licence::negates(q)) {
        return Err(Refusal::QuoteNegated { what: "terms" });
    }
    match licence::admitted_class(&page) {
        None => Err(Refusal::Licence(licence::refusal_for(&page))),
        Some(AdmittedClass::PublicDomain) => match &third.credit_ledger_id {
            None => Ok(Tier::PublicDomain),
            Some(_) => Err(Refusal::UnexpectedCreditLedgerId),
        },
        Some(AdmittedClass::CcBy40) => match &third.credit_ledger_id {
            Some(id) if !id.trim().is_empty() => Ok(Tier::CcBy40 {
                credit_ledger_id: id.clone(),
            }),
            _ => Err(Refusal::MissingCreditLedgerId),
        },
    }
}

fn check_edition(receipt: &Receipt) -> Result<(), Refusal> {
    let e = &receipt.source_edition;
    if e.publisher.as_deref().is_none_or(|p| p.trim().is_empty()) {
        return Err(Refusal::MissingEditionPublisher);
    }
    let Some(year) = e.year else {
        return Err(Refusal::MissingEditionYear);
    };
    evidenced(receipt, &e.evidence, "source edition")?;
    if let Some(first) = receipt.composition.first_publication_year
        && year < first
    {
        return Err(Refusal::EditionBeforeFirstPublication { edition_year: year });
    }
    // What an anonymous edition adds is an anonymous work of its own, first published in
    // the edition's year: the EU and US publication rules apply to it, the EU's first, for
    // the reason on [`admit`].
    if e.kind == EditionKind::Anonymous {
        if year > EU_LAST_PUBLIC_DOMAIN_ANONYMOUS_PUBLICATION_YEAR {
            return Err(Refusal::AnonymousEditionNotPublicDomainEu { edition_year: year });
        }
        if year > US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR {
            return Err(Refusal::AnonymousEditionNotPublicDomainUs { edition_year: year });
        }
    }
    if year <= LAST_OUT_OF_TERM_EDITION_YEAR {
        return Ok(());
    }
    Err(match e.kind {
        EditionKind::Scholarly => Refusal::ScholarlyEditionInTerm { edition_year: year },
        _ => Refusal::EditionTermNotShown { edition_year: year },
    })
}

/// Reads a file's licence statements again and checks the receipt recorded them exactly.
fn recorded_statements(f: &FileEntry, files: &Files<'_>) -> Result<Vec<Statement>, Refusal> {
    // Check 1 guarantees exactly one supplied file per listed file.
    let Some(bytes) = files.get(f.name.as_str()) else {
        return Err(Refusal::MissingFile {
            name: f.name.clone(),
        });
    };
    let found = infile::statements(f.media, bytes).map_err(|why| Refusal::Unreadable {
        name: f.name.clone(),
        why,
    })?;
    if found != f.in_file_licence {
        return Err(Refusal::InFileMisrecorded {
            name: f.name.clone(),
        });
    }
    Ok(found)
}

/// Check 5 for this project's own engraving: a file may state the licence of the
/// project's own engravings, [`OWN_ENGRAVING_LICENCE`], in any field, and nothing else.
fn check_own_files(receipt: &Receipt, files: &Files<'_>) -> Result<(), Refusal> {
    let own = licence::normalise(OWN_ENGRAVING_LICENCE);
    for f in &receipt.files {
        for st in recorded_statements(f, files)? {
            if licence::normalise(&st.text) != own {
                return Err(Refusal::OwnEngravingStatesLicence {
                    name: f.name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn check_in_file(receipt: &Receipt, third: &ThirdParty, files: &Files<'_>) -> Result<(), Refusal> {
    // Check 3 has already refused a page licence that does not normalise.
    let page = licence::normalise(&third.page_licence.text)
        .ok_or(Refusal::Licence(LicenceRefusal::Unknown))?;
    let mut stated = false;
    for f in &receipt.files {
        let found = recorded_statements(f, files)?;
        for st in &found {
            let Some(text) = licence::normalise(&st.text) else {
                return Err(Refusal::InFileLicenceMismatch {
                    name: f.name.clone(),
                });
            };
            // A file that names a restriction is refused by that restriction's name.
            if let Some(class) = licence::restriction_in(&text) {
                return Err(Refusal::Licence(class));
            }
            // A statement equals the page licence; a markup, which is prose, must hold the
            // licence as a whole phrase and affirm it.
            let agrees = if st.field == StatementField::LilypondCopyrightMarkup {
                licence::contains_phrase(&text, &page) && !licence::negates(&text)
            } else {
                text == page
            };
            if !agrees {
                return Err(Refusal::InFileLicenceMismatch {
                    name: f.name.clone(),
                });
            }
            if st.field != StatementField::LilypondCopyrightMarkup {
                stated = true;
            }
        }
    }
    if stated {
        Ok(())
    } else {
        Err(Refusal::NoInFileLicence)
    }
}

/// Every id is recorded, and there is at least one.
fn evidenced(receipt: &Receipt, ids: &[String], what: &'static str) -> Result<(), Refusal> {
    if ids.is_empty() || ids.iter().any(|id| receipt.evidence(id).is_none()) {
        return Err(Refusal::Unevidenced { what });
    }
    Ok(())
}

/// The normalised quotes of the evidence that hold the text: verbatim, and as whole words
/// once both are normalised. At least one is required. A quote that does not normalise
/// never holds anything.
fn quoted(
    receipt: &Receipt,
    evidence: &str,
    text: &str,
    what: &'static str,
) -> Result<Vec<String>, Refusal> {
    let Some(e) = receipt.evidence(evidence) else {
        return Err(Refusal::Unevidenced { what });
    };
    let Some(phrase) = licence::normalise(text) else {
        return Err(Refusal::QuoteNotInEvidence { what });
    };
    let holding: Vec<String> = e
        .quotes
        .iter()
        .filter(|q| q.contains(text))
        .filter_map(|q| licence::normalise(q))
        .filter(|q| licence::contains_phrase(q, &phrase))
        .collect();
    if holding.is_empty() {
        return Err(Refusal::QuoteNotInEvidence { what });
    }
    Ok(holding)
}

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
