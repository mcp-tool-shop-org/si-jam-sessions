//! The law of si-jam-sessions.
//!
//! The law holds a score, its tempo map and a played take as integers. It
//! ingests a score only through the licence predicate ([`Law::ingest`]): a
//! receipt and every file it receipts come in, the predicate admits the score
//! or refuses it, and the SMF reader reads it. It steps one quantum at a time
//! whether or not anything is proposed, admits a take note or refuses it with
//! a reason, grades every take note against the score note it cites, and
//! hashes a canonical snapshot of what it holds. It is built as one
//! `wasm32-unknown-unknown` cdylib with a raw C ABI ([`abi`]); the rlib is the
//! same code for native tests and a native harness.
//!
//! Everything the law hashes is an integer. There is no float anywhere in this
//! crate (`tests/no_float.rs` fails if one appears, and Clippy's
//! `float_arithmetic` is denied), so no transcendental function can reach a
//! hash. Arithmetic is `checked_*` and a `None` becomes a [`Refusal`]; the
//! release profile also keeps overflow checks on.
//!
//! The pins below are part of the law version: changing any of them changes
//! the snapshot header, so it moves every hash.
//!
//! The design of record is `docs/PHASE-0.md`.

#![no_std]
// A refusal is a `Result`; dropping one is a build error, not a warning.
#![deny(unused_must_use)]
// Only the C ABI module may use `unsafe`.
#![deny(unsafe_code)]
// No reachable panic and no silent wrap: arithmetic is checked, indexing goes
// through `get`, and conversions are checked `try_from`s.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::float_arithmetic,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used
)]

extern crate alloc;

// std is linked for its wasm32 allocator and panic handler, and for nothing
// else. Bound to `_`, it has no name in this crate, so the law cannot call a
// clock, a file, a thread or the environment: such a path does not compile.
extern crate std as _;

pub mod abi;
mod frames;
mod grade;
#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
mod ingest_tests;
mod law;
mod live;
#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]
mod live_tests;
mod refusal;
mod score;
mod snapshot;
mod take;
mod time;
pub mod wire;

pub use frames::{Beat, FrameNote, Frames, Voice};
pub use grade::{CitedKind, Verdict};
pub use law::{Law, Provenance};
pub use live::{LiveNote, LiveNoteOff};
pub use refusal::{Event, IngestRefusal, Refusal, WireFault};
pub use score::{LawMeter, LawNote, LawScore, LawTempo};
pub use snapshot::{SNAPSHOT_FORMAT, SNAPSHOT_MAGIC};
pub use take::{ScoreNoteId, TakeNote};
pub use time::{TempoMap, rescale_tick};

/// The law version. Every snapshot carries it; any change to what the law
/// computes or hashes bumps it.
///
/// A version number is frozen when it reaches main. Before that it may be
/// refined, but one number never names two different goldens, and every
/// pushed refinement is recorded here.
///
/// - 1: integer time, the take, grading and the snapshot (slice 1's law core).
/// - 2: the ingest verb. The receipt, the licence predicate and the SMF reader
///   became part of the law, and the snapshot records the receipt it admitted
///   and carries the predicate's version and cut-off years in its header. It
///   ran licence predicate version 1 and named the golden `f12b07b0…` on a
///   pushed head; it did not reach `main`.
/// - 3: the same law under licence predicate version 2, with rules year 2026
///   and the cut-offs 1930 (US publication), 1955 (EU death) and 2000
///   (edition). What version 2 admits and refuses is `crates/provenance`'s to
///   state (see `provenance::PREDICATE_VERSION`), and the law does not restate
///   it. Version 2 named golden `f12b07b0…` on a pushed head, so the law with
///   predicate version 2 needed a new number; it names golden `145c7af9…`.
///   The SMF reader's two track-count refusals have codes of their own.
/// - 4: the host's verbs, under the same predicate version 2 and cut-offs.
///   - The frame export ([`Law::frames`], `law_frames`) reads the committed
///     quanta of a window: every note-on of the score, the take and the live
///     take, and every beat. It changes nothing.
///   - The live verbs (`law_live_note` and `law_live_note_off`, [`Law::live`]
///     and [`Law::live_off`]) admit a person's note as a record at its own
///     onset, never refused for lateness. The law cites it by the rule of
///     [`LIVE_REACH_SAMPLES`], each score note at most once, and it goes
///     through the take's own admission and grading.
///   - In a live session, an uncited score note is never played once its
///     reach window has passed the committed horizon (see [`Law::verdicts`]).
///   - `law_horizon` reads the committed horizon, and refusal codes 160 to 172
///     name the new refusals.
///
///   Nothing version 3 computed changes: a take admitted as a batch is graded,
///   rowed and hashed as before, and the constructed take's snapshot differs
///   from version 3's in its law-version word alone. So version 4 names golden
///   `fd574ccc…`, and that snapshot with byte 12 written back to 3 hashes to
///   `145c7af9…`. A host tells the verbs are there from `law_version()`:
///   version 3 on `main` is the law without them.
///
/// The predicate's rules are the law's, and the law pins their version and
/// date cut-offs below. Moving any of them fails the build there until the
/// pin is updated, and by rule the law version with it.
pub const LAW_VERSION: u32 = 4;

