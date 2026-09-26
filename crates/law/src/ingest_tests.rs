//! The ingest verb, on the committed Entertainer and on copies of it broken at
//! one layer each.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use ingest::IngestError;
use provenance::{AuthorRole, LicenceRefusal, ReceiptError, Tier, Unreadable};
use sha2::{Digest, Sha256};

use crate::refusal::{Event, IngestRefusal, Refusal, WireFault};
use crate::score::{LawMeter, LawTempo};
use crate::wire::{self, ContainerFile};
use crate::{Law, Provenance, ScoreNoteId, TakeNote};

pub(crate) const RECEIPT: &[u8] = include_bytes!("../../../scores/entertainer/receipt.json");
pub(crate) const LY: &[u8] = include_bytes!("../../../scores/entertainer/entertainer.ly");
pub(crate) const MID: &[u8] = include_bytes!("../../../scores/entertainer/entertainer.mid");

/// SHA-256 of the committed receipt's canonical encoding. The provenance
/// crate pins the same value, cross-checked there by an independent encoder.
const RECEIPT_DIGEST: &str = "e23ba2e9a9c16704564485089f31899c316dd7b398ba10bd7628bef5605f282c";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn container(receipt: &[u8], files: &[(&str, &[u8])]) -> Vec<u8> {
    let files: Vec<ContainerFile<'_>> = files
        .iter()
        .map(|&(name, bytes)| ContainerFile { name, bytes })
        .collect();
    wire::encode_container(receipt, &files).unwrap()
}

/// The committed receipt and both files it receipts.
pub(crate) fn entertainer() -> Vec<u8> {
    container(RECEIPT, &[("entertainer.ly", LY), ("entertainer.mid", MID)])
}

fn refusal(container: &[u8]) -> IngestRefusal {
    Law::ingest(container).unwrap_err()
}

fn receipt_text() -> &'static str {
    core::str::from_utf8(RECEIPT).unwrap()
}

/// The committed receipt with the `.mid` entry's SHA-256 and size replaced by
/// those of `mid`, so the predicate checks `mid` in its place.
fn receipt_for_mid(mid: &[u8]) -> Vec<u8> {
    let digest: [u8; 32] = Sha256::digest(mid).into();
    let text = receipt_text()
        .replacen(
            "33e4e81ee64ffb2edf90d1c6a1ddee7276507296bfb915c2bb775231a467f066",
            &hex(&digest),
            1,
        )
        .replacen("\"bytes\": 22084", &format!("\"bytes\": {}", mid.len()), 1);
    assert_ne!(text.as_bytes(), RECEIPT, "the replacement took place");
    text.into_bytes()
}

/// A one-track SMF, format 0, with these track events after the header.
fn smf(ppq: u16, events: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&ppq.to_be_bytes());
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(events.len() as u32).to_be_bytes());
    out.extend_from_slice(events);
    out
}

/// The receipt's `.mid` object as it is committed, from its opening brace to
/// its closing one.
fn mid_entry() -> (usize, usize) {
    let text = receipt_text();
    let start = text
        .find("    {\n      \"name\": \"entertainer.mid\"")
        .unwrap();
    let end = start + text[start..].find("\n    }").unwrap() + "\n    }".len();
    (start, end)
}

// --- The committed score --------------------------------------------------

