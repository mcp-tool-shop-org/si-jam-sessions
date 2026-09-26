//! The canonical snapshot: the bytes the law hashes.

use alloc::string::String;
use alloc::vec::Vec;

use provenance::Tier;

use crate::grade::Verdict;
use crate::law::Provenance;
use crate::refusal::Refusal;
use crate::score::LawScore;
use crate::take::TakeNote;
use crate::{GATE_SAMPLES, HORIZON_QUANTA, LAW_VERSION, PPQ, QUANTUM_SAMPLES, SAMPLE_RATE};

/// The first eight bytes of every snapshot.
pub const SNAPSHOT_MAGIC: [u8; 8] = *b"SIJAMLAW";

/// The snapshot layout version. Every number is little-endian; every
/// collection is written as a four-byte tag, a `u32` count, then its records in
/// the order stated. Nothing depends on a hash map's order or on a struct's
/// memory layout.
///
/// ```text
/// header  "SIJAMLAW"; format u32; law version u32;
///         PPQ u32; sample rate u32; Q u32; H u32; gate u32;
///         licence predicate version u32; rules year u32;
///         US last public-domain publication year u32;
///         EU last public-domain death year u32;
///         last out-of-term edition year u32
/// "PROV"  no record for a score loaded as bytes, one for an ingested score:
///         tier u8 (public domain 1, own engraving 2, CC-BY-4.0 3);
///         receipt digest, 32 bytes (SHA-256 of the canonical receipt);
///         credit-ledger id: byte length u32; UTF-8 (empty unless tier 3)
/// "TMPO"  per tempo change, by tick:
///         tick u64; us_per_quarter u32; start_sample u64
/// "METR"  per meter change, by tick:
///         tick u64; numerator u8; denominator_pow2 u8
/// "NOTE"  per score note, by id (the canonical order):
///         start_tick u64; pitch u8; track u16; channel u8; end_tick u64;
///         velocity u8; onset_sample u64; duration_samples u64
/// "TAKE"  per take note, by key (onset_sample, pitch, cites):
///         onset_sample u64; pitch u8; velocity u8;
///         cites tag u8 (0 an addition, 1 a citation); cites u32 (0 when tag is 0)
/// "VERD"  per verdict, in grading order (score notes by id, then additions):
///         kind u8 (match 0, early 1, late 2, wrong pitch 3, addition 4,
///         never played 5); score note u32; take note u32; delta i64;
///         onset_sample u64; take pitch u8; score pitch u8
///         (a field the kind does not use is 0)
/// "ROWS"  per verdict, in the same order: byte length u32; UTF-8 text
/// ```
///
/// Ticks are law ticks at PPQ 3360. The header carries every pin, so changing
/// PPQ, the rate, Q, H, the gate, the licence predicate's version or any of
/// its cut-off years moves every hash, as does a new law version. The receipt
/// digest is hashed, so any change to a receipt, including to a file it
/// receipts, moves the hash of every score ingested under it. The rows are
/// hashed too: a change to a row's wording is a change to a label.
///
/// Format 2 added the predicate's pins to the header and the "PROV" section.
/// Law version 4 keeps format 2: its live sessions change which verdicts a
/// snapshot holds (an unreached uncited score note has none), not how any of
/// them is written.
pub const SNAPSHOT_FORMAT: u32 = 2;

struct Out {
    bytes: Vec<u8>,
}

impl Out {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| Refusal::OutOfMemory)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn u8(&mut self, v: u8) -> Result<(), Refusal> {
        self.put(&[v])
    }

    fn u16(&mut self, v: u16) -> Result<(), Refusal> {
        self.put(&v.to_le_bytes())
    }

    fn u32(&mut self, v: u32) -> Result<(), Refusal> {
        self.put(&v.to_le_bytes())
    }

    fn u64(&mut self, v: u64) -> Result<(), Refusal> {
        self.put(&v.to_le_bytes())
    }

    fn i64(&mut self, v: i64) -> Result<(), Refusal> {
        self.put(&v.to_le_bytes())
    }

    fn section(&mut self, tag: &[u8; 4], count: usize) -> Result<(), Refusal> {
        self.put(tag)?;
        self.u32(u32::try_from(count).map_err(|_| Refusal::Overflow)?)
    }
}

/// The pins the header carries, in order after the magic.
pub(crate) const HEADER_PINS: [u32; 12] = [
    SNAPSHOT_FORMAT,
    LAW_VERSION,
    PPQ,
    SAMPLE_RATE,
    QUANTUM_SAMPLES,
    HORIZON_QUANTA,
    GATE_SAMPLES,
    provenance::PREDICATE_VERSION,
    widen(provenance::RULES_YEAR),
    widen(provenance::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR),
    widen(provenance::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR),
    widen(provenance::LAST_OUT_OF_TERM_EDITION_YEAR),
];

const fn widen(year: u16) -> u32 {
    // `u32::from` is not const; this widening cannot lose a bit.
    #[allow(clippy::as_conversions)]
    let wide = year as u32;
    wide
}

/// The tier's code in the "PROV" record.
fn tier_code(tier: &Tier) -> u8 {
    match tier {
        Tier::PublicDomain => 1,
        Tier::OwnEngraving => 2,
        Tier::CcBy40 { .. } => 3,
    }
}

