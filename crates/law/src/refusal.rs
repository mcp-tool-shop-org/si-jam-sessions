//! Why the law refused, as a value.
//!
//! Every check inside the law returns `Result<_, Refusal>`. The C ABI turns a
//! refusal into its [`Refusal::code`] and keeps its text for the host; nothing
//! in the law panics on its way out.

use core::fmt;

use score_model::{MAX_US_PER_QUARTER, ModelError};

use crate::{MAX_SAMPLE, PPQ};

/// Which event of a score a tick belongs to. The derived order is the order in
/// which a tie between two failing events at the same tick is broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// The tempo change at this index.
    Tempo(usize),
    /// The meter change at this index.
    Meter(usize),
    /// The start of the note at this index.
    NoteStart(usize),
    /// The end of the note at this index.
    NoteEnd(usize),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Tempo(i) => write!(f, "tempo change {i}"),
            Event::Meter(i) => write!(f, "meter change {i}"),
            Event::NoteStart(i) => write!(f, "the start of note {i}"),
            Event::NoteEnd(i) => write!(f, "the end of note {i}"),
        }
    }
}

/// What was wrong with bytes the host passed in (see [`crate::wire`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireFault {
    /// The first four bytes are not the expected magic.
    Magic,
    /// The wire version is not one this law reads.
    Version,
    /// The bytes end before the layout does.
    Truncated,
    /// Bytes remain after the layout ends.
    Trailing,
    /// A take note's citation tag is neither 0 nor 1, or tag 0 carries an id.
    CitationTag,
}

/// Why the law refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The score broke an invariant of `score_model::IngestedScore::validate`.
    Model(ModelError),
    /// More score notes than a [`crate::ScoreNoteId`] (a `u32`) can name.
    TooManyNotes { count: usize },
    /// `tick × 3360 / source_ppq` is not a whole number of law ticks. This is
    /// the earliest such tick of any event in the score.
    InexactTick {
        event: Event,
        tick: u64,
        source_ppq: u16,
    },
    /// `tick × 3360 / source_ppq` is whole but does not fit a `u64`.
    TickOverflow { event: Event, tick: u64 },
    /// The sample position of this law tick does not fit a `u64`.
    SampleOverflow { tick: u64 },
    /// The sample position of this law tick is above [`MAX_SAMPLE`].
    SampleOutOfRange { tick: u64, sample: u64 },
    /// A take or a step arrived before any score was loaded.
    NoScore,
    /// Take note `index` has a velocity outside 1..=127.
    TakeVelocity { index: usize, velocity: u8 },
    /// Take note `index` has a pitch above 127.
    TakePitch { index: usize, pitch: u8 },
    /// Take note `index` cites a score note the score does not have.
    TakeCitation {
        index: usize,
        cites: u32,
        notes: usize,
    },
    /// Take note `index` has an onset above [`MAX_SAMPLE`].
    TakeOnsetOutOfRange { index: usize, onset_sample: u64 },
    /// Take note `index` is not strictly after the take note before it in the
    /// take's total order.
    TakeOrder { index: usize },
    /// Take note `index` is in a committed quantum. `horizon` is the last
    /// committed quantum and `lateness = horizon - quantum + 1`, the number of
    /// quanta by which the proposal missed the first open one.
    Late {
        index: usize,
        quantum: u64,
        horizon: u64,
        lateness: u64,
    },
    /// Take note `index` has the same key as a note already admitted.
    TakeDuplicate { index: usize },
    /// The take would hold more notes than a `u32` can count.
    TakeTooLong { count: usize },
    /// The transport cannot advance past this many steps.
    StepOverflow { steps: u64 },
    /// The host's bytes do not decode.
    Wire { fault: WireFault, offset: usize },
    /// The host passed a null pointer.
    NullPointer,
    /// An allocation failed.
    OutOfMemory,
    /// A checked operation overflowed where no more specific reason applies.
    Overflow,
    /// Another call into the law's C ABI was running.
    Busy,
}

