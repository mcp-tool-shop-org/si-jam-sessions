//! The bytes a host passes in: a container for the ingest verb, an ingested
//! score, and a take; and the bytes it reads out of the frame export.
//!
//! Every layout is explicit little-endian, versioned and exact. A decoder
//! refuses a wrong magic, a wrong version, a truncation, and any byte after the
//! layout. It checks each count and length against the bytes that remain
//! before it allocates anything for it, so the host's buffer bounds every
//! allocation.
//!
//! ```text
//! container  "SJIN"; version u32 = 1;
//!            receipt length u32, then the receipt's bytes (its JSON, as committed);
//!            file count u32, then per file, names strictly increasing in byte order:
//!                name length u32, then the name (UTF-8, not empty);
//!                content length u32, then the file's bytes
//! score      "SJSC"; version u32 = 1; source_ppq u16;
//!            tempo count u32, then per change: tick u64; us_per_quarter u32
//!            meter count u32, then per change: tick u64; numerator u8; denominator_pow2 u8
//!            note count u32, then per note: start_tick u64; pitch u8; track u16;
//!                channel u8; end_tick u64; velocity u8
//! take       "SJTK"; version u32 = 1;
//!            note count u32, then per note: onset_sample u64; pitch u8; velocity u8;
//!                cites tag u8 (0 an addition, 1 a citation); cites u32 (0 when tag is 0)
//! frames     "SJFR"; frames version u32 = 1; first quantum u64; last quantum u64;
//!            note count u32, then per note-on, in frame order (onset, voice,
//!                pitch, an addition before a citation, note id):
//!                onset_sample u64; voice u8 (score 0, take 1, live 2);
//!                note tag u8 (1: note_id names a score note; 0: an addition, note_id 0);
//!                note_id u32; pitch u8; velocity u8; duration_samples u64
//!            beat count u32, then per beat, by onset:
//!                onset_sample u64; bar u64; beat u8; downbeat u8 (1 on beat 0, else 0)
//! ```
//!
//! The frames are the law's output ([`crate::Law::frames`]); their decoder is
//! here for hosts, and it checks the layout and the tags, as the others do.
//!
//! The container is what the ingest verb reads ([`crate::Law::ingest`]): a
//! receipt and every file it receipts, each file under its name in the
//! receipt. Names must strictly increase, so a set of files has exactly one
//! container and no name appears twice. Which name is which file, and whether
//! the set is the receipt's, is the licence predicate's to check.
//!
//! Decoding checks the layout only. What the values mean (sorted events, a
//! velocity in range, a citation that names a score note, the commit horizon,
//! a receipt that admits its files) is checked by the law, on the same path a
//! native caller takes. The encoders are here so a native harness can hand the
//! wasm law exactly the bytes it would decode.

use alloc::vec::Vec;

use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};

use crate::frames::{Beat, FrameNote, Frames, Voice};
use crate::refusal::{Refusal, WireFault};
use crate::take::{ScoreNoteId, TakeNote};

/// The first four bytes of a container for the ingest verb.
pub const CONTAINER_MAGIC: [u8; 4] = *b"SJIN";
/// The first four bytes of a score.
pub const SCORE_MAGIC: [u8; 4] = *b"SJSC";
/// The first four bytes of a take.
pub const TAKE_MAGIC: [u8; 4] = *b"SJTK";
/// The wire version every input layout carries.
pub const WIRE_VERSION: u32 = 1;
/// The first four bytes of the frames the frame export writes.
pub const FRAMES_MAGIC: [u8; 4] = *b"SJFR";
/// The frame layout's version.
pub const FRAMES_VERSION: u32 = 1;

/// The fewest bytes a file of a container takes: its two lengths.
const FILE_RECORD: usize = 8;
const TEMPO_RECORD: usize = 12;
const METER_RECORD: usize = 10;
const NOTE_RECORD: usize = 21;
const TAKE_RECORD: usize = 15;
/// A frame's note-on, in bytes.
pub const FRAME_NOTE_RECORD: usize = 24;
/// A frame's beat, in bytes.
pub const FRAME_BEAT_RECORD: usize = 18;
/// The frame layout's fixed part: magic, version, the two quanta and the two
/// counts.
const FRAMES_FIXED: usize = 32;

