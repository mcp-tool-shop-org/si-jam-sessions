//! Ingest tests. Each synthetic file is built as bytes here, one per rule; the last tests
//! read the committed Entertainer.

use alloc::vec;
use alloc::vec::Vec;

use super::*;

/// An SMF variable-length quantity.
fn vlq(mut n: u32) -> Vec<u8> {
    let mut out = vec![(n & 0x7F) as u8];
    n >>= 7;
    while n > 0 {
        out.insert(0, (n & 0x7F) as u8 | 0x80);
        n >>= 7;
    }
    out
}

/// One track's events, written as bytes.
#[derive(Default)]
struct Track(Vec<u8>);

impl Track {
    fn event(mut self, delta: u32, bytes: &[u8]) -> Self {
        self.0.extend(vlq(delta));
        self.0.extend_from_slice(bytes);
        self
    }
    fn on(self, delta: u32, channel: u8, key: u8, vel: u8) -> Self {
        self.event(delta, &[0x90 | channel, key, vel])
    }
    fn off(self, delta: u32, channel: u8, key: u8) -> Self {
        self.event(delta, &[0x80 | channel, key, 127])
    }
    fn meta(self, delta: u32, kind: u8, data: &[u8]) -> Self {
        let mut bytes = vec![0xFF, kind, u8::try_from(data.len()).unwrap()];
        bytes.extend_from_slice(data);
        self.event(delta, &bytes)
    }
    fn tempo(self, delta: u32, us: u32) -> Self {
        let b = us.to_be_bytes();
        self.meta(delta, 0x51, &[b[1], b[2], b[3]])
    }
    fn meter(self, delta: u32, numerator: u8, denominator_pow2: u8) -> Self {
        self.meta(delta, 0x58, &[numerator, denominator_pow2, 24, 8])
    }
    /// Closes the track with an end-of-track event.
    fn end(self) -> Vec<u8> {
        self.meta(0, 0x2F, &[]).0
    }
}

/// A whole file: header chunk, then one track chunk per track.
fn smf_declaring(format: u16, division: u16, declared: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&format.to_be_bytes());
    out.extend_from_slice(&declared.to_be_bytes());
    out.extend_from_slice(&division.to_be_bytes());
    for t in tracks {
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&u32::try_from(t.len()).unwrap().to_be_bytes());
        out.extend_from_slice(t);
    }
    out
}

fn smf(format: u16, division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let count = u16::try_from(tracks.len()).unwrap();
    smf_declaring(format, division, count, tracks)
}

/// A format-0 file at 96 PPQ holding one track.
fn one(track: Track) -> Vec<u8> {
    smf(0, 96, &[track.end()])
}

fn note(
    start_tick: u64,
    pitch: u8,
    track: u16,
    channel: u8,
    end_tick: u64,
    velocity: u8,
) -> IngestedNote {
    IngestedNote {
        start_tick,
        pitch,
        track,
        channel,
        end_tick,
        velocity,
    }
}

fn tempo(tick: u64, us_per_quarter: u32) -> TempoChange {
    TempoChange {
        tick,
        us_per_quarter,
    }
}

fn meter(tick: u64, numerator: u8, denominator_pow2: u8) -> MeterChange {
    MeterChange {
        tick,
        numerator,
        denominator_pow2,
    }
}

// ---------------------------------------------------------------------------------------
// Pairing.

#[test]
fn a_single_note_takes_the_smf_defaults() {
    let score = ingest_smf(&one(Track::default().on(0, 0, 60, 100).off(96, 0, 60))).unwrap();
    assert_eq!(
        score,
        IngestedScore {
            source_ppq: 96,
            tempo: vec![tempo(0, 500_000)],
            meter: vec![meter(0, 4, 2)],
            notes: vec![note(0, 60, 0, 0, 96, 100)],
        }
    );
}