#[test]
fn the_entertainer_is_ingested_through_the_licence_predicate() {
    let law = Law::ingest(&entertainer()).unwrap();
    let provenance = law.provenance().unwrap();
    assert_eq!(provenance.tier, Tier::PublicDomain);
    assert_eq!(hex(&provenance.receipt_digest), RECEIPT_DIGEST);
    assert_eq!(law.steps(), 0, "the transport is stopped");
    assert!(law.take().is_empty());

    let score = law.score();
    assert_eq!(score.notes().len(), 2621);
    assert_eq!(
        score.tempo(),
        [LawTempo {
            tick: 0,
            us_per_quarter: 833_333,
            start_sample: 0
        }]
    );
    assert_eq!(
        score.meter(),
        [LawMeter {
            tick: 0,
            numerator: 2,
            denominator_pow2: 2
        }]
    );
    // PPQ 384 to 3360 multiplies by 35/4, so a sixteenth (96 source ticks) is
    // 840 law ticks. The opening sixteenths, D, E and C in octaves:
    let opening: Vec<(u64, u8, u64)> = score.notes()[..6]
        .iter()
        .map(|n| (n.start_tick, n.pitch, n.end_tick))
        .collect();
    assert_eq!(
        opening,
        [
            (0, 74, 840),
            (0, 86, 840),
            (840, 76, 1_680),
            (840, 88, 1_680),
            (1_680, 72, 2_520),
            (1_680, 84, 2_520)
        ]
    );
    // The last note, source ticks 116,352 to 116,544.
    let last = score.notes().last().unwrap();
    assert_eq!(
        (last.start_tick, last.pitch, last.end_tick),
        (1_018_080, 84, 1_019_760)
    );
    // One tempo, so every onset is floor(tick x 833,333 x 48,000 / 3.36e9).
    for n in score.notes() {
        let exact = u128::from(n.start_tick) * 833_333 * 48_000 / 3_360_000_000;
        assert_eq!(u128::from(n.onset_sample), exact, "{n:?}");
    }
    assert_eq!(score.notes()[2].onset_sample, 9_999, "840 ticks: 9,999.996");
}

#[test]
fn the_verb_is_the_predicate_then_the_reader_then_the_laws_load() {
    let via_verb = Law::ingest(&entertainer()).unwrap();
    let via_load = Law::load(&ingest::ingest_smf(MID).unwrap()).unwrap();
    assert_eq!(via_verb.score(), via_load.score());
    assert_eq!(via_load.provenance(), None);
    // The same score with and without a receipt: two different records.
    assert_ne!(via_verb.hash().unwrap(), via_load.hash().unwrap());
}

#[test]
fn the_snapshot_commits_the_tier_and_the_receipt_digest() {
    let law = Law::ingest(&entertainer()).unwrap();
    let bytes = law.snapshot_bytes().unwrap();
    assert_eq!(&bytes[56..60], b"PROV");
    assert_eq!(&bytes[60..64], &[1, 0, 0, 0], "one record");
    assert_eq!(bytes[64], 1, "tier 1, public domain");
    assert_eq!(hex(&bytes[65..97]), RECEIPT_DIGEST);
    assert_eq!(&bytes[97..101], &[0, 0, 0, 0], "no credit-ledger id");
    assert_eq!(&bytes[101..105], b"TMPO");
}

#[test]
fn a_change_to_the_receipt_moves_the_hash() {
    // A human note on the receipt: the predicate does not read it, so the
    // score is still admitted, but the receipt, and so the record, changed.
    let edited = receipt_text()
        .replacen(
            "Mutopia holds one typesetting",
            "Mutopia holds one typesetting ",
            1,
        )
        .into_bytes();
    assert_ne!(edited.as_slice(), RECEIPT);
    let a = Law::ingest(&entertainer()).unwrap();
    let b = Law::ingest(&container(
        &edited,
        &[("entertainer.ly", LY), ("entertainer.mid", MID)],
    ))
    .unwrap();
    assert_eq!(a.score(), b.score());
    assert_eq!(b.provenance().map(|p| &p.tier), Some(&Tier::PublicDomain));
    assert_ne!(a.provenance(), b.provenance());
    assert_ne!(a.hash().unwrap(), b.hash().unwrap());
}

