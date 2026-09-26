//! The bytes a host passes in: an ingested score, and a take.
//!
//! Both layouts are explicit little-endian, versioned and exact. A decoder
//! refuses a wrong magic, a wrong version, a truncation, and any byte after the
//! layout. It checks each count against the bytes that remain before it
//! allocates anything for it, so the host's buffer bounds every allocation.
//!
//! ```text
//! score  "SJSC"; version u32 = 1; source_ppq u16;
//!        tempo count u32, then per change: tick u64; us_per_quarter u32
//!        meter count u32, then per change: tick u64; numerator u8; denominator_pow2 u8
//!        note count u32, then per note: start_tick u64; pitch u8; track u16;
//!            channel u8; end_tick u64; velocity u8
//! take   "SJTK"; version u32 = 1;
//!        note count u32, then per note: onset_sample u64; pitch u8; velocity u8;
//!            cites tag u8 (0 an addition, 1 a citation); cites u32 (0 when tag is 0)
//! ```
//!
//! Decoding checks the layout only. What the values mean (sorted events, a
//! velocity in range, a citation that names a score note, the commit horizon)
//! is checked by the law, on the same path a native caller takes. The encoders
//! are here so a native harness can hand the wasm law exactly the bytes it
//! would decode.

use alloc::vec::Vec;

use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};

use crate::refusal::{Refusal, WireFault};
use crate::take::{ScoreNoteId, TakeNote};

/// The first four bytes of a score.
pub const SCORE_MAGIC: [u8; 4] = *b"SJSC";
/// The first four bytes of a take.
pub const TAKE_MAGIC: [u8; 4] = *b"SJTK";
/// The wire version both layouts carry.
pub const WIRE_VERSION: u32 = 1;

const TEMPO_RECORD: usize = 12;
const METER_RECORD: usize = 10;
const NOTE_RECORD: usize = 21;
const TAKE_RECORD: usize = 15;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn fault(&self, fault: WireFault) -> Refusal {
        Refusal::Wire {
            fault,
            offset: self.at,
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Refusal> {
        let truncated = self.fault(WireFault::Truncated);
        let end = self.at.checked_add(n).ok_or(truncated)?;
        let slice = self.bytes.get(self.at..end).ok_or(truncated)?;
        self.at = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Refusal> {
        let truncated = self.fault(WireFault::Truncated);
        <[u8; N]>::try_from(self.take(N)?).map_err(|_| truncated)
    }

    fn u8(&mut self) -> Result<u8, Refusal> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, Refusal> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Refusal> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Refusal> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn header(&mut self, magic: &[u8; 4]) -> Result<(), Refusal> {
        let at_magic = self.fault(WireFault::Magic);
        if self.array::<4>()? != *magic {
            return Err(at_magic);
        }
        let at_version = self.fault(WireFault::Version);
        if self.u32()? != WIRE_VERSION {
            return Err(at_version);
        }
        Ok(())
    }

    /// Reads a count and refuses it unless that many records of `record`
    /// bytes fit in what remains; returns an empty vector with room for them.
    fn records<T>(&mut self, record: usize) -> Result<(usize, Vec<T>), Refusal> {
        let count = usize::try_from(self.u32()?).map_err(|_| Refusal::Overflow)?;
        let truncated = self.fault(WireFault::Truncated);
        let needed = count.checked_mul(record).ok_or(truncated)?;
        let remaining = self.bytes.len().checked_sub(self.at).ok_or(truncated)?;
        if needed > remaining {
            return Err(truncated);
        }
        let mut v = Vec::new();
        v.try_reserve_exact(count)
            .map_err(|_| Refusal::OutOfMemory)?;
        Ok((count, v))
    }

    fn finish(&self) -> Result<(), Refusal> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(self.fault(WireFault::Trailing))
        }
    }
}