#[test]
fn a_velocity_zero_note_on_is_a_note_off() {
    let score = ingest_smf(&one(Track::default().on(0, 0, 60, 100).on(96, 0, 60, 0))).unwrap();
    assert_eq!(score.notes, vec![note(0, 60, 0, 0, 96, 100)]);
}

#[test]
fn a_note_offs_own_velocity_is_ignored() {
    // `off` writes velocity 127; the note keeps its note-on velocity.
    let score = ingest_smf(&one(Track::default().on(0, 0, 60, 33).off(96, 0, 60))).unwrap();
    assert_eq!(score.notes, vec![note(0, 60, 0, 0, 96, 33)]);
}

#[test]
fn a_restruck_key_closes_first_in_first_out() {
    let score = ingest_smf(&one(Track::default()
        .on(0, 0, 60, 10)
        .on(48, 0, 60, 20)
        .off(48, 0, 60)
        .off(48, 0, 60)))
    .unwrap();
    assert_eq!(
        score.notes,
        vec![note(0, 60, 0, 0, 96, 10), note(48, 60, 0, 0, 144, 20)]
    );
}

#[test]
fn a_restrike_on_the_release_tick_is_two_notes_in_either_event_order() {
    let off_first = one(Track::default()
        .on(0, 0, 60, 10)
        .off(96, 0, 60)
        .on(0, 0, 60, 20)
        .off(96, 0, 60));
    let on_first = one(Track::default()
        .on(0, 0, 60, 10)
        .on(96, 0, 60, 20)
        .off(0, 0, 60)
        .off(96, 0, 60));
    let expected = vec![note(0, 60, 0, 0, 96, 10), note(96, 60, 0, 0, 192, 20)];
    assert_eq!(ingest_smf(&off_first).unwrap().notes, expected);
    assert_eq!(ingest_smf(&on_first).unwrap().notes, expected);
}

#[test]
fn a_note_off_with_nothing_sounding_is_refused() {
    let file = one(Track::default()
        .on(0, 0, 60, 100)
        .off(96, 0, 60)
        .off(10, 0, 60));
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::OrphanNoteOff {
            track: 0,
            channel: 0,
            pitch: 60,
            tick: 106
        })
    );
}

#[test]
fn a_note_sounding_at_track_end_is_refused() {
    let file = one(Track::default()
        .on(0, 3, 64, 100)
        .on(10, 2, 62, 100)
        .on(10, 1, 60, 100)
        .off(90, 1, 60));
    // Two notes are left; the earlier-starting one is named.
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::UnterminatedNote {
            track: 0,
            channel: 3,
            pitch: 64,
            start_tick: 0
        })
    );
}

#[test]
fn a_note_that_ends_where_it_starts_is_refused() {
    let file = one(Track::default()
        .on(0, 0, 60, 100)
        .on(96, 0, 62, 100)
        .off(0, 0, 62)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::ZeroLengthNote {
            track: 0,
            channel: 0,
            pitch: 62,
            tick: 96
        })
    );
}

#[test]
fn pairing_is_per_channel() {
    let file = one(Track::default()
        .on(0, 0, 60, 100)
        .off(96, 1, 60)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::OrphanNoteOff {
            track: 0,
            channel: 1,
            pitch: 60,
            tick: 96
        })
    );
}

#[test]
fn pairing_is_per_track() {
    let file = smf(
        1,
        96,
        &[
            Track::default().on(0, 0, 60, 100).end(),
            Track::default().off(96, 0, 60).end(),
        ],
    );
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::UnterminatedNote {
            track: 0,
            channel: 0,
            pitch: 60,
            start_tick: 0
        })
    );
}

#[test]
fn running_status_is_read() {
    // Second and third events reuse the note-on status byte.
    let track = Track::default()
        .event(0, &[0x90, 60, 100])
        .event(96, &[60, 0])
        .event(0, &[62, 90])
        .event(96, &[62, 0])
        .end();
    let score = ingest_smf(&smf(0, 96, &[track])).unwrap();
    assert_eq!(
        score.notes,
        vec![note(0, 60, 0, 0, 96, 100), note(96, 62, 0, 0, 192, 90)]
    );
}

