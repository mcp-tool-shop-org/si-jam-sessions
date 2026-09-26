//! The predicate's tests: the real Entertainer receipt, and a synthetic fixture that each
//! test breaks in exactly one way.

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::predicate::sha256;
use crate::*;

const REAL_JSON: &[u8] = include_bytes!("../../../scores/entertainer/receipt.json");
const REAL_LY: &[u8] = include_bytes!("../../../scores/entertainer/entertainer.ly");
const REAL_MID: &[u8] = include_bytes!("../../../scores/entertainer/entertainer.mid");

/// SHA-256 of the real receipt's canonical encoding: a regression pin on the receipt and
/// the encoding together. It moves only when one of them is changed on purpose.
/// Cross-checked against an independent encoder written from the layout in `canonical.rs`.
const REAL_DIGEST: &str = "e23ba2e9a9c16704564485089f31899c316dd7b398ba10bd7628bef5605f282c";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn real() -> Receipt {
    Receipt::from_json(REAL_JSON).expect("the committed receipt loads")
}

fn real_supplied() -> [Supplied<'static>; 2] {
    [
        Supplied {
            name: "entertainer.ly",
            bytes: REAL_LY,
        },
        Supplied {
            name: "entertainer.mid",
            bytes: REAL_MID,
        },
    ]
}

// ---------------------------------------------------------------------------------------
// The real receipt.

#[test]
fn the_entertainer_is_admitted_as_public_domain() {
    let receipt = real();
    let admitted = admit(&receipt, &real_supplied()).expect("admitted");
    assert_eq!(admitted.tier, Tier::PublicDomain);
    assert_eq!(admitted.receipt_digest, receipt.digest());
}

#[test]
fn the_real_files_state_their_licences_as_recorded() {
    assert_eq!(
        statements(Media::Lilypond, REAL_LY).unwrap(),
        vec![
            Statement {
                field: StatementField::LilypondLicense,
                text: "Public Domain".to_owned(),
            },
            Statement {
                field: StatementField::LilypondCopyrightMarkup,
                text: "Mutopia Project Typeset using LilyPond by Placed in the public domain \
                       by the typesetter free to distribute, modify, and perform"
                    .to_owned(),
            },
        ]
    );
    assert_eq!(statements(Media::Smf, REAL_MID).unwrap(), vec![]);
}

#[test]
fn the_real_receipt_round_trips_and_its_digest_is_pinned() {
    let receipt = real();
    let bytes = receipt.to_canonical();
    assert_eq!(&bytes[..4], &MAGIC);
    let decoded = Receipt::from_canonical(&bytes).unwrap();
    assert_eq!(decoded, receipt);
    assert_eq!(decoded.to_canonical(), bytes);
    assert_eq!(hex(&receipt.digest()), REAL_DIGEST);
}

#[test]
fn the_real_receipt_names_what_a_receipt_must() {
    let r = real();
    assert_eq!(
        r.fetched_on,
        Date {
            year: 2026,
            month: 9,
            day: 25
        }
    );
    let Arrangement::ThirdParty(third) = &r.arrangement else {
        panic!("third party");
    };
    assert_eq!(third.record, "Mutopia-2016/11/25-263");
    assert_eq!(third.typesetter, "Chris Sawer");
    assert_eq!(third.page_licence.text, "Public Domain");
    let page = r.evidence("mutopia-piece-page").unwrap();
    assert_eq!(
        page.url,
        "https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=263"
    );
    assert_eq!(r.composition.authors[0].death_year, Some(1917));
    assert_eq!(r.composition.first_publication_year, Some(1902));
    assert_eq!(r.source_edition.year, Some(1902));
    assert_eq!(
        r.source_edition.publisher.as_deref(),
        Some("John Stark & Son")
    );
    for f in &r.files {
        let bytes = if f.media == Media::Smf {
            REAL_MID
        } else {
            REAL_LY
        };
        assert_eq!(f.sha256, sha256(bytes), "{}", f.name);
    }
    // A receipt carries no local paths.
    let text = core::str::from_utf8(REAL_JSON).unwrap();
    for needle in [":\\", ":/", "/home/", "\\Users\\", "/Users/", "file:"] {
        let allowed = needle == ":/" && text.matches(":/").count() == text.matches("://").count();
        assert!(allowed || !text.contains(needle), "{needle}");
    }
}

#[test]
fn whitespace_in_the_json_does_not_move_the_digest() {
    let text = core::str::from_utf8(REAL_JSON).unwrap();
    let mut compact = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in text.chars() {
        if in_string {
            compact.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            compact.push(c);
        } else if !c.is_ascii_whitespace() {
            compact.push(c);
        }
    }
    assert_ne!(compact.as_bytes(), REAL_JSON);
    let escaped_slash = compact.replacen("https://www.loc.gov", "https:\\/\\/www.loc.gov", 1);
    for variant in [compact.as_bytes(), escaped_slash.as_bytes()] {
        assert_eq!(
            Receipt::from_json(variant).unwrap().digest(),
            real().digest()
        );
    }
}

// ---------------------------------------------------------------------------------------
// A synthetic fixture: a valid receipt for two small files. Each test changes one thing.

const LY_NAME: &str = "piece.ly";
const MID_NAME: &str = "piece.mid";

struct Fixture {
    receipt: Receipt,
    ly: Vec<u8>,
    mid: Vec<u8>,
}

fn ly_with(header_lines: &str) -> Vec<u8> {
    format!("\\version \"2.24.0\"\n\\header {{\n  title = \"Piece\"\n{header_lines}}}\n{{ c'4 }}\n")
        .into_bytes()
}

/// A one-note format-0 SMF at 96 PPQ with the given meta events first.
fn smf_with(metas: &[(u8, &[u8])], declared_tracks: u8) -> Vec<u8> {
    let mut track = Vec::new();
    for &(kind, text) in metas {
        track.extend_from_slice(&[0x00, 0xFF, kind, u8::try_from(text.len()).unwrap()]);
        track.extend_from_slice(text);
    }
    track.extend_from_slice(&[
        0x00, 0x90, 60, 64, 0x60, 0x80, 60, 0, 0x00, 0xFF, 0x2F, 0x00,
    ]);
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd\0\0\0\x06\0\0\0");
    out.push(declared_tracks);
    out.extend_from_slice(b"\0\x60MTrk");
    out.extend_from_slice(&u32::try_from(track.len()).unwrap().to_be_bytes());
    out.extend_from_slice(&track);
    out
}

fn evidence(id: &str, quotes: &[&str]) -> Evidence {
    Evidence {
        id: id.to_owned(),
        url: format!("https://example.org/{id}"),
        resolved_url: None,
        sha256: sha256(id.as_bytes()),
        bytes: 1,
        quotes: quotes.iter().map(|q| (*q).to_owned()).collect(),
    }
}

fn named(name: &str) -> String {
    name.to_owned()
}

impl Fixture {
    fn new() -> Self {
        let file = |name: &str, media| FileEntry {
            name: name.to_owned(),
            media,
            url: format!("https://example.org/{name}"),
            sha256: [0; 32],
            bytes: 0,
            last_modified: None,
            in_file_licence: vec![],
        };
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA,
            score_id: named("piece"),
            title: named("Piece"),
            fetched_on: Date {
                year: 2026,
                month: 9,
                day: 25,
            },
            composition: Composition {
                authors: vec![Author {
                    name: Some(named("A. Composer")),
                    role: AuthorRole::Composer,
                    death_year: Some(1900),
                }],
                first_publication_year: Some(1899),
                evidence: vec![named("biblio")],
            },
            source_edition: SourceEdition {
                statement: named("Original edition (1899)"),
                publisher: Some(named("A Publisher")),
                year: Some(1899),
                kind: EditionKind::FirstEdition,
                evidence: vec![named("biblio")],
            },
            arrangement: Arrangement::ThirdParty(Box::new(ThirdParty {
                host: named("Host"),
                record: named("rec-1"),
                typesetter: named("T. Setter"),
                contributors: vec![],
                page_licence: Quote {
                    evidence: named("page"),
                    text: named("Public Domain"),
                },
                terms: Terms {
                    evidence: named("terms"),
                    text: named("Dedicated to the public domain."),
                    restrictions: vec![],
                },
                credit_ledger_id: None,
            })),
            files: vec![file(LY_NAME, Media::Lilypond), file(MID_NAME, Media::Smf)],
            evidence: vec![
                evidence("biblio", &["First edition, 1899"]),
                evidence("page", &["Copyright: Public Domain"]),
                evidence("terms", &["Dedicated to the public domain."]),
            ],
            notes: vec![],
        };
        let mut f = Fixture {
            receipt,
            ly: ly_with("  license = \"Public Domain\"\n"),
            mid: smf_with(&[(0x03, b"Piece")], 1),
        };
        f.refresh();
        f
    }

    /// Re-records every file's size, hash and statements from its current bytes, so a
    /// test that changes a file breaks only the check it means to.
    fn refresh(&mut self) {
        for f in &mut self.receipt.files {
            let bytes = if f.name == LY_NAME {
                &self.ly
            } else {
                &self.mid
            };
            f.sha256 = sha256(bytes);
            f.bytes = bytes.len() as u64;
            f.in_file_licence = statements(f.media, bytes).unwrap_or_default();
        }
    }

    fn supplied(&self) -> Vec<Supplied<'_>> {
        vec![
            Supplied {
                name: LY_NAME,
                bytes: &self.ly,
            },
            Supplied {
                name: MID_NAME,
                bytes: &self.mid,
            },
        ]
    }

    fn admit(&self) -> Result<Admitted, Refusal> {
        admit(&self.receipt, &self.supplied())
    }

    fn refused(&self) -> Refusal {
        self.admit().expect_err("refused")
    }

    fn third(&mut self) -> &mut ThirdParty {
        match &mut self.receipt.arrangement {
            Arrangement::ThirdParty(third) => third,
            Arrangement::ThisProject(_) => panic!("third party"),
        }
    }

    fn evidence_mut(&mut self, id: &str) -> &mut Evidence {
        self.receipt
            .evidence
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
    }

    /// Sets the page licence and records it among the page's quotes.
    fn page_licence(&mut self, text: &str) {
        self.third().page_licence.text = text.to_owned();
        self.evidence_mut("page").quotes = vec![format!("Copyright: {text}")];
    }

    /// Replaces the LilyPond file's header lines and re-records it.
    fn ly_header(&mut self, lines: &str) {
        self.ly = ly_with(lines);
        self.refresh();
    }
}

