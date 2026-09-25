//! Ingest: Standard MIDI File bytes to the [`IngestedScore`] the law reads.
//!
//! Every rule below has a test, and every refusal names where it happened.
//!
//! - **Layout.** The bytes must be a plain SMF: they start with one `MThd` chunk of length
//!   6, and every later chunk is an `MTrk`. A RIFF (RMID) wrapper is refused, and so is any
//!   other chunk, because midly would unwrap the one and skip the other unread, even with
//!   `strict`. A ragged end (a chunk longer than the bytes left, or a tail too short to be
//!   a chunk) is left to midly's `strict` parser, which refuses it as malformed.
//! - **Timing.** The file must be metrical (ticks per quarter note). A timecode (SMPTE)
//!   file is refused from its header, before midly reads it: midly 0.5.3 negates the
//!   division's high byte as an `i8`, which overflows for `0x80` and panics under
//!   overflow checks. An SMPTE-offset event in a metrical file is refused too, because it
//!   pins the music to a timecode rather than to the tick grid.
//! - **Parsing.** midly with its `strict` feature: a malformed file is refused, never
//!   silently shortened.
//! - **Format.** Formats 0 and 1 are read. Format 2 holds independent sequences with no
//!   shared timeline and is refused.
//! - **Notes.** A note is paired per (track, channel, pitch). A note-on with velocity 0 is
//!   a note-off, and a note-off's own velocity is ignored. When the same key is struck
//!   again before it is released, notes close first in, first out: each note-off ends the
//!   earliest note still sounding on that key. Refused, each with its place: a note-off
//!   with nothing sounding, a note still sounding when its track ends, and a note that
//!   ends on the tick it starts.
//! - **Tempo.** Tempo events are taken from every track. On one tick within one track the
//!   last event wins, as SMF plays them in order; on one tick in different tracks they
//!   must agree. With no event at tick 0, the SMF default of
//!   [`DEFAULT_US_PER_QUARTER`] µs per quarter is put there.
//! - **Meter.** The same rules, with 4/4 as the default at tick 0. Only the numerator and
//!   the denominator are kept.
//! - **Everything else** (controllers, program changes, pitch bend, aftertouch, SysEx,
//!   key signatures, text) is not part of the score and is ignored. In particular the
//!   sustain pedal does not lengthen a note: a note lasts from its note-on to its note-off.
//! - **Order.** Notes are sorted into score-model's documented order, and the score must
//!   pass [`IngestedScore::validate`]; its error is passed through unchanged.

#![no_std]

extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use midly::{ErrorKind, Format, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use score_model::{
    DEFAULT_US_PER_QUARTER, IngestedNote, IngestedScore, MeterChange, ModelError, TempoChange,
};

/// The meter assumed at tick 0 when a file states none: 4/4, as SMF defines.
pub const DEFAULT_METER: (u8, u8) = (4, 2);

/// Why a file is not ingested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestError {
    /// The bytes are not a plain SMF (see the crate documentation). `offset` is where the
    /// offending chunk starts: 0 when the bytes do not open with a header.
    NotPlainSmf { offset: usize },
    /// midly found the bytes are not an SMF. The message is midly's.
    Invalid(&'static str),
    /// midly's strict mode found the SMF malformed. The message is midly's.
    Malformed(&'static str),
    /// The file is timed in SMPTE frames, not ticks per quarter note.
    SmpteTiming,
    /// A metrical file with an SMPTE-offset event.
    SmpteOffset { track: u16, tick: u64 },
    /// Format 2: independent sequences.
    SequentialFormat,
    /// More tracks than a `u16` can index.
    TooManyTracks,
    /// A track's ticks do not fit a `u64`.
    TickOverflow { track: u16 },
    /// Two tracks set different tempos on one tick.
    ConflictingTempo { tick: u64 },
    /// Two tracks set different meters on one tick.
    ConflictingMeter { tick: u64 },
    /// A note-off with no note sounding on that key.
    OrphanNoteOff {
        track: u16,
        channel: u8,
        pitch: u8,
        tick: u64,
    },
    /// A note still sounding when its track ends.
    UnterminatedNote {
        track: u16,
        channel: u8,
        pitch: u8,
        start_tick: u64,
    },
    /// A note that ends on the tick it starts.
    ZeroLengthNote {
        track: u16,
        channel: u8,
        pitch: u8,
        tick: u64,
    },
    /// The assembled score breaks a score-model invariant.
    Model(ModelError),
}

/// Reads an SMF into an [`IngestedScore`] in the file's own resolution, or refuses it.
pub fn ingest_smf(bytes: &[u8]) -> Result<IngestedScore, IngestError> {
    let division = plain_smf(bytes)?;
    if division & 0x8000 != 0 {
        return Err(IngestError::SmpteTiming);
    }
    let smf = Smf::parse(bytes).map_err(|e| match e.kind() {
        ErrorKind::Invalid(message) => IngestError::Invalid(message),
        ErrorKind::Malformed(message) => IngestError::Malformed(message),
    })?;
    let source_ppq = match smf.header.timing {
        Timing::Metrical(ppq) => ppq.as_int(),
        Timing::Timecode(..) => return Err(IngestError::SmpteTiming),
    };
    if smf.header.format == Format::Sequential {
        return Err(IngestError::SequentialFormat);
    }

    let mut tempo: BTreeMap<u64, u32> = BTreeMap::new();
    let mut meter: BTreeMap<u64, (u8, u8)> = BTreeMap::new();
    let mut notes: Vec<IngestedNote> = Vec::new();
    for (index, events) in smf.tracks.iter().enumerate() {
        let track = u16::try_from(index).map_err(|_| IngestError::TooManyTracks)?;
        let found = read_track(track, events, &mut notes)?;
        merge(&mut tempo, found.tempo, |tick| {
            IngestError::ConflictingTempo { tick }
        })?;
        merge(&mut meter, found.meter, |tick| {
            IngestError::ConflictingMeter { tick }
        })?;
    }
    tempo.entry(0).or_insert(DEFAULT_US_PER_QUARTER);
    meter.entry(0).or_insert(DEFAULT_METER);
    // The derived order compares every field, so equal notes are identical and an
    // unstable sort is still deterministic.
    notes.sort_unstable();

    let score = IngestedScore {
        source_ppq,
        tempo: tempo
            .into_iter()
            .map(|(tick, us_per_quarter)| TempoChange {
                tick,
                us_per_quarter,
            })
            .collect(),
        meter: meter
            .into_iter()
            .map(|(tick, (numerator, denominator_pow2))| MeterChange {
                tick,
                numerator,
                denominator_pow2,
            })
            .collect(),
        notes,
    };
    score.validate().map_err(IngestError::Model)?;
    Ok(score)
}

/// Makes the layout checks midly does not make, and returns the header's division word.
/// See "Layout" in the crate documentation.
fn plain_smf(bytes: &[u8]) -> Result<u16, IngestError> {
    let header = bytes
        .get(..14)
        .filter(|h| h[..4] == *b"MThd" && h[4..8] == 6u32.to_be_bytes())
        .ok_or(IngestError::NotPlainSmf { offset: 0 })?;
    let division = u16::from_be_bytes([header[12], header[13]]);
    let mut at = 14;
    while let Some(head) = bytes.get(at..at + 8) {
        if head[..4] != *b"MTrk" {
            return Err(IngestError::NotPlainSmf { offset: at });
        }
        let len = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as usize;
        match (at + 8).checked_add(len) {
            Some(end) if end <= bytes.len() => at = end,
            // A chunk that runs past the end: midly's strict parser refuses it.
            _ => break,
        }
    }
    Ok(division)
}

/// What one track says about tempo and meter, already reduced to one value per tick.
struct TrackMaps {
    tempo: BTreeMap<u64, u32>,
    meter: BTreeMap<u64, (u8, u8)>,
}

/// Keys per channel, and so the number of (channel, pitch) slots per track.
const KEYS: usize = 128;
const SLOTS: usize = 16 * KEYS;

fn read_track(
    track: u16,
    events: &[TrackEvent<'_>],
    notes: &mut Vec<IngestedNote>,
) -> Result<TrackMaps, IngestError> {
    let mut maps = TrackMaps {
        tempo: BTreeMap::new(),
        meter: BTreeMap::new(),
    };
    // For each (channel, pitch): the notes sounding on it, oldest first, as (start, velocity).
    let mut sounding: Vec<VecDeque<(u64, u8)>> = (0..SLOTS).map(|_| VecDeque::new()).collect();
    let mut tick: u64 = 0;
    for event in events {
        tick = tick
            .checked_add(u64::from(event.delta.as_int()))
            .ok_or(IngestError::TickOverflow { track })?;
        match event.kind {
            TrackEventKind::Midi { channel, message } => {
                let channel = channel.as_int();
                let (key, velocity) = match message {
                    MidiMessage::NoteOn { key, vel } => (key.as_int(), vel.as_int()),
                    MidiMessage::NoteOff { key, .. } => (key.as_int(), 0),
                    _ => continue,
                };
                let slot = &mut sounding[usize::from(channel) * KEYS + usize::from(key)];
                if velocity > 0 {
                    slot.push_back((tick, velocity));
                    continue;
                }
                let Some((start_tick, velocity)) = slot.pop_front() else {
                    return Err(IngestError::OrphanNoteOff {
                        track,
                        channel,
                        pitch: key,
                        tick,
                    });
                };
                if start_tick == tick {
                    return Err(IngestError::ZeroLengthNote {
                        track,
                        channel,
                        pitch: key,
                        tick,
                    });
                }
                notes.push(IngestedNote {
                    start_tick,
                    pitch: key,
                    track,
                    channel,
                    end_tick: tick,
                    velocity,
                });
            }
            TrackEventKind::Meta(MetaMessage::Tempo(us)) => {
                maps.tempo.insert(tick, us.as_int());
            }
            TrackEventKind::Meta(MetaMessage::TimeSignature(numerator, denominator_pow2, _, _)) => {
                maps.meter.insert(tick, (numerator, denominator_pow2));
            }
            TrackEventKind::Meta(MetaMessage::SmpteOffset(_)) => {
                return Err(IngestError::SmpteOffset { track, tick });
            }
            _ => {}
        }
    }
    // Report the earliest-starting unfinished note; ties go to the lowest (channel, pitch).
    let unfinished = sounding
        .iter()
        .enumerate()
        .filter_map(|(slot, queue)| queue.front().map(|&(start, _)| (start, slot)))
        .min();
    if let Some((start_tick, slot)) = unfinished {
        return Err(IngestError::UnterminatedNote {
            track,
            // A slot is below 16 × 128, so both fit a u8.
            channel: u8::try_from(slot / KEYS).unwrap_or(u8::MAX),
            pitch: u8::try_from(slot % KEYS).unwrap_or(u8::MAX),
            start_tick,
        });
    }
    Ok(maps)
}

/// Adds one track's values to the file's, refusing a tick two tracks disagree on.
fn merge<T: Copy + PartialEq>(
    into: &mut BTreeMap<u64, T>,
    from: BTreeMap<u64, T>,
    conflict: impl Fn(u64) -> IngestError,
) -> Result<(), IngestError> {
    for (tick, value) in from {
        match into.get(&tick) {
            Some(existing) if *existing != value => return Err(conflict(tick)),
            Some(_) => {}
            None => {
                into.insert(tick, value);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