// ---------------------------------------------------------------------------------------
// Timing and format.

#[test]
fn smpte_timing_is_refused() {
    // Division 0xE728: 25 frames per second (-25 as i8 in the high byte), 40 ticks each.
    let file = smf(
        0,
        0xE728,
        &[Track::default().on(0, 0, 60, 100).off(40, 0, 60).end()],
    );
    assert_eq!(ingest_smf(&file), Err(IngestError::SmpteTiming));
}

#[test]
fn an_smpte_offset_in_a_metrical_file_is_refused() {
    let file = one(Track::default()
        .meta(0, 0x54, &[0x61, 0, 0, 0, 0])
        .on(0, 0, 60, 100)
        .off(96, 0, 60));
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::SmpteOffset { track: 0, tick: 0 })
    );
}

#[test]
fn format_2_is_refused() {
    let file = smf(
        2,
        96,
        &[Track::default().on(0, 0, 60, 100).off(96, 0, 60).end()],
    );
    assert_eq!(ingest_smf(&file), Err(IngestError::SequentialFormat));
}

#[test]
fn format_1_tracks_share_one_timeline() {
    let file = smf(
        1,
        480,
        &[
            Track::default().tempo(0, 600_000).meter(0, 3, 2).end(),
            Track::default().on(480, 0, 72, 90).off(480, 0, 72).end(),
            Track::default().on(0, 1, 48, 80).off(1440, 1, 48).end(),
        ],
    );
    let score = ingest_smf(&file).unwrap();
    assert_eq!(score.source_ppq, 480);
    assert_eq!(score.tempo, vec![tempo(0, 600_000)]);
    assert_eq!(score.meter, vec![meter(0, 3, 2)]);
    assert_eq!(
        score.notes,
        vec![note(0, 48, 2, 1, 1440, 80), note(480, 72, 1, 0, 960, 90)]
    );
}

// ---------------------------------------------------------------------------------------
// Tempo and meter.

#[test]
fn the_default_tempo_fills_tick_0_only_when_it_is_empty() {
    let stated = one(Track::default()
        .tempo(0, 400_000)
        .on(0, 0, 60, 1)
        .off(96, 0, 60));
    assert_eq!(ingest_smf(&stated).unwrap().tempo, vec![tempo(0, 400_000)]);
    let later = one(Track::default()
        .on(0, 0, 60, 1)
        .tempo(96, 400_000)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&later).unwrap().tempo,
        vec![tempo(0, 500_000), tempo(96, 400_000)]
    );
}

#[test]
fn the_default_meter_fills_tick_0_only_when_it_is_empty() {
    let none = one(Track::default().on(0, 0, 60, 1).off(96, 0, 60));
    assert_eq!(ingest_smf(&none).unwrap().meter, vec![meter(0, 4, 2)]);
    let stated = one(Track::default()
        .meter(0, 3, 2)
        .on(0, 0, 60, 1)
        .meter(288, 6, 3)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&stated).unwrap().meter,
        vec![meter(0, 3, 2), meter(288, 6, 3)]
    );
    let later = one(Track::default()
        .on(0, 0, 60, 1)
        .meter(96, 3, 2)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&later).unwrap().meter,
        vec![meter(0, 4, 2), meter(96, 3, 2)]
    );
}

#[test]
fn on_one_tick_in_one_track_the_last_event_wins() {
    let file = one(Track::default()
        .tempo(0, 400_000)
        .tempo(0, 300_000)
        .meter(0, 2, 2)
        .meter(0, 3, 2)
        .on(0, 0, 60, 1)
        .off(96, 0, 60));
    let score = ingest_smf(&file).unwrap();
    assert_eq!(score.tempo, vec![tempo(0, 300_000)]);
    assert_eq!(score.meter, vec![meter(0, 3, 2)]);
}