// The licence predicate this law version admits scores under. provenance's
// cut-offs move every January (`RULES_YEAR`), and a moved cut-off can change
// which scores are admitted. These asserts pin values only: a moved cut-off
// fails the build here until its pin is updated, and by rule LAW_VERSION moves
// with it, in the same commit. The compiler cannot hold the second half; the
// rule above and the golden, which carries both numbers, do. The snapshot
// header carries the same values, so a changed one also moves every hash.
const _: () = {
    assert!(LAW_VERSION == 4);
    assert!(provenance::PREDICATE_VERSION == 2);
    assert!(provenance::RULES_YEAR == 2026);
    assert!(provenance::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR == 1930);
    assert!(provenance::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR == 1955);
    assert!(provenance::LAST_OUT_OF_TERM_EDITION_YEAR == 2000);
};

/// Ticks per quarter note: 3,360 = 2⁵·3·5·7.
///
/// Every note value from a whole note down to a 128th is a multiple of 105 =
/// 3·5·7 ticks (a 128th is 3,360 / 32 = 105), so dividing any of them into 3,
/// 5 or 7 equal parts gives whole ticks. `time::tests` holds the full table.
pub const PPQ: u32 = 3_360;

/// The law's sample clock, in samples per second.
pub const SAMPLE_RATE: u32 = 48_000;

/// Samples per millisecond at [`SAMPLE_RATE`].
pub const SAMPLES_PER_MS: u32 = 48;

/// **Q**, the law's quantum: 48 samples, one millisecond at 48 kHz.
///
/// The law steps one quantum whether or not anything is proposed, and it takes
/// every admission decision in whole quanta. Q is a constant of the law
/// version, not the audio device's buffer: the host picks its own buffer
/// (WASAPI shared mode chooses its own period), and the law never learns it.
///
/// Why 48:
/// - It is one millisecond, so a lateness reported in quanta reads directly as
///   milliseconds, and every quantum boundary is a whole number of
///   milliseconds and of microseconds.
/// - It is the finest timestamp the host's live MIDI input can deliver (WinMM
///   input has 1 ms resolution, which is 48 samples), so the quantum never
///   coarsens an input onset.
/// - It divides the gate (1,920 = 40 × 48) and the sample rate (48,000 =
///   1,000 × 48), so both are whole quanta.
/// - The cost is 1,000 steps per second of music, each a checked increment.
pub const QUANTUM_SAMPLES: u32 = 48;

/// **H**, the commit horizon, in quanta: 100 quanta, 100 ms.
///
/// While the transport runs, the quantum at the playhead and the H quanta
/// after it are committed. A proposal for a committed quantum is refused with
/// its lateness in quanta, so a committed quantum never changes (PHASE-0
/// finding F29). Before the first step nothing is committed.
///
/// Why 100:
/// - The horizon is the host's render-ahead budget. The host may hand any
///   committed quantum to the audio device, so the horizon must cover the
///   device buffer plus the host's scheduling jitter. The Windows audio
///   engine's default buffer is 10 ms (Microsoft Learn, "Low Latency Audio"),
///   and a browser host that schedules from a timer looks about 100 ms ahead
///   of the audio clock, because its timers drift by tens of milliseconds
///   (Wilson 2013, "A Tale of Two Clocks"). 100 ms covers both hosts. Both
///   sources are in `docs/study-swarm/lanes/lane4-realtime-coperformance.md`,
///   outside the gated dispatch: they inform this choice and pin nothing.
/// - It adds 100 ms to the lead a model needs. That is small against how far
///   ahead a jamming agent plans (four beats in ReaLJam, PHASE-0 F29) and
///   against the round trip that decides whether an accompaniment model keeps
///   up at all (about 100 to 150 ms base latency in StreamMUSE, PHASE-0 F30);
///   the model absorbs its own latency by planning further ahead, not through
///   H.
pub const HORIZON_QUANTA: u32 = 100;