/// A container, decoded: the receipt's bytes and each file under its name,
/// all borrowed from the host's buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container<'a> {
    /// The receipt, as its JSON bytes.
    pub receipt: &'a [u8],
    /// The files, names strictly increasing in byte order.
    pub files: Vec<ContainerFile<'a>>,
}

/// One file of a container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerFile<'a> {
    /// The file's name in the receipt: UTF-8, not empty.
    pub name: &'a str,
    pub bytes: &'a [u8],
}

impl<'a> Container<'a> {
    /// The bytes of the file with this name, if the container holds one.
    pub fn file(&self, name: &str) -> Option<&'a [u8]> {
        self.files
            .binary_search_by(|f| f.name.cmp(name))
            .ok()
            .and_then(|i| self.files.get(i))
            .map(|f| f.bytes)
    }
}

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

    /// A `u32` length, then that many bytes, borrowed.
    fn blob(&mut self) -> Result<&'a [u8], Refusal> {
        let len = usize::try_from(self.u32()?).map_err(|_| Refusal::Overflow)?;
        self.take(len)
    }

    fn header(&mut self, magic: &[u8; 4]) -> Result<(), Refusal> {
        self.versioned_header(magic, WIRE_VERSION)
    }

    fn versioned_header(&mut self, magic: &[u8; 4], version: u32) -> Result<(), Refusal> {
        let at_magic = self.fault(WireFault::Magic);
        if self.array::<4>()? != *magic {
            return Err(at_magic);
        }
        let at_version = self.fault(WireFault::Version);
        if self.u32()? != version {
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

/// Decodes a container. The receipt and the files are not yet read;
/// [`crate::Law::ingest`] reads them.
pub fn decode_container(bytes: &[u8]) -> Result<Container<'_>, Refusal> {
    let mut r = Reader { bytes, at: 0 };
    r.header(&CONTAINER_MAGIC)?;
    let receipt = r.blob()?;
    let (count, mut files) = r.records::<ContainerFile<'_>>(FILE_RECORD)?;
    for _ in 0..count {
        let at_name = r.at;
        let name = core::str::from_utf8(r.blob()?)
            .ok()
            .filter(|name| !name.is_empty())
            .ok_or(Refusal::Wire {
                fault: WireFault::FileName,
                offset: at_name,
            })?;
        if files.last().is_some_and(|f| f.name >= name) {
            return Err(Refusal::Wire {
                fault: WireFault::FileOrder,
                offset: at_name,
            });
        }
        let content = r.blob()?;
        files.push(ContainerFile {
            name,
            bytes: content,
        });
    }
    r.finish()?;
    Ok(Container { receipt, files })
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

/// Encodes a container in the layout [`decode_container`] reads. The files
/// are written sorted by name; two files with one name are refused, as the
/// decoder would refuse them.
pub fn encode_container(receipt: &[u8], files: &[ContainerFile<'_>]) -> Result<Vec<u8>, Refusal> {
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(files.len())
        .map_err(|_| Refusal::OutOfMemory)?;
    sorted.extend_from_slice(files);
    sorted.sort_unstable_by(|a, b| a.name.cmp(b.name));
    let mut out = Vec::new();
    put(&mut out, &CONTAINER_MAGIC)?;
    put(&mut out, &WIRE_VERSION.to_le_bytes())?;
    put(&mut out, &count(receipt.len(), Refusal::Overflow)?)?;
    put(&mut out, receipt)?;
    put(&mut out, &count(sorted.len(), Refusal::Overflow)?)?;
    let mut previous: Option<&str> = None;
    for f in &sorted {
        let at = out.len();
        if f.name.is_empty() {
            return Err(Refusal::Wire {
                fault: WireFault::FileName,
                offset: at,
            });
        }
        if previous.is_some_and(|p| p == f.name) {
            return Err(Refusal::Wire {
                fault: WireFault::FileOrder,
                offset: at,
            });
        }
        put(&mut out, &count(f.name.len(), Refusal::Overflow)?)?;
        put(&mut out, f.name.as_bytes())?;
        put(&mut out, &count(f.bytes.len(), Refusal::Overflow)?)?;
        put(&mut out, f.bytes)?;
        previous = Some(f.name);
    }
    Ok(out)
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

/// Encodes frames in the layout [`decode_frames`] reads. The frame export
/// writes these bytes; the notes and beats go out in the order they are held,
/// which [`crate::Law::frames`] makes the frame order.
pub fn encode_frames(frames: &Frames) -> Result<Vec<u8>, Refusal> {
    let size = frames
        .notes
        .len()
        .checked_mul(FRAME_NOTE_RECORD)
        .and_then(|n| {
            frames
                .beats
                .len()
                .checked_mul(FRAME_BEAT_RECORD)
                .and_then(|b| n.checked_add(b))
        })
        .and_then(|n| n.checked_add(FRAMES_FIXED))
        .ok_or(Refusal::Overflow)?;
    let mut out = Vec::new();
    out.try_reserve_exact(size)
        .map_err(|_| Refusal::OutOfMemory)?;
    put(&mut out, &FRAMES_MAGIC)?;
    put(&mut out, &FRAMES_VERSION.to_le_bytes())?;
    put(&mut out, &frames.first_quantum.to_le_bytes())?;
    put(&mut out, &frames.last_quantum.to_le_bytes())?;
    put(&mut out, &count(frames.notes.len(), Refusal::Overflow)?)?;
    for n in &frames.notes {
        let (tag, id) = match n.note {
            None => (0u8, 0u32),
            Some(id) => (1u8, id.0),
        };
        put(&mut out, &n.onset_sample.to_le_bytes())?;
        put(&mut out, &[n.voice.code(), tag])?;
        put(&mut out, &id.to_le_bytes())?;
        put(&mut out, &[n.pitch, n.velocity])?;
        put(&mut out, &n.duration_samples.to_le_bytes())?;
    }
    put(&mut out, &count(frames.beats.len(), Refusal::Overflow)?)?;
    for b in &frames.beats {
        put(&mut out, &b.onset_sample.to_le_bytes())?;
        put(&mut out, &b.bar.to_le_bytes())?;
        put(&mut out, &[b.beat, u8::from(b.downbeat)])?;
    }
    Ok(out)
}

/// Decodes frame bytes, as a host reads them out of the frame export.
///
/// Beyond the layout, it refuses a voice other than 0, 1 or 2, a score frame
/// that does not name its own note, a note tag of 0 with an id or a tag above
/// 1, and a downbeat flag that is not 1 exactly on beat 0.
pub fn decode_frames(bytes: &[u8]) -> Result<Frames, Refusal> {
    let mut r = Reader { bytes, at: 0 };
    r.versioned_header(&FRAMES_MAGIC, FRAMES_VERSION)?;
    let first_quantum = r.u64()?;
    let last_quantum = r.u64()?;
    let (count, mut notes) = r.records::<FrameNote>(FRAME_NOTE_RECORD)?;
    for _ in 0..count {
        let onset_sample = r.u64()?;
        let at_voice = r.fault(WireFault::Voice);
        let voice = match r.u8()? {
            0 => Voice::Score,
            1 => Voice::Take,
            2 => Voice::Live,
            _ => return Err(at_voice),
        };
        let at_tag = r.fault(WireFault::CitationTag);
        let tag = r.u8()?;
        let id = r.u32()?;
        let note = match (tag, id) {
            (0, 0) => None,
            (1, id) => Some(ScoreNoteId(id)),
            _ => return Err(at_tag),
        };
        if voice == Voice::Score && note.is_none() {
            return Err(at_voice);
        }
        notes.push(FrameNote {
            onset_sample,
            voice,
            note,
            pitch: r.u8()?,
            velocity: r.u8()?,
            duration_samples: r.u64()?,
        });
    }
    let (count, mut beats) = r.records::<Beat>(FRAME_BEAT_RECORD)?;
    for _ in 0..count {
        let onset_sample = r.u64()?;
        let bar = r.u64()?;
        let beat = r.u8()?;
        let at_flag = r.fault(WireFault::Downbeat);
        let downbeat = match (r.u8()?, beat) {
            (1, 0) => true,
            (0, b) if b != 0 => false,
            _ => return Err(at_flag),
        };
        beats.push(Beat {
            onset_sample,
            bar,
            beat,
            downbeat,
        });
    }
    r.finish()?;
    Ok(Frames {
        first_quantum,
        last_quantum,
        notes,
        beats,
    })
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
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

    fn files() -> vec::Vec<ContainerFile<'static>> {
        vec![
            ContainerFile {
                name: "b.mid",
                bytes: b"\x00\x01",
            },
            ContainerFile {
                name: "a.ly",
                bytes: b"ly",
            },
        ]
    }

    /// A container written in the order given, not sorted: what a careless
    /// host could send.
    fn raw(receipt: &[u8], files: &[(&[u8], &[u8])]) -> vec::Vec<u8> {
        let mut out = vec::Vec::new();
        out.extend_from_slice(b"SJIN");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(receipt.len() as u32).to_le_bytes());
        out.extend_from_slice(receipt);
        out.extend_from_slice(&(files.len() as u32).to_le_bytes());
        for (name, bytes) in files {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        out
    }

    #[test]
    fn a_container_round_trips_with_its_files_sorted_by_name() {
        let bytes = encode_container(b"{}", &files()).unwrap();
        assert_eq!(
            bytes.len(),
            4 + 4 + (4 + 2) + 4 + (4 + 4 + 4 + 2) + (4 + 5 + 4 + 2)
        );
        assert_eq!(
            bytes,
            raw(b"{}", &[(b"a.ly", b"ly"), (b"b.mid", b"\x00\x01")]),
            "the layout, written out"
        );
        let c = decode_container(&bytes).unwrap();
        assert_eq!(c.receipt, b"{}");
        assert_eq!(c.files, [files()[1], files()[0]], "sorted by name");
        assert_eq!(c.file("b.mid"), Some(&b"\x00\x01"[..]));
        assert_eq!(c.file("a.ly"), Some(&b"ly"[..]));
        assert_eq!(c.file("c"), None);
        // No files is a layout like any other; the predicate refuses what is
        // missing.
        let empty = encode_container(b"", &[]).unwrap();
        assert_eq!(
            decode_container(&empty),
            Ok(Container {
                receipt: b"",
                files: vec![]
            })
        );
    }

    #[test]
    fn a_container_is_refused_at_the_offset_of_its_fault() {
        let good = encode_container(b"{}", &files()).unwrap();
        let fault = |bytes: &[u8]| decode_container(bytes).map(|_| ());

        let mut bad = good.clone();
        bad[0] = b'X';
        assert_eq!(fault(&bad), wire(WireFault::Magic, 0));
        let mut bad = good.clone();
        bad[4] = 2;
        assert_eq!(fault(&bad), wire(WireFault::Version, 4));
        let mut bad = good.clone();
        bad.push(0);
        assert_eq!(fault(&bad), wire(WireFault::Trailing, good.len()));
        assert!(matches!(
            fault(&good[..good.len() - 1]),
            Err(Refusal::Wire {
                fault: WireFault::Truncated,
                ..
            })
        ));
        // A score's bytes are not a container's.
        assert_eq!(
            fault(&encode_score(&ingested()).unwrap()),
            wire(WireFault::Magic, 0)
        );

        // The first file's name length is at 4 + 4 + 6 + 4 = 18.
        let first = 18;
        let mut bad = good.clone();
        bad[first + 4] = 0xFF;
        assert_eq!(fault(&bad), wire(WireFault::FileName, first));
        assert_eq!(
            fault(&raw(b"{}", &[(b"", b"x")])),
            wire(WireFault::FileName, first)
        );
        // Out of order, and a name twice: refused at the second name.
        let second = first + 4 + 4 + 4 + 2;
        assert_eq!(
            fault(&raw(b"{}", &[(b"b.ly", b"ly"), (b"a.mid", b"\x00\x01")])),
            wire(WireFault::FileOrder, second)
        );
        assert_eq!(
            fault(&raw(b"{}", &[(b"a.ly", b"ly"), (b"a.ly", b"ly")])),
            wire(WireFault::FileOrder, second)
        );
        // A length past the end.
        let mut bad = good.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(fault(&bad), wire(WireFault::Truncated, 12));
        // Four billion files with no bytes behind them: refused before anything
        // is allocated for them.
        let mut bad = raw(b"{}", &[]);
        bad[14..18].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(fault(&bad), wire(WireFault::Truncated, 18));
    }

    #[test]
    fn the_container_encoder_refuses_what_the_decoder_would() {
        let twice = [files()[0], files()[0]];
        assert_eq!(
            encode_container(b"{}", &twice).map(|_| ()),
            wire(WireFault::FileOrder, 18 + 4 + 5 + 4 + 2)
        );
        let unnamed = [ContainerFile {
            name: "",
            bytes: b"x",
        }];
        assert_eq!(
            encode_container(b"{}", &unnamed).map(|_| ()),
            wire(WireFault::FileName, 18)
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