#[test]
fn on_one_tick_different_tracks_must_agree() {
    let notes = || Track::default().on(0, 0, 60, 1).off(96, 0, 60).end();
    let agree = smf(
        1,
        96,
        &[
            Track::default().tempo(0, 400_000).end(),
            Track::default().tempo(0, 400_000).end(),
            notes(),
        ],
    );
    assert_eq!(ingest_smf(&agree).unwrap().tempo, vec![tempo(0, 400_000)]);
    let tempo_conflict = smf(
        1,
        96,
        &[
            Track::default().tempo(0, 400_000).end(),
            Track::default().tempo(0, 300_000).end(),
            notes(),
        ],
    );
    assert_eq!(
        ingest_smf(&tempo_conflict),
        Err(IngestError::ConflictingTempo { tick: 0 })
    );
    let meter_conflict = smf(
        1,
        96,
        &[
            Track::default().meter(96, 3, 2).end(),
            Track::default().meter(96, 6, 3).end(),
            notes(),
        ],
    );
    assert_eq!(
        ingest_smf(&meter_conflict),
        Err(IngestError::ConflictingMeter { tick: 96 })
    );
}

// ---------------------------------------------------------------------------------------
// Order, other events, and what score-model refuses.

#[test]
fn notes_come_out_in_score_model_order() {
    let file = smf(
        1,
        96,
        &[
            Track::default()
                .on(0, 5, 67, 1)
                .on(0, 0, 64, 2)
                .off(96, 5, 67)
                .off(0, 0, 64)
                .end(),
            Track::default()
                .on(0, 0, 64, 3)
                .on(0, 0, 60, 4)
                .off(48, 0, 64)
                .off(48, 0, 60)
                .end(),
        ],
    );
    let score = ingest_smf(&file).unwrap();
    // (start, pitch, track, channel, end)
    assert_eq!(
        score.notes,
        vec![
            note(0, 60, 1, 0, 96, 4),
            note(0, 64, 0, 0, 96, 2),
            note(0, 64, 1, 0, 48, 3),
            note(0, 67, 0, 5, 96, 1),
        ]
    );
    assert_eq!(score.validate(), Ok(()));
}

#[test]
fn events_that_are_not_score_are_ignored() {
    let plain = one(Track::default().on(0, 0, 60, 100).off(96, 0, 60));
    let noisy = one(Track::default()
        .meta(0, 0x03, b"Piano")
        .event(0, &[0xC0, 1])
        .event(0, &[0xB0, 64, 127])
        .event(0, &[0xF0, 3, 0x7E, 0x7F, 0xF7])
        .meta(0, 0x59, &[0xFF, 0])
        .on(0, 0, 60, 100)
        .event(10, &[0xE0, 0, 0x50])
        .event(10, &[0xA0, 60, 40])
        .event(10, &[0xD0, 30])
        .meta(0, 0x01, b"text")
        .event(66, &[0xB0, 64, 0])
        .off(0, 0, 60));
    assert_eq!(ingest_smf(&noisy).unwrap(), ingest_smf(&plain).unwrap());
}

#[test]
fn score_model_refusals_pass_through() {
    let empty = one(Track::default().meta(0, 0x03, b"nothing"));
    assert_eq!(
        ingest_smf(&empty),
        Err(IngestError::Model(ModelError::NoNotes))
    );

    let doubled = one(Track::default()
        .on(0, 0, 60, 64)
        .on(0, 0, 60, 64)
        .off(96, 0, 60)
        .off(0, 0, 60));
    assert_eq!(
        ingest_smf(&doubled),
        Err(IngestError::Model(ModelError::DuplicateNote { index: 1 }))
    );

    let zero_tempo = one(Track::default().tempo(0, 0).on(0, 0, 60, 1).off(96, 0, 60));
    assert_eq!(
        ingest_smf(&zero_tempo),
        Err(IngestError::Model(ModelError::TempoOutOfRange { index: 0 }))
    );

    let zero_numerator = one(Track::default()
        .meter(0, 0, 2)
        .on(0, 0, 60, 1)
        .off(96, 0, 60));
    assert_eq!(
        ingest_smf(&zero_numerator),
        Err(IngestError::Model(ModelError::MeterOutOfRange { index: 0 }))
    );

    let zero_ppq = smf(
        0,
        0,
        &[Track::default().on(0, 0, 60, 1).off(96, 0, 60).end()],
    );
    assert_eq!(
        ingest_smf(&zero_ppq),
        Err(IngestError::Model(ModelError::ZeroPpq))
    );
}