#[test]
fn an_ingested_score_takes_a_take_and_grades_it() {
    let mut law = Law::ingest(&entertainer()).unwrap();
    let first = law.score().notes()[0];
    let take = [TakeNote {
        onset_sample: first.onset_sample + 2_160,
        pitch: first.pitch,
        velocity: 90,
        cites: Some(ScoreNoteId(0)),
    }];
    law.admit(&take).unwrap();
    let rows = law.rows().unwrap();
    assert_eq!(rows.len(), 2621);
    assert_eq!(
        rows[0],
        "note 0: onset +2160 samples (+45.0 ms) vs gate \u{b1}1920, pitch 74 vs 74: late"
    );
    assert_eq!(
        rows[1],
        "note 1: onset 0 samples, pitch 86, no take note cites it: never played"
    );
    // Stepping does not touch the record of an ingested score either, and
    // the provenance survives admitting and stepping.
    let before = law.hash().unwrap();
    for _ in 0..500 {
        law.step().unwrap();
    }
    assert_eq!(law.hash().unwrap(), before);
    let provenance = law.provenance().unwrap();
    assert_eq!(
        (provenance.tier.clone(), hex(&provenance.receipt_digest)),
        (Tier::PublicDomain, String::from(RECEIPT_DIGEST))
    );
    let expected = Provenance {
        tier: Tier::PublicDomain,
        receipt_digest: provenance.receipt_digest,
    };
    assert_eq!(law.provenance(), Some(&expected));
}

// --- Each layer refuses by name ------------------------------------------

#[test]
fn the_container_refuses_first() {
    let mut bad = entertainer();
    bad[0] = b'X';
    let r = refusal(&bad);
    assert_eq!(
        r,
        IngestRefusal::Law(Refusal::Wire {
            fault: WireFault::Magic,
            offset: 0
        })
    );
    assert_eq!(r.code(), 50);
    assert_eq!(
        format!("{r}"),
        "bytes refused at offset 0: the magic is wrong"
    );
}

#[test]
fn a_receipt_that_does_not_load_is_refused() {
    let broken = &RECEIPT[..RECEIPT.len() - 3];
    let r = refusal(&container(
        broken,
        &[("entertainer.ly", LY), ("entertainer.mid", MID)],
    ));
    assert!(
        matches!(
            r,
            IngestRefusal::Licence(provenance::Refusal::Receipt(ReceiptError::Json { .. }))
        ),
        "{r:?}"
    );
    assert_eq!(r.code(), 100);
    assert!(
        format!("{r}").starts_with(
            "score refused by the licence predicate: the receipt does not load: it is not in \
             the accepted JSON subset at byte"
        ),
        "{r}"
    );
}

#[test]
fn the_licence_predicate_refuses_files_that_are_not_the_receipts() {
    let r = refusal(&container(RECEIPT, &[("entertainer.mid", MID)]));
    assert_eq!(
        r,
        IngestRefusal::Licence(provenance::Refusal::MissingFile {
            name: "entertainer.ly".into()
        })
    );
    assert_eq!(r.code(), 101);
    assert_eq!(
        format!("{r}"),
        "score refused by the licence predicate: the receipt lists entertainer.ly, which was \
         not supplied"
    );

    let r = refusal(&container(
        RECEIPT,
        &[
            ("entertainer.ly", LY),
            ("entertainer.mid", MID),
            ("extra.mid", MID),
        ],
    ));
    assert_eq!(
        r,
        IngestRefusal::Licence(provenance::Refusal::UnexpectedFile {
            name: "extra.mid".into()
        })
    );
    assert_eq!(r.code(), 102);

    // One byte of the SMF changed: same size, another SHA-256.
    let mut mid = MID.to_vec();
    mid[100] ^= 1;
    let r = refusal(&container(
        RECEIPT,
        &[("entertainer.ly", LY), ("entertainer.mid", &mid)],
    ));
    assert_eq!(
        r,
        IngestRefusal::Licence(provenance::Refusal::HashMismatch {
            name: "entertainer.mid".into()
        })
    );
    assert_eq!(r.code(), 105);
    assert_eq!(
        format!("{r}"),
        "score refused by the licence predicate: entertainer.mid's SHA-256 is not the receipt's"
    );
}