#[test]
fn the_fixture_is_admitted_as_public_domain() {
    let f = Fixture::new();
    let admitted = f.admit().unwrap();
    assert_eq!(admitted.tier, Tier::PublicDomain);
    assert_eq!(admitted.receipt_digest, f.receipt.digest());
}

// Check 0: structure.

#[test]
fn a_receipt_that_breaks_structure_is_refused() {
    let mut f = Fixture::new();
    f.receipt.files.swap(0, 1);
    assert_eq!(f.refused(), Refusal::Receipt(ReceiptError::FilesNotSorted));
    let mut f = Fixture::new();
    f.receipt.schema = 2;
    assert_eq!(
        f.refused(),
        Refusal::Receipt(ReceiptError::UnsupportedSchema(2))
    );
}

// Check 1: files.

#[test]
fn every_listed_file_must_be_supplied() {
    let f = Fixture::new();
    let only_ly = [Supplied {
        name: LY_NAME,
        bytes: &f.ly,
    }];
    assert_eq!(
        admit(&f.receipt, &only_ly),
        Err(Refusal::MissingFile {
            name: named(MID_NAME)
        })
    );
}

#[test]
fn nothing_unlisted_may_be_supplied() {
    let f = Fixture::new();
    let mut supplied = f.supplied();
    supplied.push(Supplied {
        name: "extra.mid",
        bytes: &f.mid,
    });
    assert_eq!(
        admit(&f.receipt, &supplied),
        Err(Refusal::UnexpectedFile {
            name: named("extra.mid")
        })
    );
}

#[test]
fn a_file_may_be_supplied_once() {
    let f = Fixture::new();
    let mut supplied = f.supplied();
    supplied.push(supplied[1]);
    assert_eq!(
        admit(&f.receipt, &supplied),
        Err(Refusal::DuplicateFile {
            name: named(MID_NAME)
        })
    );
}

#[test]
fn sizes_and_hashes_must_match_the_receipt() {
    let mut f = Fixture::new();
    f.mid.push(0);
    assert_eq!(
        f.refused(),
        Refusal::SizeMismatch {
            name: named(MID_NAME)
        }
    );
    let mut f = Fixture::new();
    let last = f.ly.len() - 2;
    f.ly[last] ^= 1;
    assert_eq!(
        f.refused(),
        Refusal::HashMismatch {
            name: named(LY_NAME)
        }
    );
}

// Check 2: composition.

#[test]
fn authors_and_their_years_must_be_present() {
    let mut f = Fixture::new();
    f.receipt.composition.authors.clear();
    assert_eq!(f.refused(), Refusal::NoAuthors);

    let mut f = Fixture::new();
    f.receipt.composition.authors[0].death_year = None;
    assert_eq!(
        f.refused(),
        Refusal::MissingDeathYear {
            author: named("A. Composer")
        }
    );

    let mut f = Fixture::new();
    f.receipt.composition.first_publication_year = None;
    assert_eq!(f.refused(), Refusal::MissingFirstPublicationYear);
}

#[test]
fn composition_years_need_recorded_evidence() {
    let mut f = Fixture::new();
    f.receipt.composition.evidence.clear();
    assert_eq!(
        f.refused(),
        Refusal::Unevidenced {
            what: "composition"
        }
    );
    let mut f = Fixture::new();
    f.receipt.composition.evidence = vec![named("nowhere")];
    assert_eq!(
        f.refused(),
        Refusal::Unevidenced {
            what: "composition"
        }
    );
}

#[test]
fn the_us_cut_off_is_publication_in_1930_or_earlier() {
    let mut f = Fixture::new();
    f.receipt.composition.first_publication_year = Some(1930);
    f.receipt.source_edition.year = Some(1930);
    assert!(f.admit().is_ok());
    f.receipt.composition.first_publication_year = Some(1931);
    assert_eq!(
        f.refused(),
        Refusal::NotPublicDomainUs {
            first_publication_year: 1931
        }
    );
}

#[test]
fn the_eu_cut_off_is_every_author_dead_by_1955() {
    let mut f = Fixture::new();
    f.receipt.composition.authors[0].death_year = Some(1955);
    assert!(f.admit().is_ok());
    f.receipt.composition.authors[0].death_year = Some(1956);
    assert_eq!(
        f.refused(),
        Refusal::NotPublicDomainEu {
            author: named("A. Composer"),
            death_year: 1956
        }
    );
    // A lyricist counts: the term runs from the last death.
    let mut f = Fixture::new();
    f.receipt.composition.authors.push(Author {
        name: Some(named("A. Lyricist")),
        role: AuthorRole::Lyricist,
        death_year: Some(1970),
    });
    assert_eq!(
        f.refused(),
        Refusal::NotPublicDomainEu {
            author: named("A. Lyricist"),
            death_year: 1970
        }
    );
}

// Check 3: arrangement, and the licence refusal classes.

#[test]
fn a_third_party_typesetting_needs_a_typesetter() {
    let mut f = Fixture::new();
    f.third().typesetter = named("  ");
    assert_eq!(f.refused(), Refusal::MissingTypesetter);
}

#[test]
fn licence_and_terms_quotes_must_be_in_their_evidence() {
    let mut f = Fixture::new();
    f.evidence_mut("page").quotes = vec![named("Copyright: see elsewhere")];
    assert_eq!(
        f.refused(),
        Refusal::QuoteNotInEvidence {
            what: "page licence"
        }
    );
    let mut f = Fixture::new();
    f.third().page_licence.evidence = named("nowhere");
    assert_eq!(
        f.refused(),
        Refusal::Unevidenced {
            what: "page licence"
        }
    );
    let mut f = Fixture::new();
    f.third().terms.text = named("Some other terms.");
    assert_eq!(f.refused(), Refusal::QuoteNotInEvidence { what: "terms" });
    let mut f = Fixture::new();
    f.third().terms.text = named("");
    assert_eq!(f.refused(), Refusal::QuoteNotInEvidence { what: "terms" });
}

#[test]
fn every_restriction_in_the_terms_refuses_by_name() {
    for &r in Restriction::ALL {
        let mut f = Fixture::new();
        f.third().terms.restrictions = vec![r];
        assert_eq!(f.refused(), Refusal::Licence(r.into()), "{r:?}");
    }
    let mut f = Fixture::new();
    f.third().terms.restrictions = vec![Restriction::AiRestricted];
    assert_eq!(f.refused(), Refusal::Licence(LicenceRefusal::AiRestricted));
}

#[test]
fn each_licence_refusal_class_is_named_from_the_page() {
    let cases = [
        ("Unknown Licence 1.0", LicenceRefusal::Unknown),
        ("CC0", LicenceRefusal::Unknown),
        ("Creative Commons Attribution 3.0", LicenceRefusal::Unknown),
        (
            "Copyright 2019 A Publisher. All rights reserved.",
            LicenceRefusal::AllRightsReserved,
        ),
        ("Not for redistribution", LicenceRefusal::NoRedistribution),
        (
            "Creative Commons Attribution-ShareAlike 4.0",
            LicenceRefusal::ShareAlike,
        ),
        ("CC BY-NC 4.0", LicenceRefusal::NonCommercial),
        (
            "Creative Commons Attribution-NoDerivatives 4.0",
            LicenceRefusal::NoDerivatives,
        ),
    ];
    for (text, class) in cases {
        let mut f = Fixture::new();
        f.page_licence(text);
        assert_eq!(f.refused(), Refusal::Licence(class), "{text}");
    }
}

#[test]
fn a_page_licence_that_does_not_normalise_is_unknown() {
    let mut f = Fixture::new();
    f.page_licence("Public\u{7}Domain");
    assert_eq!(f.refused(), Refusal::Licence(LicenceRefusal::Unknown));
}

#[test]
fn cc_by_4_is_its_own_tier_and_carries_a_credit() {
    let mut f = Fixture::new();
    f.page_licence("Creative Commons Attribution 4.0");
    f.ly_header("  license = \"Creative Commons Attribution 4.0\"\n");
    assert_eq!(f.refused(), Refusal::MissingCreditLedgerId);
    f.third().credit_ledger_id = Some(named(" "));
    assert_eq!(f.refused(), Refusal::MissingCreditLedgerId);
    f.third().credit_ledger_id = Some(named("credit-0001"));
    assert_eq!(
        f.admit().unwrap().tier,
        Tier::CcBy40 {
            credit_ledger_id: named("credit-0001")
        }
    );
}

#[test]
fn a_public_domain_typesetting_carries_no_credit() {
    let mut f = Fixture::new();
    f.third().credit_ledger_id = Some(named("credit-0001"));
    assert_eq!(f.refused(), Refusal::UnexpectedCreditLedgerId);
}

#[test]
fn an_own_engraving_is_its_own_tier() {
    let mut f = Fixture::new();
    // No host page to compare with: the files must state no licence of their own.
    f.ly_header("");
    f.receipt.arrangement = Arrangement::ThisProject(ThisProject {
        engraver: named("si-jam-sessions"),
    });
    assert_eq!(f.admit().unwrap().tier, Tier::OwnEngraving);
    f.receipt.arrangement = Arrangement::ThisProject(ThisProject {
        engraver: named(""),
    });
    assert_eq!(f.refused(), Refusal::MissingEngraver);
    // The composition rules still apply.
    let mut f = Fixture::new();
    f.receipt.arrangement = Arrangement::ThisProject(ThisProject {
        engraver: named("si-jam-sessions"),
    });
    f.receipt.composition.first_publication_year = Some(1950);
    assert!(matches!(f.refused(), Refusal::NotPublicDomainUs { .. }));
}

#[test]
fn an_own_engraving_whose_files_state_a_licence_is_refused() {
    let own = |f: &mut Fixture| {
        f.receipt.arrangement = Arrangement::ThisProject(ThisProject {
            engraver: named("si-jam-sessions"),
        });
    };
    // Any statement refuses, restrictive or not, in either file.
    for header in [
        "  license = \"Copyright 2024. All rights reserved.\"\n",
        "  license = \"Public Domain\"\n",
        "  copyright = \\markup { \"Engraved 2026\" }\n",
    ] {
        let mut f = Fixture::new();
        f.ly_header(header);
        own(&mut f);
        assert_eq!(
            f.refused(),
            Refusal::OwnEngravingStatesLicence {
                name: named(LY_NAME)
            },
            "{header}"
        );
    }
    let mut f = Fixture::new();
    f.ly_header("");
    f.mid = smf_with(&[(0x02, b"All rights reserved")], 1);
    f.refresh();
    own(&mut f);
    assert_eq!(
        f.refused(),
        Refusal::OwnEngravingStatesLicence {
            name: named(MID_NAME)
        }
    );
    // The receipt's record is still checked against the bytes.
    let mut f = Fixture::new();
    own(&mut f);
    f.receipt.files[0].in_file_licence.clear();
    assert_eq!(
        f.refused(),
        Refusal::InFileMisrecorded {
            name: named(LY_NAME)
        }
    );
}