// ---------------------------------------------------------------------------------------
// midly's strict mode is live.
//
// Three malformed files, byte for byte as the studio's Rust knowledge base measured them
// (midi-notation-ingest lane, wave 5: compiler oracle on rustc 1.98.1 with midly 0.5.3,
// re-run by an independent verifier). Without `strict` midly parses all three as `Ok`:
// one track holding end-of-track; one track with no events; one track whose MIDI channel
// is masked to 15. That half cannot be re-measured here: Cargo unifies features across
// the build, so every midly in this workspace has `strict`. With `strict` each is
// `Malformed`, an error kind midly emits only when `strict` is on, and ingest passes that
// refusal through. So these results are the feature's fingerprint.

fn hex(text: &str) -> Vec<u8> {
    text.split_ascii_whitespace()
        .map(|h| u8::from_str_radix(h, 16).unwrap())
        .collect()
}

#[test]
fn strict_refuses_a_file_declaring_a_track_it_lacks() {
    // Format 1 declares 2 tracks; 1 is present.
    let file = hex("4D 54 68 64 00 00 00 06 00 01 00 02 00 60 4D 54 72 6B 00 00 00 04 00 FF 2F 00");
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::Malformed(
            "file has a different amount of tracks than declared"
        ))
    );
    // The same bytes declaring 1 track get past midly; only score-model refuses them, for
    // holding no notes.
    let mut declared_one = file.clone();
    declared_one[11] = 1;
    assert_eq!(
        ingest_smf(&declared_one),
        Err(IngestError::Model(ModelError::NoNotes))
    );
}

#[test]
fn strict_refuses_a_truncated_track() {
    // The MTrk declares 0x10 bytes; 2 are present.
    let file = hex("4D 54 68 64 00 00 00 06 00 00 00 01 00 60 4D 54 72 6B 00 00 00 10 00 FF");
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::Malformed("invalid chunk"))
    );
}

#[test]
fn strict_refuses_an_out_of_range_meta_value() {
    // A MIDI-channel meta event (FF 20 01) whose data byte is 0xFF.
    let file =
        hex("4D 54 68 64 00 00 00 06 00 00 00 01 00 60 4D 54 72 6B 00 00 00 05 00 FF 20 01 FF");
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::Malformed("malformed event"))
    );
}

#[test]
fn a_header_midly_cannot_read_is_invalid() {
    // Format 3 does not exist. midly reports the outermost context of its error chain,
    // in debug and in release alike.
    let file = smf(
        3,
        96,
        &[Track::default().on(0, 0, 60, 1).off(96, 0, 60).end()],
    );
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::Invalid("invalid midi header"))
    );
}

// ---------------------------------------------------------------------------------------
// Layout: only a plain SMF reaches midly.

fn plain() -> Vec<u8> {
    one(Track::default().on(0, 0, 60, 1).off(96, 0, 60))
}

#[test]
fn bytes_without_an_smf_header_are_refused() {
    assert_eq!(ingest_smf(b""), Err(IngestError::NotPlainSmf { offset: 0 }));
    assert_eq!(
        ingest_smf(b"BADHEADER"),
        Err(IngestError::NotPlainSmf { offset: 0 })
    );
    // A track chunk first.
    let track_only = Track::default().on(0, 0, 60, 1).off(96, 0, 60).end();
    let mut file = b"MTrk".to_vec();
    file.extend_from_slice(&u32::try_from(track_only.len()).unwrap().to_be_bytes());
    file.extend_from_slice(&track_only);
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::NotPlainSmf { offset: 0 })
    );
}