/// The two-sided timing gate: 1,920 samples, 40 ms at 48 kHz.
///
/// A take note is on time when `|delta| <= GATE_SAMPLES`, where `delta` is its
/// onset minus the onset of the score note it cites. The edges are
/// **inclusive**: a delta of ±1,920 is on time and ±1,921 is not.
///
/// Why inclusive: the slice-1 specification writes the gate as
/// `|delta| ≤ 1920`, and the sibling's 40 ms gate, whose width PHASE-0 pins at
/// 1,920 samples, keeps a difference equal to its tolerance: in
/// `ai-jam-sessions`, `src/score-performance.ts` skips a match only when
/// `timeDiff > toleranceSec`, and `src/audio/onsets.ts` accepts
/// `d <= toleranceSec`. The edge therefore classifies here as it did in the
/// sibling's labelled data.
pub const GATE_SAMPLES: u32 = 1_920;

/// How far a live note of a score note's pitch may be from it and still
/// answer it: twice the gate, 3,840 samples (80 ms).
///
/// A person's note does not say which score note it answers, and the host
/// decides nothing, so the live verb decides ([`Law::live`]). This is slice
/// 1's placeholder score follower:
/// - a live note answers the nearest score note of its own pitch up to this
///   far away, and grades match, early or late;
/// - failing that, the nearest score note of any pitch within the gate, and
///   grades wrong pitch;
/// - failing both, it is an addition;
/// - a score note is cited at most once: one already cited is passed over,
///   so a second live note in its reach takes the next candidate or becomes an
///   addition;
/// - ties break by the earlier onset, then the lowest id for the same pitch,
///   and by the nearest pitch, then the lower pitch, then the lowest id for
///   another pitch (`live::cite` states them in full);
/// - same-pitch candidates are taken before other pitches, so every note of a
///   chord played at its own pitch finds its own score note.
///
/// Why twice the gate: the constructed take plays notes up to 60 ms late
/// (2,880 samples, one and a half gates), and a note played that late is still
/// the note it was meant to be, graded late. Past two gates the rule stops
/// guessing and calls the note an addition, which leaves its score note never
/// played; both rows say so in digits. It is derived from the gate rather than
/// pinned on its own, so the snapshot header, which carries the gate, carries
/// it too.
pub const LIVE_REACH_SAMPLES: u32 = 3_840;

/// The largest sample position the law holds: `i64::MAX`. Every onset fits an
/// `i64`, so every difference of two onsets is an exact `i64`. At 48 kHz this
/// is about six million years.
pub const MAX_SAMPLE: u64 = 0x7FFF_FFFF_FFFF_FFFF;

// The arithmetic facts the pins are chosen for. Const evaluation fails the
// build if any of them stops holding, so the operators here cannot wrap.
#[allow(clippy::arithmetic_side_effects, clippy::as_conversions)]
const _: () = {
    assert!(PPQ == 32 * 3 * 5 * 7);
    assert!(PPQ.is_multiple_of(105) && PPQ / 32 == 105);
    assert!(SAMPLE_RATE == SAMPLES_PER_MS * 1_000);
    assert!(GATE_SAMPLES == 40 * SAMPLES_PER_MS);
    assert!(QUANTUM_SAMPLES == SAMPLES_PER_MS);
    assert!(GATE_SAMPLES.is_multiple_of(QUANTUM_SAMPLES));
    assert!(SAMPLE_RATE.is_multiple_of(QUANTUM_SAMPLES));
    assert!(MAX_SAMPLE == i64::MAX as u64);
    assert!(LIVE_REACH_SAMPLES == 2 * GATE_SAMPLES);
};