pub(crate) fn encode(
    score: &LawScore,
    provenance: Option<&Provenance>,
    take: &[TakeNote],
    verdicts: &[Verdict],
    rows: &[String],
) -> Result<Vec<u8>, Refusal> {
    let mut out = Out { bytes: Vec::new() };

    out.put(&SNAPSHOT_MAGIC)?;
    for pin in HEADER_PINS {
        out.u32(pin)?;
    }

    out.section(b"PROV", usize::from(provenance.is_some()))?;
    if let Some(p) = provenance {
        out.u8(tier_code(&p.tier))?;
        out.put(&p.receipt_digest)?;
        let ledger = match &p.tier {
            Tier::CcBy40 { credit_ledger_id } => credit_ledger_id.as_bytes(),
            Tier::PublicDomain | Tier::OwnEngraving => &[],
        };
        out.u32(u32::try_from(ledger.len()).map_err(|_| Refusal::Overflow)?)?;
        out.put(ledger)?;
    }

    out.section(b"TMPO", score.tempo().len())?;
    for t in score.tempo() {
        out.u64(t.tick)?;
        out.u32(t.us_per_quarter)?;
        out.u64(t.start_sample)?;
    }

    out.section(b"METR", score.meter().len())?;
    for m in score.meter() {
        out.u64(m.tick)?;
        out.u8(m.numerator)?;
        out.u8(m.denominator_pow2)?;
    }

    out.section(b"NOTE", score.notes().len())?;
    for n in score.notes() {
        out.u64(n.start_tick)?;
        out.u8(n.pitch)?;
        out.u16(n.track)?;
        out.u8(n.channel)?;
        out.u64(n.end_tick)?;
        out.u8(n.velocity)?;
        out.u64(n.onset_sample)?;
        out.u64(n.duration_samples)?;
    }

    out.section(b"TAKE", take.len())?;
    for t in take {
        out.u64(t.onset_sample)?;
        out.u8(t.pitch)?;
        out.u8(t.velocity)?;
        match t.cites {
            None => {
                out.u8(0)?;
                out.u32(0)?;
            }
            Some(id) => {
                out.u8(1)?;
                out.u32(id.0)?;
            }
        }
    }

    out.section(b"VERD", verdicts.len())?;
    for v in verdicts {
        let (note, take_note, delta, onset_sample, take_pitch, score_pitch) = match *v {
            Verdict::Cited {
                note,
                take,
                delta,
                onset_sample,
                take_pitch,
                score_pitch,
                ..
            } => (note.0, take, delta, onset_sample, take_pitch, score_pitch),
            Verdict::Addition {
                take,
                onset_sample,
                pitch,
            } => (0, take, 0, onset_sample, pitch, 0),
            Verdict::NeverPlayed {
                note,
                onset_sample,
                pitch,
            } => (note.0, 0, 0, onset_sample, 0, pitch),
        };
        out.u8(v.kind_code())?;
        out.u32(note)?;
        out.u32(take_note)?;
        out.i64(delta)?;
        out.u64(onset_sample)?;
        out.u8(take_pitch)?;
        out.u8(score_pitch)?;
    }

    out.section(b"ROWS", rows.len())?;
    for row in rows {
        out.u32(u32::try_from(row.len()).map_err(|_| Refusal::Overflow)?)?;
        out.put(row.as_bytes())?;
    }

    Ok(out.bytes)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
mod tests {
    use crate::Law;
    use crate::score::tests::ingested;
    use crate::take::{ScoreNoteId, TakeNote};

    /// Walks a snapshot by its declared layout and checks every count and
    /// length lines up with the end of the bytes.
    #[test]
    fn the_layout_is_the_declared_layout() {
        let mut law = Law::load(&ingested()).unwrap();
        law.admit(&[
            TakeNote {
                onset_sample: 100,
                pitch: 60,
                velocity: 70,
                cites: Some(ScoreNoteId(0)),
            },
            TakeNote {
                onset_sample: 5_000,
                pitch: 90,
                velocity: 20,
                cites: None,
            },
        ])
        .unwrap();
        let bytes = law.snapshot_bytes().unwrap();
        let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());

        let mut at = 8 + 12 * 4;
        assert_eq!(&bytes[at..at + 4], b"PROV");
        assert_eq!(u32_at(at + 4), 0, "a score loaded as bytes has no receipt");
        at += 8;
        for (tag, record) in [
            (b"TMPO", 20),
            (b"METR", 10),
            (b"NOTE", 37),
            (b"TAKE", 15),
            (b"VERD", 27),
        ] {
            assert_eq!(&bytes[at..at + 4], tag);
            let count = u32_at(at + 4) as usize;
            at += 8 + count * record;
        }
        assert_eq!(&bytes[at..at + 4], b"ROWS");
        let rows = u32_at(at + 4);
        assert_eq!(
            rows, 5,
            "4 score notes (1 cited, 3 never played) and 1 addition"
        );
        at += 8;
        let texts = law.rows().unwrap();
        for text in &texts {
            let len = u32_at(at) as usize;
            assert_eq!(&bytes[at + 4..at + 4 + len], text.as_bytes());
            at += 4 + len;
        }
        assert_eq!(at, bytes.len(), "nothing after the rows");
        assert_eq!(
            texts[0],
            "note 0: onset +100 samples (+2.1 ms) vs gate \u{b1}1920, pitch 60 vs 60: match"
        );
    }
}