#[test]
fn a_riff_wrapped_file_is_refused_though_midly_would_unwrap_it() {
    let inner = plain();
    let mut riff = b"RIFF".to_vec();
    riff.extend_from_slice(&u32::try_from(inner.len() + 12).unwrap().to_le_bytes());
    riff.extend_from_slice(b"RMIDdata");
    riff.extend_from_slice(&u32::try_from(inner.len()).unwrap().to_le_bytes());
    riff.extend_from_slice(&inner);
    assert!(Smf::parse(&riff).is_ok(), "midly unwraps RMID");
    assert_eq!(
        ingest_smf(&riff),
        Err(IngestError::NotPlainSmf { offset: 0 })
    );
}

#[test]
fn an_unknown_chunk_is_refused_though_midly_would_skip_it() {
    let mut file = plain();
    let at = file.len();
    file.extend_from_slice(b"XFIH\0\0\0\x02hi");
    assert!(Smf::parse(&file).is_ok(), "midly skips unknown chunks");
    assert_eq!(
        ingest_smf(&file),
        Err(IngestError::NotPlainSmf { offset: at })
    );
}

#[test]
fn a_header_of_another_length_is_refused() {
    let mut long_header = plain();
    long_header[7] = 8;
    long_header.splice(14..14, [0, 0]);
    assert_eq!(
        ingest_smf(&long_header),
        Err(IngestError::NotPlainSmf { offset: 0 })
    );
}

#[test]
fn a_ragged_end_is_left_to_strict_which_refuses_it() {
    // A byte after the last chunk: too short to be a chunk header.
    let mut trailing = plain();
    trailing.push(0);
    assert_eq!(
        ingest_smf(&trailing),
        Err(IngestError::Malformed("invalid chunk"))
    );
    // The last track one byte short of its declared length.
    let mut truncated = plain();
    truncated.pop();
    assert_eq!(
        ingest_smf(&truncated),
        Err(IngestError::Malformed("invalid chunk"))
    );
}

#[test]
fn a_timecode_division_is_refused_before_midly_can_panic_on_it() {
    // midly 0.5.3 computes -(0x80 as i8) for this division, which overflows: with overflow
    // checks on, `Smf::parse` panics. Ingest refuses it from the header instead.
    let file = smf(
        0,
        0x8028,
        &[Track::default().on(0, 0, 60, 1).off(40, 0, 60).end()],
    );
    assert_eq!(ingest_smf(&file), Err(IngestError::SmpteTiming));
}

// ---------------------------------------------------------------------------------------
// The committed Entertainer.

const ENTERTAINER: &[u8] = include_bytes!("../../../scores/entertainer/entertainer.mid");

/// The law's resolution (PHASE-0 item 2).
const LAW_PPQ: u64 = 3360;