// Check 4: source edition.

#[test]
fn the_source_edition_needs_publisher_year_and_evidence() {
    let mut f = Fixture::new();
    f.receipt.source_edition.publisher = None;
    assert_eq!(f.refused(), Refusal::MissingEditionPublisher);
    f.receipt.source_edition.publisher = Some(named(" "));
    assert_eq!(f.refused(), Refusal::MissingEditionPublisher);

    let mut f = Fixture::new();
    f.receipt.source_edition.year = None;
    assert_eq!(f.refused(), Refusal::MissingEditionYear);

    let mut f = Fixture::new();
    f.receipt.source_edition.evidence.clear();
    assert_eq!(
        f.refused(),
        Refusal::Unevidenced {
            what: "source edition"
        }
    );
}

#[test]
fn an_edition_cannot_predate_first_publication() {
    let mut f = Fixture::new();
    f.receipt.source_edition.year = Some(1898);
    assert_eq!(
        f.refused(),
        Refusal::EditionBeforeFirstPublication { edition_year: 1898 }
    );
}

#[test]
fn an_edition_inside_its_term_is_refused() {
    let mut f = Fixture::new();
    f.receipt.source_edition.kind = EditionKind::Scholarly;
    f.receipt.source_edition.year = Some(2000);
    assert!(f.admit().is_ok(), "2000 is out of term in 2026");
    f.receipt.source_edition.year = Some(2001);
    assert_eq!(
        f.refused(),
        Refusal::ScholarlyEditionInTerm { edition_year: 2001 }
    );
    for kind in [
        EditionKind::Reprint,
        EditionKind::Unknown,
        EditionKind::FirstEdition,
    ] {
        f.receipt.source_edition.kind = kind;
        assert_eq!(
            f.refused(),
            Refusal::EditionTermNotShown { edition_year: 2001 },
            "{kind:?}"
        );
    }
}

// Check 5: in-file licence.

#[test]
fn the_receipt_must_record_what_each_file_states() {
    let mut f = Fixture::new();
    f.receipt.files[0].in_file_licence.clear();
    assert_eq!(
        f.refused(),
        Refusal::InFileMisrecorded {
            name: named(LY_NAME)
        }
    );
    let mut f = Fixture::new();
    f.receipt.files[0].in_file_licence[0].text = named("public domain");
    assert_eq!(
        f.refused(),
        Refusal::InFileMisrecorded {
            name: named(LY_NAME)
        }
    );
    let mut f = Fixture::new();
    f.receipt.files[1].in_file_licence.push(Statement {
        field: StatementField::SmfCopyright,
        text: named("Public Domain"),
    });
    assert_eq!(
        f.refused(),
        Refusal::InFileMisrecorded {
            name: named(MID_NAME)
        }
    );
}

#[test]
fn an_in_file_licence_that_differs_from_the_page_is_refused() {
    // A statement that names a refusal class is refused by that class's name.
    let mut f = Fixture::new();
    f.ly_header("  license = \"Creative Commons Attribution-ShareAlike 4.0\"\n");
    assert_eq!(f.refused(), Refusal::Licence(LicenceRefusal::ShareAlike));
    // A second statement in the same file must agree too.
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain\"\n  copyright = \"Copyright 1990 A. Person\"\n");
    assert_eq!(
        f.refused(),
        Refusal::InFileLicenceMismatch {
            name: named(LY_NAME)
        }
    );
    // So must a statement in another file.
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x02, b"All rights reserved")], 1);
    f.refresh();
    assert_eq!(
        f.refused(),
        Refusal::Licence(LicenceRefusal::AllRightsReserved)
    );
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x01, b"(c) 1998 A Sequencer")], 1);
    f.refresh();
    assert_eq!(
        f.refused(),
        Refusal::InFileLicenceMismatch {
            name: named(MID_NAME)
        }
    );
}

#[test]
fn in_file_comparison_uses_the_minimal_normalisation_only() {
    let mut f = Fixture::new();
    f.ly_header("  license = \"  PUBLIC \\t DOMAIN \"\n");
    assert!(f.admit().is_ok());
    for different in ["Public\u{a0}Domain", "Public-Domain", "Public Domain.", ""] {
        let mut f = Fixture::new();
        f.ly_header(&format!("  license = \"{different}\"\n"));
        assert_eq!(
            f.refused(),
            Refusal::InFileLicenceMismatch {
                name: named(LY_NAME)
            },
            "{different:?}"
        );
    }
}

#[test]
fn a_matching_statement_in_a_second_file_is_fine() {
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x02, b"Public Domain")], 1);
    f.refresh();
    assert!(f.admit().is_ok());
}

#[test]
fn a_copyright_markup_must_contain_the_page_licence_and_no_restriction() {
    let mut f = Fixture::new();
    f.ly_header(
        "  license = \"Public Domain\"\n  copyright = \\markup { \"Placed in the\" \"public domain\" }\n",
    );
    assert!(f.admit().is_ok());

    // A markup without the licence phrase does not agree.
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain\"\n  copyright = \\markup { \"Engraved 2016\" }\n");
    assert_eq!(
        f.refused(),
        Refusal::InFileLicenceMismatch {
            name: named(LY_NAME)
        }
    );

    // A markup that names a restriction is refused by the restriction's name.
    let mut f = Fixture::new();
    f.ly_header(
        "  license = \"Public Domain\"\n  copyright = \\markup { \"public domain in the US; all rights reserved elsewhere\" }\n",
    );
    assert_eq!(
        f.refused(),
        Refusal::Licence(LicenceRefusal::AllRightsReserved)
    );
}

/// Finding 1 of the external review: a markup that holds the phrase but denies it.
#[test]
fn a_copyright_markup_that_negates_the_licence_is_refused() {
    for markup in [
        "not in the public domain",
        "This work is not public domain",
        "no longer public domain",
        "never placed in the public domain",
        "it isn't public domain",
        "it isn\u{2019}t public domain",
        "public domain, except the fingering",
        "public domain in the US only",
        "possibly public domain",
        "public domain?",
        "non-public domain",
    ] {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"{markup}\" }}\n"
        ));
        assert_eq!(
            f.refused(),
            Refusal::InFileLicenceMismatch {
                name: named(LY_NAME)
            },
            "{markup}"
        );
    }
}

/// Finding 2 of the external review: evidence that holds the text but denies it.
#[test]
fn an_evidence_quote_that_negates_the_licence_does_not_support_it() {
    for quote in [
        "This is not Public Domain",
        "Copyright: Public Domain (no longer)",
        "Public Domain? Unclear.",
        "Copyright: Public Domain, except in the EU",
    ] {
        let mut f = Fixture::new();
        f.evidence_mut("page").quotes = vec![named(quote)];
        assert_eq!(
            f.refused(),
            Refusal::QuoteNegated {
                what: "page licence"
            },
            "{quote}"
        );
    }
    // One negating quote beside an affirming one: the evidence contradicts itself.
    let mut f = Fixture::new();
    f.evidence_mut("page").quotes = vec![
        named("Copyright: Public Domain"),
        named("This is not Public Domain"),
    ];
    assert_eq!(
        f.refused(),
        Refusal::QuoteNegated {
            what: "page licence"
        }
    );
    // The terms quote likewise.
    let mut f = Fixture::new();
    f.evidence_mut("terms").quotes = vec![named("Never Dedicated to the public domain.")];
    f.third().terms.text = named("Dedicated to the public domain.");
    assert_eq!(f.refused(), Refusal::QuoteNegated { what: "terms" });
    // Holding the text inside a longer word is not holding it.
    let mut f = Fixture::new();
    f.evidence_mut("page").quotes = vec![named("Copyright: Public Domains")];
    assert_eq!(
        f.refused(),
        Refusal::QuoteNotInEvidence {
            what: "page licence"
        }
    );
}

/// Finding 3 of the external review: an AI restriction is named wherever it is read.
#[test]
fn ai_restrictions_are_refused_wherever_they_are_read() {
    let ai = Refusal::Licence(LicenceRefusal::AiRestricted);

    // The page licence itself.
    let mut f = Fixture::new();
    f.page_licence("No AI training");
    assert_eq!(f.refused(), ai, "page licence");

    // The terms text.
    let mut f = Fixture::new();
    let terms = "Dedicated to the public domain. Not for use in machine learning.";
    f.third().terms.text = named(terms);
    f.evidence_mut("terms").quotes = vec![named(terms)];
    assert_eq!(f.refused(), ai, "terms text");

    // The terms quote around a clean terms text.
    let mut f = Fixture::new();
    f.evidence_mut("terms").quotes = vec![named(
        "Dedicated to the public domain. Text and data mining is reserved.",
    )];
    assert_eq!(f.refused(), ai, "terms quote");

    // The page quote around a clean licence.
    let mut f = Fixture::new();
    f.evidence_mut("page").quotes = vec![named("Copyright: Public Domain (TDM reserved)")];
    assert_eq!(f.refused(), ai, "page quote");

    // A copyright markup.
    let mut f = Fixture::new();
    f.ly_header(
        "  license = \"Public Domain\"\n  copyright = \\markup { \"Placed in the public domain; no AI training\" }\n",
    );
    assert_eq!(f.refused(), ai, "markup");

    // A licence string in the file.
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain, not for artificial intelligence\"\n");
    assert_eq!(f.refused(), ai, "licence string");

    // A text event in the MIDI file, which is the file the law ingests.
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x01, b"Not for AI training")], 1);
    f.refresh();
    assert_eq!(f.refused(), ai, "smf text event");
}