impl Refusal {
    /// The status code an export returns for this refusal. Zero is success and
    /// is never a refusal. The codes are stable within a law version.
    ///
    /// | Code | Refusal |
    /// |---|---|
    /// | 1–14 | [`Refusal::Model`], one per `ModelError` variant in declaration order |
    /// | 20 | [`Refusal::TooManyNotes`] |
    /// | 21 | [`Refusal::InexactTick`] |
    /// | 22 | [`Refusal::TickOverflow`] |
    /// | 23 | [`Refusal::SampleOverflow`] |
    /// | 24 | [`Refusal::SampleOutOfRange`] |
    /// | 30 | [`Refusal::NoScore`] |
    /// | 31 | [`Refusal::TakeVelocity`] |
    /// | 32 | [`Refusal::TakePitch`] |
    /// | 33 | [`Refusal::TakeCitation`] |
    /// | 34 | [`Refusal::TakeOnsetOutOfRange`] |
    /// | 35 | [`Refusal::TakeOrder`] |
    /// | 36 | [`Refusal::Late`] |
    /// | 37 | [`Refusal::TakeDuplicate`] |
    /// | 38 | [`Refusal::TakeTooLong`] |
    /// | 40 | [`Refusal::StepOverflow`] |
    /// | 50 | [`Refusal::Wire`] |
    /// | 60 | [`Refusal::NullPointer`] |
    /// | 61 | [`Refusal::OutOfMemory`] |
    /// | 62 | [`Refusal::Overflow`] |
    /// | 63 | [`Refusal::Busy`] |
    pub fn code(&self) -> u32 {
        match self {
            Refusal::Model(error) => match error {
                ModelError::ZeroPpq => 1,
                ModelError::NoTempoAtZero => 2,
                ModelError::TempoNotSorted { .. } => 3,
                ModelError::TempoOutOfRange { .. } => 4,
                ModelError::NoMeterAtZero => 5,
                ModelError::MeterNotSorted { .. } => 6,
                ModelError::MeterOutOfRange { .. } => 7,
                ModelError::NoNotes => 8,
                ModelError::NotesNotSorted { .. } => 9,
                ModelError::DuplicateNote { .. } => 10,
                ModelError::EmptyNote { .. } => 11,
                ModelError::PitchOutOfRange { .. } => 12,
                ModelError::ChannelOutOfRange { .. } => 13,
                ModelError::VelocityOutOfRange { .. } => 14,
            },
            Refusal::TooManyNotes { .. } => 20,
            Refusal::InexactTick { .. } => 21,
            Refusal::TickOverflow { .. } => 22,
            Refusal::SampleOverflow { .. } => 23,
            Refusal::SampleOutOfRange { .. } => 24,
            Refusal::NoScore => 30,
            Refusal::TakeVelocity { .. } => 31,
            Refusal::TakePitch { .. } => 32,
            Refusal::TakeCitation { .. } => 33,
            Refusal::TakeOnsetOutOfRange { .. } => 34,
            Refusal::TakeOrder { .. } => 35,
            Refusal::Late { .. } => 36,
            Refusal::TakeDuplicate { .. } => 37,
            Refusal::TakeTooLong { .. } => 38,
            Refusal::StepOverflow { .. } => 40,
            Refusal::Wire { .. } => 50,
            Refusal::NullPointer => 60,
            Refusal::OutOfMemory => 61,
            Refusal::Overflow => 62,
            Refusal::Busy => 63,
        }
    }
}