#[test]
fn the_receipt_must_list_exactly_one_smf_file() {
    // Without the .mid entry the .ly alone states the licence, so the
    // predicate admits the receipt, and the verb has no score to read.
    let (start, end) = mid_entry();
    let text = receipt_text();
    let ly_only = format!("{}{}", &text[..start - ",\n".len()], &text[end..]);
    let r = refusal(&container(ly_only.as_bytes(), &[("entertainer.ly", LY)]));
    assert_eq!(r, IngestRefusal::SmfCount { count: 0 });
    assert_eq!(r.code(), 90);
    assert_eq!(
        format!("{r}"),
        "score refused: the receipt lists no SMF file"
    );

    // A second SMF entry: both admitted, and the verb will not choose.
    let second = text[start..end].replacen("entertainer.mid", "entertainer2.mid", 1);
    let two = format!("{},\n{}{}", &text[..end], second, &text[end..]);
    let r = refusal(&container(
        two.as_bytes(),
        &[
            ("entertainer.ly", LY),
            ("entertainer.mid", MID),
            ("entertainer2.mid", MID),
        ],
    ));
    assert_eq!(r, IngestRefusal::SmfCount { count: 2 });
    assert_eq!(r.code(), 91);
    assert_eq!(
        format!("{r}"),
        "score refused: the receipt lists 2 SMF files, and the ingest verb reads one"
    );
}

#[test]
fn the_smf_reader_refuses_after_the_predicate_admits() {
    // A note-off with nothing sounding: a well-formed SMF with no licence
    // text, so the predicate admits it, and the reader refuses it.
    let orphan = smf(384, &[0x00, 0x80, 60, 0x00, 0x00, 0xFF, 0x2F, 0x00]);
    let r = refusal(&container(
        &receipt_for_mid(&orphan),
        &[("entertainer.ly", LY), ("entertainer.mid", &orphan)],
    ));
    assert_eq!(
        r,
        IngestRefusal::Smf(IngestError::OrphanNoteOff {
            track: 0,
            channel: 0,
            pitch: 60,
            tick: 0
        })
    );
    assert_eq!(r.code(), 82);
    assert_eq!(
        format!("{r}"),
        "score refused by the SMF reader: track 0 channel 0 releases pitch 60 at tick 0 with no \
         note sounding"
    );
}

#[test]
fn the_laws_rescale_refuses_last() {
    // A note from tick 1 to tick 2 at PPQ 384: a valid SMF whose ticks have
    // no whole law tick (3360 / 384 = 35 / 4).
    let off_grid = smf(
        384,
        &[
            0x01, 0x90, 60, 0x40, 0x01, 0x80, 60, 0x00, 0x00, 0xFF, 0x2F, 0x00,
        ],
    );
    assert!(ingest::ingest_smf(&off_grid).is_ok(), "the reader reads it");
    let r = refusal(&container(
        &receipt_for_mid(&off_grid),
        &[("entertainer.ly", LY), ("entertainer.mid", &off_grid)],
    ));
    assert_eq!(
        r,
        IngestRefusal::Law(Refusal::InexactTick {
            event: Event::NoteStart(0),
            tick: 1,
            source_ppq: 384
        })
    );
    assert_eq!(r.code(), 21);
}