/// Decodes a score. The result is not yet validated; [`crate::Law::load`]
/// validates it.
pub fn decode_score(bytes: &[u8]) -> Result<IngestedScore, Refusal> {
    let mut r = Reader { bytes, at: 0 };
    r.header(&SCORE_MAGIC)?;
    let source_ppq = r.u16()?;
    let (count, mut tempo) = r.records(TEMPO_RECORD)?;
    for _ in 0..count {
        tempo.push(TempoChange {
            tick: r.u64()?,
            us_per_quarter: r.u32()?,
        });
    }
    let (count, mut meter) = r.records(METER_RECORD)?;
    for _ in 0..count {
        meter.push(MeterChange {
            tick: r.u64()?,
            numerator: r.u8()?,
            denominator_pow2: r.u8()?,
        });
    }
    let (count, mut notes) = r.records(NOTE_RECORD)?;
    for _ in 0..count {
        notes.push(IngestedNote {
            start_tick: r.u64()?,
            pitch: r.u8()?,
            track: r.u16()?,
            channel: r.u8()?,
            end_tick: r.u64()?,
            velocity: r.u8()?,
        });
    }
    r.finish()?;
    Ok(IngestedScore {
        source_ppq,
        tempo,
        meter,
        notes,
    })
}

/// Decodes a take. The notes are not yet admitted; [`crate::Law::admit`]
/// checks them.
pub fn decode_take(bytes: &[u8]) -> Result<Vec<TakeNote>, Refusal> {
    let mut r = Reader { bytes, at: 0 };
    r.header(&TAKE_MAGIC)?;
    let (count, mut notes) = r.records(TAKE_RECORD)?;
    for _ in 0..count {
        let onset_sample = r.u64()?;
        let pitch = r.u8()?;
        let velocity = r.u8()?;
        let at_tag = r.fault(WireFault::CitationTag);
        let tag = r.u8()?;
        let id = r.u32()?;
        let cites = match (tag, id) {
            (0, 0) => None,
            (1, id) => Some(ScoreNoteId(id)),
            _ => return Err(at_tag),
        };
        notes.push(TakeNote {
            onset_sample,
            pitch,
            velocity,
            cites,
        });
    }
    r.finish()?;
    Ok(notes)
}

fn count(len: usize, refusal: Refusal) -> Result<[u8; 4], Refusal> {
    Ok(u32::try_from(len).map_err(|_| refusal)?.to_le_bytes())
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Refusal> {
    out.try_reserve(bytes.len())
        .map_err(|_| Refusal::OutOfMemory)?;
    out.extend_from_slice(bytes);
    Ok(())
}

/// Encodes a score in the layout [`decode_score`] reads.
pub fn encode_score(score: &IngestedScore) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::new();
    put(&mut out, &SCORE_MAGIC)?;
    put(&mut out, &WIRE_VERSION.to_le_bytes())?;
    put(&mut out, &score.source_ppq.to_le_bytes())?;
    put(&mut out, &count(score.tempo.len(), Refusal::Overflow)?)?;
    for t in &score.tempo {
        put(&mut out, &t.tick.to_le_bytes())?;
        put(&mut out, &t.us_per_quarter.to_le_bytes())?;
    }
    put(&mut out, &count(score.meter.len(), Refusal::Overflow)?)?;
    for m in &score.meter {
        put(&mut out, &m.tick.to_le_bytes())?;
        put(&mut out, &[m.numerator, m.denominator_pow2])?;
    }
    let notes = score.notes.len();
    put(
        &mut out,
        &count(notes, Refusal::TooManyNotes { count: notes })?,
    )?;
    for n in &score.notes {
        put(&mut out, &n.start_tick.to_le_bytes())?;
        put(&mut out, &[n.pitch])?;
        put(&mut out, &n.track.to_le_bytes())?;
        put(&mut out, &[n.channel])?;
        put(&mut out, &n.end_tick.to_le_bytes())?;
        put(&mut out, &[n.velocity])?;
    }
    Ok(out)
}