fn model_reason(f: &mut fmt::Formatter<'_>, error: &ModelError) -> fmt::Result {
    match error {
        ModelError::ZeroPpq => write!(f, "the score's source PPQ is 0"),
        ModelError::NoTempoAtZero => write!(f, "the score has no tempo change at tick 0"),
        ModelError::TempoNotSorted { index } => {
            write!(
                f,
                "tempo change {index} is not after the tempo change before it"
            )
        }
        ModelError::TempoOutOfRange { index } => write!(
            f,
            "tempo change {index} is outside 1..={MAX_US_PER_QUARTER} microseconds per quarter"
        ),
        ModelError::NoMeterAtZero => write!(f, "the score has no meter change at tick 0"),
        ModelError::MeterNotSorted { index } => {
            write!(
                f,
                "meter change {index} is not after the meter change before it"
            )
        }
        ModelError::MeterOutOfRange { index } => write!(
            f,
            "meter change {index} has numerator 0 or a denominator above 2^6"
        ),
        ModelError::NoNotes => write!(f, "the score has no notes"),
        ModelError::NotesNotSorted { index } => {
            write!(
                f,
                "note {index} is before the note ahead of it in the canonical order"
            )
        }
        ModelError::DuplicateNote { index } => write!(f, "note {index} repeats the note before it"),
        ModelError::EmptyNote { index } => write!(f, "note {index} does not end after it starts"),
        ModelError::PitchOutOfRange { index } => write!(f, "note {index} has a pitch above 127"),
        ModelError::ChannelOutOfRange { index } => {
            write!(f, "note {index} has a channel above 15")
        }
        ModelError::VelocityOutOfRange { index } => {
            write!(f, "note {index} has a velocity outside 1..=127")
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Model(error) => {
                write!(f, "score refused: ")?;
                model_reason(f, error)
            }
            Refusal::TooManyNotes { count } => write!(
                f,
                "score refused: {count} notes is more than a score note id can name"
            ),
            Refusal::InexactTick {
                event,
                tick,
                source_ppq,
            } => write!(
                f,
                "score refused: tick {tick} of {event} is not a whole law tick: \
                 {tick} x {PPQ} is not divisible by the source PPQ {source_ppq}"
            ),
            Refusal::TickOverflow { event, tick } => write!(
                f,
                "score refused: tick {tick} of {event} is past the last law tick"
            ),
            Refusal::SampleOverflow { tick } => write!(
                f,
                "score refused: law tick {tick} is past the last sample position"
            ),
            Refusal::SampleOutOfRange { tick, sample } => write!(
                f,
                "score refused: law tick {tick} falls at sample {sample}, above the law's last \
                 sample {MAX_SAMPLE}"
            ),
            Refusal::NoScore => write!(f, "refused: no score is loaded"),
            Refusal::TakeVelocity { index, velocity } => write!(
                f,
                "take refused: note {index} has velocity {velocity}, outside 1..=127"
            ),
            Refusal::TakePitch { index, pitch } => {
                write!(f, "take refused: note {index} has pitch {pitch}, above 127")
            }
            Refusal::TakeCitation {
                index,
                cites,
                notes,
            } => write!(
                f,
                "take refused: note {index} cites score note {cites}, but the score has {notes} \
                 notes"
            ),
            Refusal::TakeOnsetOutOfRange {
                index,
                onset_sample,
            } => write!(
                f,
                "take refused: note {index} has onset sample {onset_sample}, above the law's \
                 last sample {MAX_SAMPLE}"
            ),
            Refusal::TakeOrder { index } => write!(
                f,
                "take refused: note {index} is not after the note before it in the order \
                 (onset, pitch, cited note)"
            ),
            Refusal::Late {
                index,
                quantum,
                horizon,
                lateness,
            } => write!(
                f,
                "take refused: note {index} is late by {lateness} quanta: it falls in quantum \
                 {quantum}, and quanta through {horizon} are committed"
            ),
            Refusal::TakeDuplicate { index } => write!(
                f,
                "take refused: note {index} repeats a note already admitted at the same onset, \
                 pitch and cited note"
            ),
            Refusal::TakeTooLong { count } => write!(
                f,
                "take refused: {count} notes is more than the take can count"
            ),
            Refusal::StepOverflow { steps } => write!(
                f,
                "step refused: the transport cannot advance past {steps} steps"
            ),
            Refusal::Wire { fault, offset } => {
                let what = match fault {
                    WireFault::Magic => "the magic is wrong",
                    WireFault::Version => "the wire version is not one this law reads",
                    WireFault::Truncated => "the bytes end early",
                    WireFault::Trailing => "bytes remain after the layout",
                    WireFault::CitationTag => "a citation tag is not 0 or 1, or tag 0 has an id",
                };
                write!(f, "bytes refused at offset {offset}: {what}")
            }
            Refusal::NullPointer => write!(f, "refused: a null pointer"),
            Refusal::OutOfMemory => write!(f, "refused: an allocation failed"),
            Refusal::Overflow => write!(f, "refused: an integer overflowed"),
            Refusal::Busy => write!(f, "refused: another call into the law is running"),
        }
    }
}