/// The whole-word regression found at `fead502`: a markup whose only restriction word was
/// inflected agreed with the page licence, and was admitted. A restriction's inflections
/// are now refused by its class, wherever they are read.
#[test]
fn an_inflected_restriction_is_refused_by_its_class() {
    use LicenceRefusal::*;
    let markups = [
        ("Public Domain. Free to use noncommercially.", NonCommercial),
        ("Public Domain, Attribution-NoDerivative", NoDerivatives),
        ("Public Domain, noncommercially", NonCommercial),
        ("Public Domain, non-commercial use", NonCommercial),
        ("Public Domain, NonCommercial", NonCommercial),
        ("Public Domain, NoDerivatives", NoDerivatives),
        ("Public Domain, no-derivs", NoDerivatives),
        ("Public Domain, NoDerivative", NoDerivatives),
        ("Public Domain, ShareAlike", ShareAlike),
        ("Public Domain, share-alike", ShareAlike),
        ("Public Domain, all rights reserved", AllRightsReserved),
    ];
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture, class: LicenceRefusal| {
        let got = f.admit().map(|a| a.tier);
        if got != Err(Refusal::Licence(class)) {
            wrong.push(format!("{place}: {got:?}, not {class:?}"));
        }
    };
    for (markup, class) in markups {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"{markup}\" }}\n"
        ));
        check(markup, f, class);
    }
    // The terms quote, around a clean terms text.
    let mut f = Fixture::new();
    f.evidence_mut("terms").quotes = vec![named(
        "Dedicated to the public domain. Use it noncommercially.",
    )];
    check("terms quote", f, NonCommercial);
    // A text event in the MIDI file.
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x01, b"Free to use noncommercially")], 1);
    f.refresh();
    check("smf text event", f, NonCommercial);
    let total = markups.len() + 2;
    assert!(
        wrong.is_empty(),
        "{} of {total} wrong: {wrong:#?}",
        wrong.len()
    );
}

/// The matching rule must not reach the admitted texts: a plain "Public Domain" in the
/// licence string and in the markup, and the real Entertainer, whose digest is unchanged.
#[test]
fn plain_public_domain_and_the_entertainer_are_still_admitted() {
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain\"\n  copyright = \\markup { \"Public Domain\" }\n");
    assert_eq!(f.admit().map(|a| a.tier), Ok(Tier::PublicDomain));

    let receipt = real();
    assert_eq!(hex(&receipt.digest()), REAL_DIGEST);
    let admitted = admit(&receipt, &real_supplied()).expect("the Entertainer is admitted");
    assert_eq!(admitted.tier, Tier::PublicDomain);
    assert_eq!(hex(&admitted.receipt_digest), REAL_DIGEST);
}

/// The second external review, at `390336b`: CC forms written with spaces, and AI wording
/// beyond the listed forms, passed the phrase scan. A markup that held them was admitted.
/// Each is now refused by its class, wherever it is read.
#[test]
fn spaced_cc_forms_and_wider_ai_wording_are_refused_by_their_class() {
    use LicenceRefusal::*;
    let markups = [
        ("Placed in the public domain (CC BY ND)", NoDerivatives),
        ("Placed in the public domain (CC BY NC)", NonCommercial),
        ("Placed in the public domain (CC BY SA)", ShareAlike),
        ("Placed in the public domain (CC BY NC ND)", NonCommercial),
        ("Placed in the public domain (CC BY NC SA)", NonCommercial),
        ("Placed in the public domain; no neural nets", AiRestricted),
        (
            "Placed in the public domain, not for training models",
            AiRestricted,
        ),
        (
            "Placed in the public domain; neural networks excluded",
            AiRestricted,
        ),
    ];
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture, class: LicenceRefusal| {
        let got = f.admit().map(|a| a.tier);
        if got != Err(Refusal::Licence(class)) {
            wrong.push(format!("{place}: {got:?}, not {class:?}"));
        }
    };
    for (markup, class) in markups {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"{markup}\" }}\n"
        ));
        check(markup, f, class);
    }
    // A licence string in the file.
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain (CC BY NC)\"\n");
    check("licence string", f, NonCommercial);
    // The terms quote, around a clean terms text.
    let mut f = Fixture::new();
    f.evidence_mut("terms").quotes = vec![named("Dedicated to the public domain. CC BY SA.")];
    check("terms quote", f, ShareAlike);
    // A text event in the MIDI file.
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x01, b"Free to use, no neural nets")], 1);
    f.refresh();
    check("smf text event", f, AiRestricted);
    let total = markups.len() + 3;
    assert!(
        wrong.is_empty(),
        "{} of {total} wrong: {wrong:#?}",
        wrong.len()
    );
}