#[test]
fn the_entertainer_ingests() {
    let score = ingest_smf(ENTERTAINER).unwrap();
    assert_eq!(score.source_ppq, 384);
    // `\midi { \tempo 4 = 72 }` in the LilyPond source: 60_000_000 / 72, truncated.
    assert_eq!(score.tempo, vec![tempo(0, 833_333)]);
    assert_eq!(score.meter, vec![meter(0, 2, 2)]);
    assert_eq!(score.notes.len(), 2621);
    assert_eq!(
        score
            .notes
            .iter()
            .filter(|n| n.track == 1 && n.channel == 0)
            .count(),
        1394
    );
    assert_eq!(
        score
            .notes
            .iter()
            .filter(|n| n.track == 2 && n.channel == 1)
            .count(),
        1227
    );
    // The opening: D, E, C, A (tied), B, G in octaves, sixteenths at 384 PPQ.
    assert_eq!(
        score.notes[..8],
        [
            note(0, 74, 1, 0, 96, 90),
            note(0, 86, 1, 0, 96, 90),
            note(96, 76, 1, 0, 192, 90),
            note(96, 88, 1, 0, 192, 90),
            note(192, 72, 1, 0, 288, 90),
            note(192, 84, 1, 0, 288, 90),
            note(288, 69, 1, 0, 480, 90),
            note(288, 81, 1, 0, 480, 90),
        ]
    );
    assert_eq!(
        score.notes.last(),
        Some(&note(116_352, 84, 1, 0, 116_544, 90))
    );
    assert!(score.notes.iter().all(|n| n.velocity == 90));
    assert_eq!(score.validate(), Ok(()));
}

#[test]
fn every_entertainer_tick_is_exact_at_ppq_3360() {
    let score = ingest_smf(ENTERTAINER).unwrap();
    let ppq = u64::from(score.source_ppq);
    let ticks = score
        .notes
        .iter()
        .flat_map(|n| [n.start_tick, n.end_tick])
        .chain(score.tempo.iter().map(|t| t.tick))
        .chain(score.meter.iter().map(|m| m.tick));
    let mut gcd = 0u64;
    for tick in ticks {
        assert_eq!(tick * LAW_PPQ % ppq, 0, "tick {tick}");
        gcd = gcd_of(gcd, tick);
    }
    // Every tick is a multiple of a sixteenth (96 at 384 PPQ), which is 840 at 3360.
    assert_eq!(gcd, 96);
    assert_eq!(96 * LAW_PPQ / ppq, 840);
}

fn gcd_of(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd_of(b, a % b) }
}

// ---------------------------------------------------------------------------------------
// Finding 4 of the external review: the two refusals no input has reached.

/// `count` tracks; every one but the last holds only end-of-track, and the last holds one
/// note.
fn many_tracks(declared: u16, count: usize) -> Vec<u8> {
    let empty = Track::default().end();
    let last = Track::default().on(0, 0, 60, 1).off(96, 0, 60).end();
    let mut tracks: Vec<Vec<u8>> = vec![empty; count - 1];
    tracks.push(last);
    smf_declaring(1, 96, declared, &tracks)
}

#[test]
fn the_largest_track_count_a_header_can_declare_fits_a_track_index() {
    // A header declares at most u16::MAX tracks, so the last index is u16::MAX - 1.
    let score = ingest_smf(&many_tracks(u16::MAX, usize::from(u16::MAX))).unwrap();
    assert_eq!(score.notes, vec![note(0, 60, u16::MAX - 1, 0, 96, 1)]);
    // One track more than any header can declare is refused by midly's strict parser
    // before ingest indexes a single track.
    assert_eq!(
        ingest_smf(&many_tracks(u16::MAX, usize::from(u16::MAX) + 1)),
        Err(IngestError::Malformed(
            "file has a different amount of tracks than declared"
        ))
    );
}

#[test]
fn a_track_index_beyond_u16_is_refused_by_name() {
    assert_eq!(track_index(0), Ok(0));
    assert_eq!(track_index(usize::from(u16::MAX)), Ok(u16::MAX));
    assert_eq!(
        track_index(usize::from(u16::MAX) + 1),
        Err(IngestError::TooManyTracks)
    );
}

#[test]
fn a_tick_beyond_u64_is_refused_by_name() {
    assert_eq!(advance(u64::MAX - 5, 5, 3), Ok(u64::MAX));
    assert_eq!(
        advance(u64::MAX - 5, 6, 3),
        Err(IngestError::TickOverflow { track: 3 })
    );
    // The largest delta a file can hold, from the largest tick.
    assert_eq!(
        advance(u64::MAX, (1 << 28) - 1, 7),
        Err(IngestError::TickOverflow { track: 7 })
    );
}
