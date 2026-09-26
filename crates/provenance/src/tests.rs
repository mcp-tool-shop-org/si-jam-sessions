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
                    name: named("A. Composer"),
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
        name: named("A. Lyricist"),
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
    assert_eq!(LAST_OUT_OF_TERM_EDITION_YEAR, 2000);
    assert_eq!(PREDICATE_VERSION, 2);
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