/// Encodes a take in the layout [`decode_take`] reads.
pub fn encode_take(take: &[TakeNote]) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::new();
    put(&mut out, &TAKE_MAGIC)?;
    put(&mut out, &WIRE_VERSION.to_le_bytes())?;
    put(
        &mut out,
        &count(take.len(), Refusal::TakeTooLong { count: take.len() })?,
    )?;
    for t in take {
        put(&mut out, &t.onset_sample.to_le_bytes())?;
        put(&mut out, &[t.pitch, t.velocity])?;
        let (tag, id) = match t.cites {
            None => (0u8, 0u32),
            Some(id) => (1u8, id.0),
        };
        put(&mut out, &[tag])?;
        put(&mut out, &id.to_le_bytes())?;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
mod tests {
    use super::*;
    use crate::score::tests::ingested;
    use alloc::vec;

    fn take() -> vec::Vec<TakeNote> {
        vec![
            TakeNote {
                onset_sample: 0x0102_0304_0506_0708,
                pitch: 60,
                velocity: 1,
                cites: Some(ScoreNoteId(0xAABB_CCDD)),
            },
            TakeNote {
                onset_sample: u64::MAX,
                pitch: 127,
                velocity: 127,
                cites: None,
            },
        ]
    }

    #[test]
    fn both_layouts_round_trip() {
        let score = ingested();
        let bytes = encode_score(&score).unwrap();
        assert_eq!(bytes.len(), 4 + 4 + 2 + 4 + 2 * 12 + 4 + 10 + 4 + 4 * 21);
        assert_eq!(decode_score(&bytes), Ok(score));
        let bytes = encode_take(&take()).unwrap();
        assert_eq!(bytes.len(), 4 + 4 + 4 + 2 * 15);
        assert_eq!(&bytes[12..20], &[8, 7, 6, 5, 4, 3, 2, 1], "little-endian");
        assert_eq!(decode_take(&bytes), Ok(take()));
    }

    fn wire(fault: WireFault, offset: usize) -> Result<(), Refusal> {
        Err(Refusal::Wire { fault, offset })
    }

    #[test]
    fn a_layout_error_is_refused_at_its_offset() {
        let score = encode_score(&ingested()).unwrap();
        let mut bad = score.clone();
        bad[0] = b'X';
        assert_eq!(decode_score(&bad).map(|_| ()), wire(WireFault::Magic, 0));
        let mut bad = score.clone();
        bad[4] = 2;
        assert_eq!(decode_score(&bad).map(|_| ()), wire(WireFault::Version, 4));
        let bad = &score[..score.len() - 1];
        assert!(matches!(
            decode_score(bad),
            Err(Refusal::Wire {
                fault: WireFault::Truncated,
                ..
            })
        ));
        let mut bad = score.clone();
        bad.push(0);
        assert_eq!(
            decode_score(&bad).map(|_| ()),
            wire(WireFault::Trailing, score.len())
        );
        assert_eq!(decode_score(&[]).map(|_| ()), wire(WireFault::Truncated, 0));
        // A take's bytes are not a score's.
        let take_bytes = encode_take(&take()).unwrap();
        assert_eq!(
            decode_score(&take_bytes).map(|_| ()),
            wire(WireFault::Magic, 0)
        );
    }

    #[test]
    fn a_count_is_checked_before_anything_is_allocated() {
        let mut bytes = encode_take(&[]).unwrap();
        // Claim four billion notes with no bytes behind them.
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decode_take(&bytes).map(|_| ()),
            wire(WireFault::Truncated, 12)
        );
    }

    #[test]
    fn a_citation_tag_is_zero_with_no_id_or_one() {
        let good = encode_take(&take()).unwrap();
        let tag = 12 + 8 + 2;
        let mut bad = good.clone();
        bad[tag] = 2;
        assert_eq!(
            decode_take(&bad).map(|_| ()),
            wire(WireFault::CitationTag, tag)
        );
        // The second note is an addition: tag 0 with a non-zero id.
        let second_id = 12 + 15 + 8 + 2 + 1;
        let mut bad = good;
        bad[second_id] = 1;
        assert_eq!(
            decode_take(&bad).map(|_| ()),
            wire(WireFault::CitationTag, second_id - 1)
        );
    }
}