/// Every refusal of the verb has a code of its own, none of them zero and
/// none shared with the law's own refusals, and each has a reason.
#[test]
fn every_ingest_refusal_has_its_own_code_and_a_reason() {
    use provenance::Refusal as P;
    let name = || String::from("f.mid");
    let smf_errors = [
        IngestError::NotPlainSmf { offset: 14 },
        IngestError::Invalid("x"),
        IngestError::Malformed("x"),
        IngestError::TrackCountMismatch {
            declared: 3,
            found: 2,
        },
        IngestError::SingleTrackFormat { tracks: 2 },
        IngestError::SmpteTiming,
        IngestError::SmpteOffset { track: 1, tick: 2 },
        IngestError::SequentialFormat,
        IngestError::TooManyTracks,
        IngestError::TickOverflow { track: 1 },
        IngestError::ConflictingTempo { tick: 1 },
        IngestError::ConflictingMeter { tick: 1 },
        IngestError::OrphanNoteOff {
            track: 1,
            channel: 2,
            pitch: 3,
            tick: 4,
        },
        IngestError::UnterminatedNote {
            track: 1,
            channel: 2,
            pitch: 3,
            start_tick: 4,
        },
        IngestError::ZeroLengthNote {
            track: 1,
            channel: 2,
            pitch: 3,
            tick: 4,
        },
        IngestError::Model(score_model::ModelError::NoNotes),
    ];
    let licence = [
        P::Receipt(ReceiptError::InvalidDate),
        P::MissingFile { name: name() },
        P::UnexpectedFile { name: name() },
        P::DuplicateFile { name: name() },
        P::SizeMismatch { name: name() },
        P::HashMismatch { name: name() },
        P::NoAuthors,
        P::MissingDeathYear { author: name() },
        P::MissingFirstPublicationYear,
        P::Unevidenced {
            what: "composition",
        },
        P::NotPublicDomainUs {
            first_publication_year: 1931,
        },
        P::NotPublicDomainEu {
            author: name(),
            death_year: 1956,
        },
        P::AnonymousNotPublicDomainEu {
            role: AuthorRole::Composer,
            first_publication_year: 1956,
        },
        P::MissingTypesetter,
        P::MissingEngraver,
        P::QuoteNotInEvidence { what: "terms" },
        P::Licence(LicenceRefusal::Unknown),
        P::Licence(LicenceRefusal::AllRightsReserved),
        P::Licence(LicenceRefusal::NoRedistribution),
        P::Licence(LicenceRefusal::ShareAlike),
        P::Licence(LicenceRefusal::NonCommercial),
        P::Licence(LicenceRefusal::NoDerivatives),
        P::Licence(LicenceRefusal::AiRestricted),
        P::MissingCreditLedgerId,
        P::UnexpectedCreditLedgerId,
        P::QuoteNegated { what: "terms" },
        P::MissingEditionPublisher,
        P::MissingEditionYear,
        P::EditionBeforeFirstPublication { edition_year: 1900 },
        P::ScholarlyEditionInTerm { edition_year: 2010 },
        P::EditionTermNotShown { edition_year: 2010 },
        P::AnonymousEditionNotPublicDomainUs { edition_year: 1931 },
        P::AnonymousEditionNotPublicDomainEu { edition_year: 1956 },
        P::Unreadable {
            name: name(),
            why: Unreadable::NotUtf8,
        },
        P::InFileMisrecorded { name: name() },
        P::InFileLicenceMismatch { name: name() },
        P::NoInFileLicence,
        P::OwnEngravingStatesLicence { name: name() },
    ];
    let mut all: Vec<IngestRefusal> = smf_errors.into_iter().map(IngestRefusal::Smf).collect();
    all.extend(licence.into_iter().map(IngestRefusal::Licence));
    all.push(IngestRefusal::SmfCount { count: 0 });
    all.push(IngestRefusal::SmfCount { count: 3 });

    let mut codes: Vec<u32> = all.iter().map(IngestRefusal::code).collect();
    assert_eq!(&codes[..16], &(70..=85).collect::<Vec<u32>>()[..]);
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), all.len(), "a code is shared");
    // The law's own codes: 1-14, 20-24, 30-38, 40, 50, 60-63.
    for code in &codes {
        assert!(*code >= 70, "code {code} is in the law's own range");
    }
    for r in &all {
        let text = format!("{r}");
        assert!(text.starts_with("score refused"), "{text}");
    }
    assert_eq!(
        format!(
            "{}",
            IngestRefusal::Licence(P::NotPublicDomainUs {
                first_publication_year: 1931
            })
        ),
        "score refused by the licence predicate: first published in 1931, after 1930: not \
         public domain in the United States"
    );
    assert_eq!(
        format!(
            "{}",
            IngestRefusal::Licence(P::Licence(LicenceRefusal::ShareAlike))
        ),
        "score refused by the licence predicate: the licence refuses the score: it is \
         share-alike"
    );
    // A refusal the law makes keeps the law's code.
    let law = IngestRefusal::Law(Refusal::NoScore);
    assert_eq!(law.code(), 30);
}