/// No stem refuses plain text: a markup that runs a restriction phrase on into an ordinary
/// word, or holds a word that only contains an AI topic word, still agrees with the page
/// licence. At `f4038cc`, the first two were refused.
#[test]
fn plain_text_in_a_markup_is_admitted() {
    let markups = [
        "Placed in the public domain for deep learners of the piano",
        "Placed in the public domain; prepared for the CC by Sarah",
        "Placed in the public domain by a trainee",
        "Placed in the public domain; a modern edition, modest in size",
    ];
    let mut wrong = Vec::new();
    for markup in markups {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"{markup}\" }}\n"
        ));
        let got = f.admit().map(|a| a.tier);
        if got != Ok(Tier::PublicDomain) {
            wrong.push(format!("{markup}: {got:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} wrong: {wrong:#?}",
        wrong.len(),
        markups.len()
    );
}

/// The AI class fails closed on its topic, wherever a licence text is read: a text that
/// names training, models, mining or generative systems is refused as AI-restricted,
/// whatever else it says.
#[test]
fn a_licence_text_that_names_the_ai_topic_is_refused() {
    let ai: Result<Tier, Refusal> = Err(Refusal::Licence(LicenceRefusal::AiRestricted));
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture| {
        let got = f.admit().map(|a| a.tier);
        if got != ai {
            wrong.push(format!("{place}: {got:?}"));
        }
    };
    // A copyright markup.
    let mut f = Fixture::new();
    f.ly_header(
        "  license = \"Public Domain\"\n  copyright = \\markup { \"Placed in the public domain, for the purpose of training any model\" }\n",
    );
    check("markup", f);
    // The terms quote, around a clean terms text.
    let mut f = Fixture::new();
    f.evidence_mut("terms").quotes = vec![named(
        "Dedicated to the public domain. Text mining is welcome.",
    )];
    check("terms quote", f);
    // A text event in the MIDI file.
    let mut f = Fixture::new();
    f.mid = smf_with(&[(0x01, b"Generative arrangement")], 1);
    f.refresh();
    check("smf text event", f);
    assert!(wrong.is_empty(), "{} of 3 wrong: {wrong:#?}", wrong.len());
}

/// The third external review, at `8c4444d`: a public-domain markup that bans only an AI
/// product passed. The product names are AI wording now. A text that prohibits anything
/// no longer affirms its licence, so a prohibition outside the vocabulary is refused too,
/// though not named AI-restricted.
#[test]
fn ai_products_and_prohibitions_in_a_licence_text_are_refused() {
    let ai = Refusal::Licence(LicenceRefusal::AiRestricted);
    let mismatch = Refusal::InFileLicenceMismatch {
        name: named(LY_NAME),
    };
    let markup = |text: &str| {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"Placed in the public domain. {text}\" }}\n"
        ));
        f
    };
    let terms_quote = |text: &str| {
        let mut f = Fixture::new();
        f.evidence_mut("terms").quotes = vec![format!("Dedicated to the public domain. {text}")];
        f
    };
    let cases = [
        (
            "ChatGPT use is prohibited",
            markup("ChatGPT use is prohibited"),
            ai.clone(),
        ),
        (
            "OpenAI use is prohibited",
            markup("OpenAI use is prohibited"),
            ai.clone(),
        ),
        (
            "GPT-4 use is prohibited",
            markup("GPT-4 use is prohibited"),
            ai.clone(),
        ),
        ("no ML", markup("no ML"), ai.clone()),
        (
            "terms: ChatGPT use is prohibited",
            terms_quote("ChatGPT use is prohibited."),
            ai.clone(),
        ),
        (
            "Claude use is prohibited",
            markup("Claude use is prohibited"),
            mismatch.clone(),
        ),
        (
            "commercial use prohibited",
            markup("commercial use prohibited"),
            mismatch.clone(),
        ),
        (
            "terms: commercial use prohibited",
            terms_quote("Commercial use prohibited."),
            Refusal::QuoteNegated { what: "terms" },
        ),
    ];
    let mut wrong = Vec::new();
    for (place, f, want) in &cases {
        let got = f.admit().map(|a| a.tier);
        if got != Err(want.clone()) {
            wrong.push(format!("{place}: {got:?}, not {want:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} wrong: {wrong:#?}",
        wrong.len(),
        cases.len()
    );
}

/// The fourth external review, at `0196e6d`: the nouns "restriction" and "restrictions"
/// negated, so the Public Domain Mark's own sentence refused a public-domain markup or
/// evidence quote that carried it. The nouns no longer negate; the verb forms and "no"
/// still do.
#[test]
fn the_public_domain_mark_sentence_affirms() {
    const PDM: &str = "This work has been identified as being free of known restrictions \
                       under copyright law, including all related and neighboring rights.";
    const PDM_SHORT: &str = "This work is free of known copyright restrictions.";
    let markup = |text: &str| {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"Placed in the public domain. {text}\" }}\n"
        ));
        f
    };
    let mismatch = Err(Refusal::InFileLicenceMismatch {
        name: named(LY_NAME),
    });
    let mut terms = Fixture::new();
    terms.evidence_mut("terms").quotes = vec![format!("Dedicated to the public domain. {PDM}")];
    let cases = [
        ("markup: the mark", markup(PDM), Ok(Tier::PublicDomain)),
        ("terms quote: the mark", terms, Ok(Tier::PublicDomain)),
        (
            "markup: the short form",
            markup(PDM_SHORT),
            Ok(Tier::PublicDomain),
        ),
        (
            "markup: use is restricted",
            markup("Use is restricted."),
            mismatch.clone(),
        ),
        (
            "markup: no restrictions",
            markup("No restrictions."),
            mismatch.clone(),
        ),
    ];
    let mut wrong = Vec::new();
    for (place, f, want) in &cases {
        let got = f.admit().map(|a| a.tier);
        if got != *want {
            wrong.push(format!("{place}: {got:?}, not {want:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} wrong: {wrong:#?}",
        wrong.len(),
        cases.len()
    );
}

#[test]
fn some_file_must_state_the_licence_outright() {
    // No statement anywhere.
    let mut f = Fixture::new();
    f.ly_header("");
    assert_eq!(f.refused(), Refusal::NoInFileLicence);
    // A markup alone is prose, not a statement of the licence.
    let mut f = Fixture::new();
    f.ly_header("  copyright = \\markup { \"public domain\" }\n");
    assert_eq!(f.refused(), Refusal::NoInFileLicence);
}

#[test]
fn an_unreadable_file_is_refused() {
    let mut f = Fixture::new();
    f.ly_header("  license = \"Public Domain\n");
    assert_eq!(
        f.refused(),
        Refusal::Unreadable {
            name: named(LY_NAME),
            why: Unreadable::UnterminatedString
        }
    );
    // The header declares two tracks and the file holds one: this reader's own count check.
    let mut f = Fixture::new();
    f.mid = smf_with(&[], 2);
    f.refresh();
    assert_eq!(
        f.refused(),
        Refusal::Unreadable {
            name: named(MID_NAME),
            why: Unreadable::TrackCountMismatch {
                declared: 2,
                found: 1
            }
        }
    );
    // The last track one byte short of its declared length: midly's strict chunk reader.
    let mut f = Fixture::new();
    f.mid.pop();
    f.refresh();
    assert_eq!(
        f.refused(),
        Refusal::Unreadable {
            name: named(MID_NAME),
            why: Unreadable::Smf("invalid chunk")
        }
    );
}

// ---------------------------------------------------------------------------------------
// Predicate version 3: CC0, the Public Domain Mark, own engravings under CC0, and the
// prohibition group's irregular forms.

/// CC0 1.0 admits as public domain: as the page licence, and in a file equal to it. The
/// in-file rule is unchanged, so every statement must still equal the page's licence.
#[test]
fn cc0_is_admitted_as_a_page_licence_and_in_file() {
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture, want: Result<Tier, Refusal>| {
        let got = f.admit().map(|a| a.tier);
        if got != want {
            wrong.push(format!("{place}: {got:?}, not {want:?}"));
        }
    };
    // The page licence, stated equal in the LilyPond file.
    for text in ["CC0 1.0", "CC0 1.0 Universal"] {
        let mut f = Fixture::new();
        f.page_licence(text);
        f.ly_header(&format!("  license = \"{text}\"\n"));
        check(text, f, Ok(Tier::PublicDomain));
    }
    // Stated by the MIDI file alone, in its copyright event.
    let mut f = Fixture::new();
    f.page_licence("CC0 1.0");
    f.ly_header("");
    f.mid = smf_with(&[(0x02, b"CC0 1.0")], 1);
    f.refresh();
    check("smf copyright event", f, Ok(Tier::PublicDomain));
    // Stated by the MIDI file alone, in a text event.
    let mut f = Fixture::new();
    f.page_licence("CC0 1.0");
    f.ly_header("");
    f.mid = smf_with(&[(0x01, b"CC0 1.0")], 1);
    f.refresh();
    check("smf text event", f, Ok(Tier::PublicDomain));
    // A markup that holds it and affirms it agrees, beside a statement of it.
    let mut f = Fixture::new();
    f.page_licence("CC0 1.0");
    f.ly_header(
        "  license = \"CC0 1.0\"\n  copyright = \\markup { \"Dedicated to the public domain under CC0 1.0\" }\n",
    );
    check("markup", f, Ok(Tier::PublicDomain));
    // Public Domain in a file beside a CC0 page, and CC0 in a file beside a Public Domain
    // page, are statements that differ from the page.
    let mut f = Fixture::new();
    f.page_licence("CC0 1.0");
    check(
        "public domain beside cc0",
        f,
        Err(Refusal::InFileLicenceMismatch {
            name: named(LY_NAME),
        }),
    );
    let mut f = Fixture::new();
    f.ly_header("  license = \"CC0 1.0\"\n");
    check(
        "cc0 beside public domain",
        f,
        Err(Refusal::InFileLicenceMismatch {
            name: named(LY_NAME),
        }),
    );
    // CC0 is public domain, so it carries no credit.
    let mut f = Fixture::new();
    f.page_licence("CC0 1.0");
    f.ly_header("  license = \"CC0 1.0\"\n");
    f.third().credit_ledger_id = Some(named("credit-0001"));
    check("credit", f, Err(Refusal::UnexpectedCreditLedgerId));
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The Public Domain Mark admits as public domain, by its sentence and by its name. A
/// sentence that only resembles the mark's is not it, and a rights statement that holds
/// a negating word still refuses (Known limits).
#[test]
fn the_public_domain_mark_is_admitted_as_public_domain() {
    const PDM: &str = "This work has been identified as being free of known restrictions \
                       under copyright law, including all related and neighboring rights.";
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture, want: Result<Tier, Refusal>| {
        let got = f.admit().map(|a| a.tier);
        if got != want {
            wrong.push(format!("{place}: {got:?}, not {want:?}"));
        }
    };
    // The mark's sentence as the page licence, stated equal in the LilyPond file.
    let mut f = Fixture::new();
    f.page_licence(PDM);
    f.ly_header(&format!("  license = \"{PDM}\"\n"));
    check("the sentence", f, Ok(Tier::PublicDomain));
    // The mark by its name, stated by the MIDI file's copyright event.
    let mut f = Fixture::new();
    f.page_licence("Public Domain Mark 1.0");
    f.ly_header("");
    f.mid = smf_with(&[(0x02, b"Public Domain Mark 1.0")], 1);
    f.refresh();
    check("the name", f, Ok(Tier::PublicDomain));
    // The short sentence is neither.
    let short = "This work is free of known copyright restrictions.";
    let mut f = Fixture::new();
    f.page_licence(short);
    f.ly_header(&format!("  license = \"{short}\"\n"));
    check(
        "the short sentence",
        f,
        Err(Refusal::Licence(LicenceRefusal::Unknown)),
    );
    // A rights statement that holds "no": its own quote negates it.
    let mut f = Fixture::new();
    f.page_licence("No known copyright restrictions");
    f.ly_header("  license = \"No known copyright restrictions\"\n");
    check(
        "no known copyright restrictions",
        f,
        Err(Refusal::QuoteNegated {
            what: "page licence",
        }),
    );
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The project's own engravings are under CC0 1.0. A file of one may state exactly that,
/// in any field that states a licence; any other statement is still refused, CC0 in
/// another form included.
#[test]
fn an_own_engraving_may_state_cc0_1_0_and_nothing_else() {
    fn own(f: &mut Fixture) {
        f.receipt.arrangement = Arrangement::ThisProject(ThisProject {
            engraver: named("si-jam-sessions"),
        });
    }
    let mut wrong = Vec::new();
    for header in [
        "  license = \"CC0 1.0\"\n",
        "  copyright = \"CC0 1.0\"\n",
        "  license = \"  cc0 \\t 1.0 \"\n",
        "  license = \"CC0 1.0\"\n  copyright = \"CC0 1.0\"\n",
        "  copyright = \\markup { \"CC0 1.0\" }\n",
    ] {
        let mut f = Fixture::new();
        f.ly_header(header);
        own(&mut f);
        let got = f.admit().map(|a| a.tier);
        if got != Ok(Tier::OwnEngraving) {
            wrong.push(format!("{header:?}: {got:?}"));
        }
    }
    for (place, metas) in [
        ("smf copyright event", &[(0x02, &b"CC0 1.0"[..])][..]),
        ("smf text event", &[(0x01, &b"CC0 1.0"[..])][..]),
    ] {
        let mut f = Fixture::new();
        f.ly_header("");
        f.mid = smf_with(metas, 1);
        f.refresh();
        own(&mut f);
        let got = f.admit().map(|a| a.tier);
        if got != Ok(Tier::OwnEngraving) {
            wrong.push(format!("{place}: {got:?}"));
        }
    }
    for header in [
        "  license = \"CC0 1.0 Universal\"\n",
        "  license = \"CC0\"\n",
        "  license = \"CC0-1.0\"\n",
        "  license = \"Creative Commons Attribution 4.0\"\n",
        "  license = \"CC0 1.0\"\n  copyright = \"Engraved 2026\"\n",
        "  copyright = \\markup { \"Dedicated to the public domain under CC0 1.0\" }\n",
    ] {
        let mut f = Fixture::new();
        f.ly_header(header);
        own(&mut f);
        let want = Err(Refusal::OwnEngravingStatesLicence {
            name: named(LY_NAME),
        });
        let got = f.admit().map(|a| a.tier);
        if got != want {
            wrong.push(format!("{header:?}: {got:?}, not {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// A licence text that holds one of the prohibition group's irregular forms does not
/// affirm its licence, in a copyright markup or in an evidence quote.
#[test]
fn a_licence_text_with_an_irregular_prohibiting_form_does_not_affirm() {
    let texts = [
        "The typesetter forbade resale.",
        "Resale was forbad by the typesetter.",
        "A forbiddance of resale is attached.",
        "Placed there on restrictive terms.",
        "Resale is restrictively licensed.",
        "This notice is prohibitory.",
        "Resale prices are prohibitive.",
        "Resale is prohibitively priced.",
        "A disallowance of resale is attached.",
    ];
    let mut wrong = Vec::new();
    for text in texts {
        let mut f = Fixture::new();
        f.ly_header(&format!(
            "  license = \"Public Domain\"\n  copyright = \\markup {{ \"Placed in the public domain. {text}\" }}\n"
        ));
        let want = Err(Refusal::InFileLicenceMismatch {
            name: named(LY_NAME),
        });
        let got = f.admit().map(|a| a.tier);
        if got != want {
            wrong.push(format!("markup {text:?}: {got:?}"));
        }
        let mut f = Fixture::new();
        f.evidence_mut("terms").quotes = vec![format!("Dedicated to the public domain. {text}")];
        let want = Err(Refusal::QuoteNegated { what: "terms" });
        let got = f.admit().map(|a| a.tier);
        if got != want {
            wrong.push(format!("terms quote {text:?}: {got:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} wrong: {wrong:#?}",
        wrong.len(),
        texts.len() * 2
    );
}

// ---------------------------------------------------------------------------------------
// Predicate version 3: anonymous works and anonymous editions.

/// An author the receipt records as unknown.
fn unknown(role: AuthorRole) -> Author {
    Author {
        name: None,
        role,
        death_year: None,
    }
}

/// The fixture with its composer unknown and the work and its edition first published in
/// `year`.
fn anonymous_published(year: u16) -> Fixture {
    let mut f = Fixture::new();
    f.receipt.composition.authors = vec![unknown(AuthorRole::Composer)];
    f.receipt.composition.first_publication_year = Some(year);
    f.receipt.source_edition.year = Some(year);
    f
}

/// An anonymous work is public domain by its first publication: at or before 1930 in the
/// US, at or before 1955 in the EU (Directive 2006/116/EC, Art. 1(3)).
#[test]
fn an_anonymous_work_is_admitted_by_its_first_publication() {
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture| {
        let got = f.admit().map(|a| a.tier);
        if got != Ok(Tier::PublicDomain) {
            wrong.push(format!("{place}: {got:?}"));
        }
    };
    check("1859", anonymous_published(1859));
    check("1930", anonymous_published(1930));
    // A named lyricist and an unknown composer: the words' author died, the tune's is
    // unknown.
    let mut f = anonymous_published(1862);
    f.receipt.composition.authors.insert(
        0,
        Author {
            name: Some(named("A. Lyricist")),
            role: AuthorRole::Lyricist,
            death_year: Some(1910),
        },
    );
    check("a named lyricist beside an unknown composer", f);
    // Every author unknown.
    let mut f = anonymous_published(1859);
    f.receipt
        .composition
        .authors
        .push(unknown(AuthorRole::Lyricist));
    check("every author unknown", f);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// First published in 1931, an anonymous work is refused by the US rule; in 1956, by the
/// EU rule for anonymous works, which is checked first. Both read the same year, and the
/// EU cut-off is the later one, so checked second it could never refuse anything.
#[test]
fn an_anonymous_work_is_refused_by_its_first_publication() {
    let cases = [
        (
            1931,
            Refusal::NotPublicDomainUs {
                first_publication_year: 1931,
            },
        ),
        // The EU cut-off is inclusive: 1955 passes it, and the US rule refuses.
        (
            1955,
            Refusal::NotPublicDomainUs {
                first_publication_year: 1955,
            },
        ),
        (
            1956,
            Refusal::AnonymousNotPublicDomainEu {
                role: AuthorRole::Composer,
                first_publication_year: 1956,
            },
        ),
    ];
    let mut wrong = Vec::new();
    for (year, want) in cases {
        let got = anonymous_published(year).refused();
        if got != want {
            wrong.push(format!("{year}: {got:?}, not {want:?}"));
        }
    }
    // An unknown lyricist beside a named composer who died in time: the refusal names the
    // unknown author's role.
    let mut f = Fixture::new();
    f.receipt
        .composition
        .authors
        .push(unknown(AuthorRole::Lyricist));
    f.receipt.composition.first_publication_year = Some(1956);
    f.receipt.source_edition.year = Some(1956);
    let want = Refusal::AnonymousNotPublicDomainEu {
        role: AuthorRole::Lyricist,
        first_publication_year: 1956,
    };
    let got = f.refused();
    if got != want {
        wrong.push(format!("unknown lyricist: {got:?}, not {want:?}"));
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The named-author path is unchanged. A named author still needs a death year, even
/// beside an unknown one; a work with no unknown author is refused at 1956 by the US rule,
/// as before; and a named author who died too late is refused beside an unknown one.
#[test]
fn the_named_author_path_is_unchanged() {
    let mut wrong = Vec::new();
    let mut check = |place: &str, f: Fixture, want: Refusal| {
        let got = f.refused();
        if got != want {
            wrong.push(format!("{place}: {got:?}, not {want:?}"));
        }
    };
    let mut f = Fixture::new();
    f.receipt.composition.authors = vec![
        unknown(AuthorRole::Composer),
        Author {
            name: Some(named("A. Lyricist")),
            role: AuthorRole::Lyricist,
            death_year: None,
        },
    ];
    check(
        "a named author without a death year",
        f,
        Refusal::MissingDeathYear {
            author: named("A. Lyricist"),
        },
    );
    let mut f = Fixture::new();
    f.receipt.composition.first_publication_year = Some(1956);
    f.receipt.source_edition.year = Some(1956);
    check(
        "no unknown author, first published 1956",
        f,
        Refusal::NotPublicDomainUs {
            first_publication_year: 1956,
        },
    );
    let mut f = Fixture::new();
    f.receipt.composition.authors[0].death_year = Some(1956);
    f.receipt
        .composition
        .authors
        .push(unknown(AuthorRole::Lyricist));
    check(
        "a named composer who died in 1956",
        f,
        Refusal::NotPublicDomainEu {
            author: named("A. Composer"),
            death_year: 1956,
        },
    );
    // An empty author list is still refused: an unknown author is recorded, not left out.
    let mut f = Fixture::new();
    f.receipt.composition.authors.clear();
    check("no authors", f, Refusal::NoAuthors);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// An unknown author has no death year: the receipt is refused as it loads, from JSON or
/// from a receipt built in code.
#[test]
fn an_unknown_author_has_no_death_year() {
    let mut f = Fixture::new();
    f.receipt.composition.authors[0].name = None;
    assert_eq!(
        f.refused(),
        Refusal::Receipt(ReceiptError::AnonymousAuthorDeathYear)
    );
    assert_eq!(
        json_error(|t| t.replacen("\"name\": \"Scott Joplin\"", "\"name\": null", 1)),
        ReceiptError::AnonymousAuthorDeathYear
    );
}

/// An author is named or recorded as unknown: a name that is empty, or only whitespace,
/// is refused as the receipt loads, from JSON or from a receipt built in code.
#[test]
fn an_author_name_is_never_empty() {
    for name in ["", " ", "\t\n"] {
        let mut f = Fixture::new();
        f.receipt.composition.authors[0].name = Some(name.into());
        assert_eq!(
            f.refused(),
            Refusal::Receipt(ReceiptError::EmptyAuthorName),
            "{name:?}"
        );
    }
    assert_eq!(
        json_error(|t| t.replacen("\"name\": \"Scott Joplin\"", "\"name\": \"\"", 1)),
        ReceiptError::EmptyAuthorName
    );
}

/// In JSON an unknown author's name is `null`, as every value the receipt does not know
/// is. Any other value that is not a string is refused.
#[test]
fn an_unknown_author_is_written_as_a_null_name() {
    let text = core::str::from_utf8(REAL_JSON).unwrap();
    let edited = text
        .replacen("\"name\": \"Scott Joplin\"", "\"name\": null", 1)
        .replacen("\"death_year\": 1917", "\"death_year\": null", 1);
    let receipt = Receipt::from_json(edited.as_bytes()).expect("loads");
    assert_eq!(
        receipt.composition.authors,
        vec![unknown(AuthorRole::Composer)]
    );
    // The Entertainer with its composer unknown is public domain by its publication in
    // 1902, and its receipt is a different receipt.
    let admitted = admit(&receipt, &real_supplied()).expect("admitted");
    assert_eq!(admitted.tier, Tier::PublicDomain);
    assert_ne!(hex(&admitted.receipt_digest), REAL_DIGEST);
    assert_eq!(
        json_error(|t| t.replacen("\"name\": \"Scott Joplin\"", "\"name\": 1917", 1)),
        ReceiptError::BadValue {
            object: "authors[]",
            key: "name",
        }
    );
}

/// An anonymous source edition's own term runs from its publication: at or before 1930
/// in the US, at or before 1955 in the EU, the EU rule checked first. The other kinds of
/// edition are unchanged: their date alone shows them out of term through 2000.
#[test]
fn an_anonymous_edition_is_public_domain_by_its_publication() {
    let edition = |kind: EditionKind, year: u16| {
        let mut f = Fixture::new();
        f.receipt.source_edition.kind = kind;
        f.receipt.source_edition.year = Some(year);
        f.admit().map(|a| a.tier)
    };
    let cases = [
        (1899, Ok(Tier::PublicDomain)),
        (1930, Ok(Tier::PublicDomain)),
        (
            1931,
            Err(Refusal::AnonymousEditionNotPublicDomainUs { edition_year: 1931 }),
        ),
        (
            1955,
            Err(Refusal::AnonymousEditionNotPublicDomainUs { edition_year: 1955 }),
        ),
        (
            1956,
            Err(Refusal::AnonymousEditionNotPublicDomainEu { edition_year: 1956 }),
        ),
    ];
    let mut wrong = Vec::new();
    for (year, want) in cases {
        let got = edition(EditionKind::Anonymous, year);
        if got != want {
            wrong.push(format!("anonymous {year}: {got:?}, not {want:?}"));
        }
    }
    for kind in [
        EditionKind::FirstEdition,
        EditionKind::Reprint,
        EditionKind::Scholarly,
        EditionKind::Unknown,
    ] {
        let got = edition(kind, 1956);
        if got != Ok(Tier::PublicDomain) {
            wrong.push(format!("{kind:?} 1956: {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
    // In JSON the kind is `anonymous`.
    let receipt = Receipt::from_json(
        core::str::from_utf8(REAL_JSON)
            .unwrap()
            .replacen("\"kind\": \"first-edition\"", "\"kind\": \"anonymous\"", 1)
            .as_bytes(),
    )
    .expect("loads");
    assert_eq!(receipt.source_edition.kind, EditionKind::Anonymous);
}

/// An unknown author and an anonymous edition round-trip through the canonical encoding,
/// which gives them values no version 2 receipt holds: the author's death tag 2 after an
/// empty name, and edition kind 4. A named author encodes as before (the Entertainer's
/// digest is pinned above).
#[test]
fn an_unknown_author_and_an_anonymous_edition_round_trip() {
    let mut f = Fixture::new();
    f.receipt.composition.authors = vec![
        unknown(AuthorRole::Composer),
        Author {
            name: Some(named("A. Lyricist")),
            role: AuthorRole::Lyricist,
            death_year: Some(1910),
        },
        unknown(AuthorRole::Lyricist),
    ];
    f.receipt.source_edition.kind = EditionKind::Anonymous;
    let bytes = f.receipt.to_canonical();
    let back = Receipt::from_canonical(&bytes).expect("decodes");
    assert_eq!(back, f.receipt);
    assert_eq!(back.to_canonical(), bytes);

    // The first author's record starts after the magic, the version, the schema, the
    // score id, the title, the date and the author count: an empty name, the composer's
    // tag, then death tag 2.
    let at = 4 + 2 + 2 + (4 + 5) + (4 + 5) + 4 + 4;
    assert_eq!(&bytes[at..at + 6], &[0, 0, 0, 0, 0, 2]);
    // An unknown author with a name is refused, and so is a death tag past 2.
    let mut named_unknown = f.receipt.clone();
    named_unknown.composition.authors[0] = Author {
        name: Some(named("X")),
        role: AuthorRole::Composer,
        death_year: None,
    };
    let mut b = named_unknown.to_canonical();
    // `X` sits at at + 4, its role at at + 5 and its death tag at at + 6.
    assert_eq!(b[at + 6], 0);
    b[at + 6] = 2;
    assert_eq!(
        Receipt::from_canonical(&b),
        Err(ReceiptError::Canonical {
            offset: at,
            problem: CanonicalProblem::UnknownAuthorNamed
        })
    );
    let mut b = bytes.clone();
    b[at + 5] = 3;
    assert_eq!(
        Receipt::from_canonical(&b),
        Err(ReceiptError::Canonical {
            offset: at + 5,
            problem: CanonicalProblem::BadFlag
        })
    );
}

// ---------------------------------------------------------------------------------------
// The Battle Hymn fixture: a receipt for this project's own CC0 engraving of Battle Hymn
// of the Republic, built from `research/battle-hymn/evidence-manifest.json`. Its two files
// are placeholders; the real score comes later.

const BH_JSON: &[u8] = include_bytes!("../fixtures/battle-hymn/receipt.json");
const BH_LY: &[u8] = include_bytes!("../fixtures/battle-hymn/battle-hymn.ly");
const BH_MID: &[u8] = include_bytes!("../fixtures/battle-hymn/battle-hymn.mid");
const BH_MANIFEST: &[u8] = include_bytes!("../../../research/battle-hymn/evidence-manifest.json");

fn battle_hymn() -> Receipt {
    Receipt::from_json(BH_JSON).expect("the Battle Hymn fixture loads")
}

fn battle_hymn_supplied() -> [Supplied<'static>; 2] {
    [
        Supplied {
            name: "battle-hymn.ly",
            bytes: BH_LY,
        },
        Supplied {
            name: "battle-hymn.mid",
            bytes: BH_MID,
        },
    ]
}

/// The fixture is admitted as this project's own engraving, and it records what it must:
/// the words by Julia Ward Howe, who died in 1910, first published in 1862; the tune's
/// composer as unknown, with the tune's 1859 printing and its claimants in the notes; the
/// anonymous Ditson edition of 1862; and the engraving's own CC0 1.0 statement.
#[test]
fn the_battle_hymn_fixture_is_admitted_as_an_own_engraving() {
    let r = battle_hymn();
    let admitted = admit(&r, &battle_hymn_supplied()).expect("admitted");
    assert_eq!(admitted.tier, Tier::OwnEngraving);
    assert_eq!(admitted.receipt_digest, r.digest());

    assert_eq!(
        r.composition.authors,
        vec![
            Author {
                name: Some(named("Julia Ward Howe")),
                role: AuthorRole::Lyricist,
                death_year: Some(1910),
            },
            unknown(AuthorRole::Composer),
        ]
    );
    assert_eq!(r.composition.first_publication_year, Some(1862));
    assert_eq!(
        r.source_edition.publisher.as_deref(),
        Some("Oliver Ditson & Co.")
    );
    assert_eq!(r.source_edition.year, Some(1862));
    assert_eq!(r.source_edition.kind, EditionKind::Anonymous);
    assert_eq!(
        r.arrangement,
        Arrangement::ThisProject(ThisProject {
            engraver: named("si-jam-sessions"),
        })
    );
    assert_eq!(
        r.files[0].in_file_licence,
        vec![Statement {
            field: StatementField::LilypondCopyright,
            text: named(OWN_ENGRAVING_LICENCE),
        }]
    );
    assert_eq!(r.files[1].in_file_licence, vec![]);
    // The tune's own year and its claimants are notes; a claimant is never an author.
    let notes = r.notes.join("\n");
    assert!(notes.contains("The tune was in print by 1859"), "{notes}");
    assert!(
        notes.contains("William Steffe is the most-cited"),
        "{notes}"
    );
    for a in &r.composition.authors {
        assert!(
            !a.name.as_deref().unwrap_or_default().contains("Steffe"),
            "{a:?}"
        );
    }
    // The canonical encoding round-trips, and the receipt carries no local paths.
    let bytes = r.to_canonical();
    assert_eq!(Receipt::from_canonical(&bytes).as_ref(), Ok(&r));
    let text = core::str::from_utf8(BH_JSON).unwrap();
    for needle in [":\\", "/home/", "\\Users\\", "/Users/", "file:"] {
        assert!(!text.contains(needle), "{needle}");
    }
    assert_eq!(text.matches(":/").count(), text.matches("://").count());
}

/// Reads a JSON object's member, for the manifest check below.
fn member<'a>(value: &'a crate::json::Value, key: &str) -> &'a crate::json::Value {
    match value {
        crate::json::Value::Obj(members) => {
            &members
                .iter()
                .find(|(k, _)| k == key)
                .unwrap_or_else(|| panic!("no {key}"))
                .1
        }
        _ => panic!("not an object at {key}"),
    }
}

fn items(value: &crate::json::Value) -> &[crate::json::Value] {
    match value {
        crate::json::Value::Arr(items) => items,
        _ => panic!("not an array"),
    }
}

fn text(value: &crate::json::Value) -> Option<&str> {
    match value {
        crate::json::Value::Str(s) => Some(s),
        crate::json::Value::Null => None,
        _ => panic!("not a string or null"),
    }
}

/// The evidence manifest's files that the Battle Hymn fixture leaves off. The fixture
/// carries, for each claim it rests on, the files that hold the quoted words. A file joins
/// this list only by review.
const LEFT_OFF_THE_FIXTURE: &[&str] = &[
    // Other pages of documents the fixture cites, which hold none of its quotes.
    "ia-atlantic-1862-02-leaf1-p146.jpg",
    "ia-atlantic-1862-02-leaf3-p148.jpg",
    "loc-ditson-1862-muscivilwar-200000858-003.jp2",
    "loc-ditson-1862-muscivilwar-200000858-004.jp2",
    "ia-prayer-meeting-tune-book-1859-title-n6.jpg",
    "ia-prayer-meeting-tune-book-1859-verso-n7.jpg",
    "loc-glory-glory-1861-ditson-002.jp2",
    // The archives' catalogue records of documents the fixture cites.
    "ia-prayer-meeting-tune-book-1859-metadata.json",
    "ia-grand-lodge-pa-1911-metadata.json",
    // Further sources for claims the cited files already carry: the tune in print by 1859,
    // its disputed authorship, and William Steffe's dates.
    "ia-sunday-school-times-1859-01-15-djvu.txt",
    "ia-sunday-school-times-1859-01-15-metadata.json",
    "loc-ditson-1890-reissue-item-2023871344.json",
    "loc-ditson-1890-reissue-p1-50pct.jpg",
    "ia-army-navy-journal-1885-03-21-djvu.txt",
    "ia-army-navy-journal-1885-03-21-metadata.json",
    "ia-elson-national-music-of-america-1900-djvu.txt",
    "ia-elson-national-music-of-america-1900-metadata.json",
    "ia-hymn-society-papers-djvu.txt",
    "ia-hymn-society-papers-metadata.json",
    "ia-grand-lodge-pa-1912-djvu.txt",
    "ia-grand-lodge-pa-1912-metadata.json",
    "ia-masonic-temple-dedication-1875-djvu.txt",
    "ia-masonic-temple-dedication-1875-metadata.json",
];

/// Every evidence entry on the fixture is one file of the committed evidence manifest:
/// its URL, where it resolved, its size and its SHA-256 are the manifest's, it was fetched
/// on the receipt's day, and its quotes are that file's manifest quotes, in order. Every
/// entry is cited, by the composition, the edition or a note. And every manifest file is on
/// the receipt or in `LEFT_OFF_THE_FIXTURE`, so nothing the research found is dropped
/// unseen.
#[test]
fn the_battle_hymn_fixture_is_built_from_the_evidence_manifest() {
    use crate::json::Value;
    let manifest = crate::json::parse(BH_MANIFEST).expect("the manifest parses");
    let r = battle_hymn();
    let day = format!(
        "{:04}-{:02}-{:02}T",
        r.fetched_on.year, r.fetched_on.month, r.fetched_on.day
    );
    let mut found = 0;
    let mut left_off = Vec::new();
    for source in items(member(&manifest, "evidence")) {
        for file in items(member(source, "files")) {
            let path = text(member(file, "file")).unwrap();
            let name = path.strip_prefix("evidence/").unwrap();
            let Some(e) = r.evidence(name) else {
                left_off.push(name);
                continue;
            };
            found += 1;
            assert_eq!(Some(e.url.as_str()), text(member(file, "url")), "{name}");
            assert_eq!(
                e.resolved_url.as_deref(),
                text(member(file, "resolved_url")),
                "{name}"
            );
            assert_eq!(
                Some(hex(&e.sha256).as_str()),
                text(member(file, "sha256")),
                "{name}"
            );
            assert_eq!(&Value::Uint(e.bytes), member(file, "bytes"), "{name}");
            assert!(
                text(member(file, "fetched_at")).unwrap().starts_with(&day),
                "{name}"
            );
            let quotes: Vec<&str> = items(member(source, "quotes"))
                .iter()
                .filter(|q| text(member(q, "file")) == Some(path))
                .map(|q| text(member(q, "text")).unwrap())
                .collect();
            assert_eq!(e.quotes, quotes, "{name}");
        }
    }
    assert_eq!(found, r.evidence.len(), "an entry is not a manifest file");
    let mut listed = LEFT_OFF_THE_FIXTURE.to_vec();
    listed.sort_unstable();
    left_off.sort_unstable();
    assert_eq!(
        left_off, listed,
        "a manifest file is neither on the receipt nor listed as left off"
    );
    for e in &r.evidence {
        let cited = r.composition.evidence.contains(&e.id)
            || r.source_edition.evidence.contains(&e.id)
            || r.notes.iter().any(|n| n.contains(e.id.as_str()));
        assert!(cited, "{} is cited nowhere", e.id);
    }
}

// ---------------------------------------------------------------------------------------
// The canonical encoding.

#[test]
fn canonical_round_trips_for_every_shape() {
    let mut f = Fixture::new();
    f.third().terms.restrictions = vec![Restriction::ShareAlike, Restriction::AiRestricted];
    f.third().credit_ledger_id = Some(named("credit-0001"));
    f.third().contributors = vec![named("One"), named("Two")];
    f.receipt.files[0].last_modified = Some(named("Fri, 25 Nov 2016 09:28:17 GMT"));
    f.receipt.evidence[0].resolved_url = Some(named("https://example.org/moved"));
    f.receipt.composition.authors[0].death_year = None;
    f.receipt.composition.first_publication_year = None;
    f.receipt.source_edition.publisher = None;
    f.receipt.source_edition.year = None;
    f.receipt.notes = vec![named("ü ‘note’ \u{1d11e}"), String::new()];
    let own = Receipt {
        arrangement: Arrangement::ThisProject(ThisProject {
            engraver: named("si-jam-sessions"),
        }),
        ..f.receipt.clone()
    };
    for receipt in [f.receipt.clone(), own, real()] {
        let bytes = receipt.to_canonical();
        let back = Receipt::from_canonical(&bytes).unwrap();
        assert_eq!(back, receipt);
        assert_eq!(back.to_canonical(), bytes);
    }
}

fn canonical_problem(bytes: &[u8]) -> Option<CanonicalProblem> {
    match Receipt::from_canonical(bytes) {
        Err(ReceiptError::Canonical { problem, .. }) => Some(problem),
        _ => None,
    }
}

#[test]
fn canonical_decoding_is_strict() {
    let good = Fixture::new().receipt.to_canonical();

    // Every proper prefix is refused.
    for n in 0..good.len() {
        assert!(Receipt::from_canonical(&good[..n]).is_err(), "prefix {n}");
    }

    let mut b = good.clone();
    b.push(0);
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::TrailingBytes));

    let mut b = good.clone();
    b[0] = b'X';
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::BadMagic));

    let mut b = good.clone();
    b[4] = 2;
    assert_eq!(
        canonical_problem(&b),
        Some(CanonicalProblem::UnsupportedVersion)
    );

    // Bytes 6..8 are the schema, which the structure check owns.
    let mut b = good.clone();
    b[6] = 9;
    assert_eq!(
        Receipt::from_canonical(&b),
        Err(ReceiptError::UnsupportedSchema(9))
    );

    // Bytes 8..12 are the length of `score_id` and 12.. its text, "piece".
    let mut b = good.clone();
    b[12] = 0xFF;
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::NotUtf8));

    // A length larger than the bytes left is refused.
    let mut b = good.clone();
    b[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::UnexpectedEnd));
}

/// The offset of the first byte two encodings disagree on.
fn first_difference(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).position(|(x, y)| x != y).unwrap()
}

#[test]
fn canonical_tags_and_flags_outside_the_vocabulary_are_refused() {
    let f = Fixture::new();
    let good = f.receipt.to_canonical();

    let mut other = f.receipt.clone();
    other.source_edition.kind = EditionKind::Unknown;
    let at = first_difference(&good, &other.to_canonical());
    let mut b = good.clone();
    b[at] = 9;
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::BadTag));

    let mut other = f.receipt.clone();
    other.files[0].last_modified = Some(named("x"));
    let at = first_difference(&good, &other.to_canonical());
    let mut b = good.clone();
    b[at] = 2;
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::BadFlag));

    let mut other = f.receipt.clone();
    other.arrangement = Arrangement::ThisProject(ThisProject {
        engraver: named("x"),
    });
    let at = first_difference(&good, &other.to_canonical());
    let mut b = good.clone();
    b[at] = 7;
    assert_eq!(canonical_problem(&b), Some(CanonicalProblem::BadTag));
}

#[test]
fn canonical_decoding_enforces_structure() {
    let mut f = Fixture::new();
    f.receipt.files.swap(0, 1);
    let bytes = f.receipt.to_canonical();
    assert_eq!(
        Receipt::from_canonical(&bytes),
        Err(ReceiptError::FilesNotSorted)
    );
}

// ---------------------------------------------------------------------------------------
// The JSON schema.

fn json_error(edit: impl Fn(&str) -> String) -> ReceiptError {
    let text = core::str::from_utf8(REAL_JSON).unwrap();
    let edited = edit(text);
    assert_ne!(edited, text, "the edit changed nothing");
    Receipt::from_json(edited.as_bytes()).expect_err("refused")
}

/// An edit to the receipt's JSON text.
type JsonEdit = fn(&str) -> String;

#[test]
fn the_json_schema_is_closed() {
    let cases: [(JsonEdit, ReceiptError); 15] = [
        (
            |t| t.replacen("\"schema\": 1,", "\"schema\": 1, \"extra\": 0,", 1),
            ReceiptError::UnknownKey {
                object: "receipt",
                key: named("extra"),
            },
        ),
        (
            |t| t.replacen("  \"title\": \"The Entertainer\",\n", "", 1),
            ReceiptError::MissingKey {
                object: "receipt",
                key: "title",
            },
        ),
        (
            |t| t.replacen("\"death_year\": 1917", "\"death_year\": \"1917\"", 1),
            ReceiptError::BadValue {
                object: "authors[]",
                key: "death_year",
            },
        ),
        (
            |t| t.replacen("\"role\": \"composer\"", "\"role\": \"arranger\"", 1),
            ReceiptError::BadValue {
                object: "authors[]",
                key: "role",
            },
        ),
        (
            |t| t.replacen("\"kind\": \"third-party\"", "\"kind\": \"other\"", 1),
            ReceiptError::BadValue {
                object: "arrangement",
                key: "kind",
            },
        ),
        (
            |t| t.replacen("\"year\": 1902", "\"year\": 70000", 1),
            ReceiptError::BadValue {
                object: "source_edition",
                key: "year",
            },
        ),
        (
            |t| t.replacen("\"schema\": 1", "\"schema\": 2", 1),
            ReceiptError::UnsupportedSchema(2),
        ),
        (
            |t| t.replacen("2026-09-25", "2026-02-30", 1),
            ReceiptError::InvalidDate,
        ),
        (
            |t| t.replacen("3a07fcf4", "3A07FCF4", 1),
            ReceiptError::BadValue {
                object: "files[]",
                key: "sha256",
            },
        ),
        (
            |t| {
                t.replacen(
                    "\"name\": \"entertainer.ly\"",
                    "\"name\": \"../entertainer.ly\"",
                    1,
                )
            },
            ReceiptError::BadName,
        ),
        (
            |t| t.replacen("\"name\": \"entertainer.ly\"", "\"name\": \"zz.ly\"", 1),
            ReceiptError::FilesNotSorted,
        ),
        (
            |t| t.replacen("\"id\": \"loc-item\"", "\"id\": \"zz\"", 1),
            ReceiptError::EvidenceNotSorted,
        ),
        (
            |t| {
                t.replacen(
                    "\"restrictions\": []",
                    "\"restrictions\": [\"share-alike\", \"all-rights-reserved\"]",
                    1,
                )
            },
            ReceiptError::RestrictionsNotSorted,
        ),
        (
            |t| {
                t.replacen(
                    "\"restrictions\": []",
                    "\"restrictions\": [\"share-alike\", \"share-alike\"]",
                    1,
                )
            },
            ReceiptError::RestrictionsNotSorted,
        ),
        (
            |t| {
                t.replacen(
                    "\"field\": \"lilypond-license\"",
                    "\"field\": \"license\"",
                    1,
                )
            },
            ReceiptError::BadValue {
                object: "in_file_licence[]",
                key: "field",
            },
        ),
    ];
    for (edit, expected) in cases {
        assert_eq!(json_error(edit), expected);
    }
    assert!(matches!(
        json_error(|t| t.replacen("\"bytes\": 11100", "\"bytes\": 11100.0", 1)),
        ReceiptError::Json {
            problem: JsonProblem::NotAnInteger,
            ..
        }
    ));
    assert!(matches!(
        json_error(|t| t.replacen(
            "\"title\": \"The Entertainer\",",
            "\"title\": \"The Entertainer\", \"title\": \"x\",",
            1
        )),
        ReceiptError::Json {
            problem: JsonProblem::DuplicateKey,
            ..
        }
    ));
}

// The date rules belong to the law version: changing them must be a deliberate edit here.

#[test]
fn the_rules_are_those_of_2026() {
    assert_eq!(RULES_YEAR, 2026);
    assert_eq!(US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR, 1930);
    assert_eq!(EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR, 1955);
    assert_eq!(EU_LAST_PUBLIC_DOMAIN_ANONYMOUS_PUBLICATION_YEAR, 1955);
    assert_eq!(LAST_OUT_OF_TERM_EDITION_YEAR, 2000);
    assert_eq!(PREDICATE_VERSION, 3);
    assert_eq!(CANONICAL_VERSION, 1);
    assert_eq!(RECEIPT_SCHEMA, 1);
}

// ---------------------------------------------------------------------------------------
// The SMF reader's `read_tracks` is `Smf::parse` without its floating-point capacity
// guess. (Adapted from a reference patch made for PR #4, the golden hash; here the two
// count checks refuse by their own names.)

/// What `Smf::parse` would have said for a refusal of `read_tracks`.
fn as_smf_parse_says(ours: Unreadable) -> Unreadable {
    match ours {
        Unreadable::TrackCountMismatch { .. } => {
            Unreadable::Smf("file has a different amount of tracks than declared")
        }
        Unreadable::SingleTrackFormat { .. } => {
            Unreadable::Smf("singletrack format file has multiple tracks")
        }
        other => other,
    }
}

/// Parses `bytes` both ways and requires the same header and events, or the same refusal.
fn reads_as_smf_parse_does(bytes: &[u8]) {
    match (crate::infile::read_tracks(bytes), midly::Smf::parse(bytes)) {
        (Ok((header, tracks)), Ok(smf)) => {
            assert_eq!(header, smf.header);
            assert_eq!(tracks, smf.tracks);
        }
        (Err(ours), Err(theirs)) => assert_eq!(
            as_smf_parse_says(ours),
            Unreadable::Smf(theirs.kind().message())
        ),
        (ours, theirs) => panic!(
            "read_tracks gave {:?} and Smf::parse gave {:?}",
            ours.map(|_| ()),
            theirs.map(|_| ())
        ),
    }
}

#[test]
fn read_tracks_reads_what_smf_parse_reads() {
    reads_as_smf_parse_does(REAL_MID);
    // Truncations, which end inside every kind of event and chunk.
    for len in (0..REAL_MID.len()).step_by(7) {
        reads_as_smf_parse_does(&REAL_MID[..len]);
    }
    // Byte changes in the format and the declared track count, and throughout the tracks.
    // The division (bytes 12 and 13) is left alone: midly panics on a division of 0x80xx,
    // which this reader refuses before either parser runs.
    for at in (8..12).chain((14..REAL_MID.len()).step_by(23)) {
        for flip in [0x01, 0x40, 0x80, 0xFF] {
            let mut changed = REAL_MID.to_vec();
            changed[at] ^= flip;
            reads_as_smf_parse_does(&changed);
        }
    }
}

#[test]
fn read_tracks_repeats_smf_parses_two_track_count_checks() {
    let one_declared_two = smf_with(&[], 2);
    assert_eq!(
        crate::infile::read_tracks(&one_declared_two).map(|_| ()),
        Err(Unreadable::TrackCountMismatch {
            declared: 2,
            found: 1
        })
    );
    reads_as_smf_parse_does(&one_declared_two);
    // Format 0 declaring and holding two tracks.
    let mut two = smf_with(&[], 2);
    let track = two[14..].to_vec();
    two.extend_from_slice(&track);
    assert_eq!(
        crate::infile::read_tracks(&two).map(|_| ()),
        Err(Unreadable::SingleTrackFormat { tracks: 2 })
    );
    reads_as_smf_parse_does(&two);
    reads_as_smf_parse_does(&smf_with(&[(0x02, b"(c) 1902")], 1));
}