/// Predicate version 3's refusals, each with a code of its own in its layer's range
/// and a reason in words: an anonymous work past the EU cut-off for anonymous works,
/// an anonymous edition past either cut-off, and an unknown author with a death year,
/// which the receipt's structure refuses.
#[test]
fn the_predicate_version_3_refusals_have_codes_and_reasons() {
    use provenance::Refusal as P;
    let cases = [
        (
            P::AnonymousNotPublicDomainEu {
                role: AuthorRole::Composer,
                first_publication_year: 1956,
            },
            116,
            "the composer is unknown and the work was first published in 1956, after \
             1955: not public domain in the European Union",
        ),
        (
            P::AnonymousEditionNotPublicDomainUs { edition_year: 1931 },
            145,
            "the anonymous edition of 1931 was published after 1930: not public domain in \
             the United States",
        ),
        (
            P::AnonymousEditionNotPublicDomainEu { edition_year: 1956 },
            146,
            "the anonymous edition of 1956 was published after 1955: not public domain in \
             the European Union",
        ),
        (
            P::Receipt(ReceiptError::AnonymousAuthorDeathYear),
            100,
            "the receipt does not load: an author it records as unknown has a death year",
        ),
    ];
    for (refusal, code, reason) in cases {
        let r = IngestRefusal::Licence(refusal);
        assert_eq!(r.code(), code, "{r}");
        assert_eq!(
            format!("{r}"),
            format!("score refused by the licence predicate: {reason}")
        );
    }
}

// --- The Battle Hymn fixture ----------------------------------------------

const BATTLE_HYMN_RECEIPT: &[u8] =
    include_bytes!("../../provenance/fixtures/battle-hymn/receipt.json");
const BATTLE_HYMN_LY: &[u8] =
    include_bytes!("../../provenance/fixtures/battle-hymn/battle-hymn.ly");
const BATTLE_HYMN_MID: &[u8] =
    include_bytes!("../../provenance/fixtures/battle-hymn/battle-hymn.mid");

/// The ingest verb admits the Battle Hymn fixture: this project's own CC0 engraving of a
/// work whose composer is unknown, from an anonymous edition. The snapshot commits the
/// own-engraving tier and the fixture receipt's digest.
#[test]
fn the_battle_hymn_fixture_is_ingested_as_an_own_engraving() {
    let law = Law::ingest(&container(
        BATTLE_HYMN_RECEIPT,
        &[
            ("battle-hymn.ly", BATTLE_HYMN_LY),
            ("battle-hymn.mid", BATTLE_HYMN_MID),
        ],
    ))
    .unwrap();
    let digest = provenance::Receipt::from_json(BATTLE_HYMN_RECEIPT)
        .unwrap()
        .digest();
    assert_eq!(
        law.provenance(),
        Some(&Provenance {
            tier: Tier::OwnEngraving,
            receipt_digest: digest,
        })
    );
    assert_eq!(law.score().notes().len(), 2, "the placeholder's two notes");
    let bytes = law.snapshot_bytes().unwrap();
    assert_eq!(&bytes[56..60], b"PROV");
    assert_eq!(&bytes[60..64], &[1, 0, 0, 0], "one record");
    assert_eq!(bytes[64], 2, "tier 2, own engraving");
    assert_eq!(&bytes[65..97], &digest);
    assert_eq!(&bytes[97..101], &[0, 0, 0, 0], "no credit-ledger id");
}
