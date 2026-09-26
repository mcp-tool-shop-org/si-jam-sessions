//! Why the law refused, as a value.
//!
//! Every check inside the law returns `Result<_, Refusal>`, and the ingest verb
//! returns `Result<_, IngestRefusal>`, which also carries the refusals of the
//! licence predicate and the SMF reader it runs. The C ABI turns a refusal
//! into its code and keeps its text for the host; nothing in the law panics on
//! its way out.

use core::fmt;

use ingest::IngestError;
use provenance::{LicenceRefusal, ReceiptError};
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

/// What was wrong with bytes the host passed in, or with frame bytes a host
/// decodes (see [`crate::wire`]).
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
    /// A container file's name is empty or not UTF-8.
    FileName,
    /// A container file's name is not after the name before it: the names
    /// are out of order, or one is repeated.
    FileOrder,
    /// A frame's voice is not 0 (score), 1 (take) or 2 (live), or a score
    /// frame does not name its own note.
    Voice,
    /// A beat's downbeat flag is not 0 or 1, or is not 1 exactly on beat 0.
    Downbeat,
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
    /// A live note-on or note-off arrived while the transport is stopped:
    /// nothing is committed yet, so there is no clock for it to be a record on.
    LiveStopped,
    /// A live note-on or note-off is before sample 0, where the take starts.
    /// `sample` is the note-on's onset or the note-off's release.
    LiveBeforeStart { sample: i64 },
    /// A live note-on or note-off is in a quantum after the committed horizon.
    /// A live note records what was played, and that quantum has not been
    /// reached.
    LiveAhead {
        sample: u64,
        quantum: u64,
        horizon: u64,
    },
    /// A live note's pitch is above 127.
    LivePitch { pitch: u32 },
    /// A live note's velocity is outside 1..=127.
    LiveVelocity { velocity: u32 },
    /// A live note-off is not after the note-on it ends: the note would last
    /// no samples. The note stays held.
    LiveEmpty { onset_sample: u64, off_sample: u64 },
    /// A live note has the onset, the pitch and the citation of a note
    /// already admitted.
    LiveDuplicate {
        onset_sample: u64,
        pitch: u8,
        cites: Option<u32>,
    },
    /// A live note-off for a pitch no live note is holding.
    LiveNotHeld { pitch: u8 },
    /// A frame window's first quantum is after its last.
    FramesWindow { first: u64, last: u64 },
    /// Frames were asked for while the transport is stopped: no quantum is
    /// committed.
    FramesStopped,
    /// A frame window reaches past the committed horizon.
    FramesNotCommitted { last: u64, horizon: u64 },
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
    /// | 160 | [`Refusal::LiveStopped`] |
    /// | 161 | [`Refusal::LiveBeforeStart`] |
    /// | 162 | [`Refusal::LiveAhead`] |
    /// | 163 | [`Refusal::LivePitch`] |
    /// | 164 | [`Refusal::LiveVelocity`] |
    /// | 165 | [`Refusal::LiveEmpty`] |
    /// | 167 | [`Refusal::LiveDuplicate`] |
    /// | 168 | [`Refusal::LiveNotHeld`] |
    /// | 170 | [`Refusal::FramesWindow`] |
    /// | 171 | [`Refusal::FramesStopped`] |
    /// | 172 | [`Refusal::FramesNotCommitted`] |
    ///
    /// Codes 160 to 172 came with law version 4, the live verbs and the frame
    /// export. They follow every code the ingest verb uses (see
    /// [`IngestRefusal::code`]), so no status names two refusals. 166 is not
    /// used: it named a live note ending past the last sample while the live
    /// verb took a length, before the note-off verb replaced it, and it never
    /// reached `main`.
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
            Refusal::LiveStopped => 160,
            Refusal::LiveBeforeStart { .. } => 161,
            Refusal::LiveAhead { .. } => 162,
            Refusal::LivePitch { .. } => 163,
            Refusal::LiveVelocity { .. } => 164,
            Refusal::LiveEmpty { .. } => 165,
            Refusal::LiveDuplicate { .. } => 167,
            Refusal::LiveNotHeld { .. } => 168,
            Refusal::FramesWindow { .. } => 170,
            Refusal::FramesStopped => 171,
            Refusal::FramesNotCommitted { .. } => 172,
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
                    WireFault::FileName => "a file name is empty or not UTF-8",
                    WireFault::FileOrder => {
                        "a file name is not after the one before it, so the names are out of \
                         order or repeated"
                    }
                    WireFault::Voice => {
                        "a frame's voice is not 0, 1 or 2, or a score frame does not name its note"
                    }
                    WireFault::Downbeat => {
                        "a downbeat flag is not 0 or 1, or is not 1 exactly on beat 0"
                    }
                };
                write!(f, "bytes refused at offset {offset}: {what}")
            }
            Refusal::NullPointer => write!(f, "refused: a null pointer"),
            Refusal::OutOfMemory => write!(f, "refused: an allocation failed"),
            Refusal::Overflow => write!(f, "refused: an integer overflowed"),
            Refusal::Busy => write!(f, "refused: another call into the law is running"),
            Refusal::LiveStopped => write!(
                f,
                "live note refused: the transport is stopped, so no quantum is committed"
            ),
            Refusal::LiveBeforeStart { sample } => write!(
                f,
                "live note refused: sample {sample} is before sample 0, where the take starts"
            ),
            Refusal::LiveAhead {
                sample,
                quantum,
                horizon,
            } => write!(
                f,
                "live note refused: sample {sample} falls in quantum {quantum}, after the \
                 committed horizon {horizon}; a live note records what was played"
            ),
            Refusal::LivePitch { pitch } => {
                write!(f, "live note refused: pitch {pitch} is above 127")
            }
            Refusal::LiveVelocity { velocity } => write!(
                f,
                "live note refused: velocity {velocity} is outside 1..=127"
            ),
            Refusal::LiveEmpty {
                onset_sample,
                off_sample,
            } => write!(
                f,
                "live note-off refused: sample {off_sample} is not after the note-on at sample \
                 {onset_sample} it ends, so the note stays held"
            ),
            Refusal::LiveDuplicate {
                onset_sample,
                pitch,
                cites,
            } => {
                write!(
                    f,
                    "live note refused: a note at onset sample {onset_sample} with pitch {pitch} "
                )?;
                match cites {
                    Some(id) => write!(f, "citing score note {id}")?,
                    None => write!(f, "citing no score note")?,
                }
                write!(f, " is already admitted")
            }
            Refusal::LiveNotHeld { pitch } => write!(
                f,
                "live note-off refused: no live note of pitch {pitch} is held"
            ),
            Refusal::FramesWindow { first, last } => write!(
                f,
                "frames refused: the window's first quantum {first} is after its last {last}"
            ),
            Refusal::FramesStopped => write!(
                f,
                "frames refused: the transport is stopped, so no quantum is committed"
            ),
            Refusal::FramesNotCommitted { last, horizon } => write!(
                f,
                "frames refused: the window ends at quantum {last}, after the committed horizon \
                 {horizon}"
            ),
        }
    }
}

/// Why the ingest verb ([`crate::Law::ingest`]) refused a score.
///
/// The verb runs its layers in order, and the refusal names the layer that
/// stopped it: the container's bytes, then the receipt and the licence
/// predicate, then the count of SMF files on the receipt, then the SMF reader,
/// then the law's own load of the score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IngestRefusal {
    /// The container's bytes do not decode ([`Refusal::Wire`]), or the law's
    /// own load of the ingested score refused it (a [`Refusal`] from
    /// [`crate::LawScore::from_ingested`]).
    Law(Refusal),
    /// The receipt does not load, or the licence predicate refused the score.
    Licence(provenance::Refusal),
    /// The receipt lists this many SMF files; the verb reads exactly one.
    SmfCount { count: usize },
    /// The SMF reader refused the receipt's SMF file.
    Smf(IngestError),
}

impl IngestRefusal {
    /// The status code an export returns for this refusal. A [`Refusal`]
    /// keeps its own code; the other layers have codes of their own, stable
    /// within a law version.
    ///
    /// | Code | Refusal |
    /// |---|---|
    /// | as [`Refusal::code`] | [`IngestRefusal::Law`] |
    /// | 70–85 | [`IngestRefusal::Smf`], one per `IngestError` variant in declaration order |
    /// | 90 | [`IngestRefusal::SmfCount`]: no SMF file |
    /// | 91 | [`IngestRefusal::SmfCount`]: more than one |
    /// | 100 | the receipt does not load, or breaks a structural rule |
    /// | 101–105 | the files: missing, unexpected, supplied twice, size, SHA-256 |
    /// | 110–115 | the composition: no authors, no death year, no first-publication year, unevidenced, not public domain in the US, not in the EU |
    /// | 120–122 | the arrangement: no typesetter, no engraver, a quote not in its evidence |
    /// | 123–129 | the licence: unknown, all rights reserved, no redistribution, share-alike, non-commercial, no derivatives, AI-restricted |
    /// | 130–131 | the credit-ledger id: missing, unexpected |
    /// | 132 | an evidence quote that holds the licence or terms text also negates it |
    /// | 140–144 | the source edition: no publisher, no year, before first publication, scholarly and in term, term not shown |
    /// | 150–154 | the in-file licence: unreadable, misrecorded, disagrees with the page, stated nowhere, stated by an own engraving |
    pub fn code(&self) -> u32 {
        match self {
            IngestRefusal::Law(refusal) => refusal.code(),
            IngestRefusal::Smf(error) => match error {
                IngestError::NotPlainSmf { .. } => 70,
                IngestError::Invalid(_) => 71,
                IngestError::Malformed(_) => 72,
                IngestError::TrackCountMismatch { .. } => 73,
                IngestError::SingleTrackFormat { .. } => 74,
                IngestError::SmpteTiming => 75,
                IngestError::SmpteOffset { .. } => 76,
                IngestError::SequentialFormat => 77,
                IngestError::TooManyTracks => 78,
                IngestError::TickOverflow { .. } => 79,
                IngestError::ConflictingTempo { .. } => 80,
                IngestError::ConflictingMeter { .. } => 81,
                IngestError::OrphanNoteOff { .. } => 82,
                IngestError::UnterminatedNote { .. } => 83,
                IngestError::ZeroLengthNote { .. } => 84,
                IngestError::Model(_) => 85,
            },
            IngestRefusal::SmfCount { count: 0 } => 90,
            IngestRefusal::SmfCount { .. } => 91,
            IngestRefusal::Licence(refusal) => licence_code(refusal),
        }
    }
}

fn licence_code(refusal: &provenance::Refusal) -> u32 {
    use provenance::Refusal as P;
    match refusal {
        P::Receipt(_) => 100,
        P::MissingFile { .. } => 101,
        P::UnexpectedFile { .. } => 102,
        P::DuplicateFile { .. } => 103,
        P::SizeMismatch { .. } => 104,
        P::HashMismatch { .. } => 105,
        P::NoAuthors => 110,
        P::MissingDeathYear { .. } => 111,
        P::MissingFirstPublicationYear => 112,
        P::Unevidenced { .. } => 113,
        P::NotPublicDomainUs { .. } => 114,
        P::NotPublicDomainEu { .. } => 115,
        P::MissingTypesetter => 120,
        P::MissingEngraver => 121,
        P::QuoteNotInEvidence { .. } => 122,
        P::Licence(licence) => match licence {
            LicenceRefusal::Unknown => 123,
            LicenceRefusal::AllRightsReserved => 124,
            LicenceRefusal::NoRedistribution => 125,
            LicenceRefusal::ShareAlike => 126,
            LicenceRefusal::NonCommercial => 127,
            LicenceRefusal::NoDerivatives => 128,
            LicenceRefusal::AiRestricted => 129,
        },
        P::MissingCreditLedgerId => 130,
        P::UnexpectedCreditLedgerId => 131,
        P::QuoteNegated { .. } => 132,
        P::MissingEditionPublisher => 140,
        P::MissingEditionYear => 141,
        P::EditionBeforeFirstPublication { .. } => 142,
        P::ScholarlyEditionInTerm { .. } => 143,
        P::EditionTermNotShown { .. } => 144,
        P::Unreadable { .. } => 150,
        P::InFileMisrecorded { .. } => 151,
        P::InFileLicenceMismatch { .. } => 152,
        P::NoInFileLicence => 153,
        P::OwnEngravingStatesLicence { .. } => 154,
    }
}

fn receipt_reason(f: &mut fmt::Formatter<'_>, error: &ReceiptError) -> fmt::Result {
    match error {
        ReceiptError::Json { offset, problem } => write!(
            f,
            "it is not in the accepted JSON subset at byte {offset} ({problem:?})"
        ),
        ReceiptError::MissingKey { object, key } => write!(f, "{object} has no key \"{key}\""),
        ReceiptError::UnknownKey { object, key } => {
            write!(f, "{object} has the unknown key \"{key}\"")
        }
        ReceiptError::BadValue { object, key } => write!(
            f,
            "{object}'s \"{key}\" has the wrong type, is out of range, or is outside its \
             vocabulary"
        ),
        ReceiptError::UnsupportedSchema(schema) => {
            write!(f, "receipt schema {schema} is not one this law reads")
        }
        ReceiptError::InvalidDate => write!(f, "its fetch date is not a calendar day"),
        ReceiptError::FilesNotSorted => write!(f, "its files are not sorted by name"),
        ReceiptError::EvidenceNotSorted => write!(f, "its evidence is not sorted by id"),
        ReceiptError::RestrictionsNotSorted => write!(f, "its restrictions are not sorted"),
        ReceiptError::BadName => write!(f, "a file or evidence name is not a plain name"),
        ReceiptError::Canonical { offset, problem } => write!(
            f,
            "its canonical bytes are malformed at byte {offset} ({problem:?})"
        ),
    }
}

fn licence_reason(f: &mut fmt::Formatter<'_>, refusal: &provenance::Refusal) -> fmt::Result {
    use provenance::Refusal as P;
    match refusal {
        P::Receipt(error) => {
            write!(f, "the receipt does not load: ")?;
            receipt_reason(f, error)
        }
        P::MissingFile { name } => write!(f, "the receipt lists {name}, which was not supplied"),
        P::UnexpectedFile { name } => write!(f, "{name} was supplied but is not on the receipt"),
        P::DuplicateFile { name } => write!(f, "{name} was supplied twice"),
        P::SizeMismatch { name } => write!(f, "{name}'s size is not the receipt's"),
        P::HashMismatch { name } => write!(f, "{name}'s SHA-256 is not the receipt's"),
        P::NoAuthors => write!(f, "the composition has no authors"),
        P::MissingDeathYear { author } => {
            write!(f, "{author}'s death year is not on the receipt")
        }
        P::MissingFirstPublicationYear => {
            write!(f, "the work's first-publication year is not on the receipt")
        }
        P::Unevidenced { what } => write!(f, "the {what} has no recorded evidence"),
        P::NotPublicDomainUs {
            first_publication_year,
        } => write!(
            f,
            "first published in {first_publication_year}, after {}: not public domain in the \
             United States",
            provenance::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR
        ),
        P::NotPublicDomainEu { author, death_year } => write!(
            f,
            "{author} died in {death_year}, after {}: not public domain in the European Union",
            provenance::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR
        ),
        P::MissingTypesetter => write!(f, "the typesetter is not named"),
        P::MissingEngraver => write!(f, "the engraver is not named"),
        P::QuoteNotInEvidence { what } => write!(
            f,
            "the {what} text is not among its evidence's recorded quotes"
        ),
        P::Licence(licence) => {
            let why = match licence {
                LicenceRefusal::Unknown => "it is unknown, or not one this law version admits",
                LicenceRefusal::AllRightsReserved => "all rights are reserved",
                LicenceRefusal::NoRedistribution => "it forbids redistribution",
                LicenceRefusal::ShareAlike => "it is share-alike",
                LicenceRefusal::NonCommercial => "it is non-commercial",
                LicenceRefusal::NoDerivatives => "it forbids derivatives",
                LicenceRefusal::AiRestricted => "it restricts use by or for AI models",
            };
            write!(f, "the licence refuses the score: {why}")
        }
        P::MissingCreditLedgerId => write!(f, "a CC-BY-4.0 score needs a credit-ledger id"),
        P::UnexpectedCreditLedgerId => {
            write!(f, "a public-domain score carries a credit-ledger id")
        }
        P::QuoteNegated { what } => write!(
            f,
            "an evidence quote that holds the {what} text also negates, limits or conditions it"
        ),
        P::MissingEditionPublisher => {
            write!(f, "the source edition's publisher is not on the receipt")
        }
        P::MissingEditionYear => write!(f, "the source edition's year is not on the receipt"),
        P::EditionBeforeFirstPublication { edition_year } => write!(
            f,
            "the source edition of {edition_year} is dated before the work's first publication"
        ),
        P::ScholarlyEditionInTerm { edition_year } => write!(
            f,
            "the scholarly edition of {edition_year} is still inside its term"
        ),
        P::EditionTermNotShown { edition_year } => write!(
            f,
            "the edition of {edition_year} is too recent for its date to show it is out of term"
        ),
        P::Unreadable { name, why } => {
            write!(f, "{name}'s licence statements cannot be read ({why:?})")
        }
        P::InFileMisrecorded { name } => write!(
            f,
            "the receipt's record of {name}'s licence statements is not what the file says"
        ),
        P::InFileLicenceMismatch { name } => write!(
            f,
            "a licence statement in {name} disagrees with the host page's licence"
        ),
        P::NoInFileLicence => write!(f, "no file states the host page's licence"),
        P::OwnEngravingStatesLicence { name } => write!(
            f,
            "{name}, an engraving by this project, states a licence of its own"
        ),
    }
}

fn smf_reason(f: &mut fmt::Formatter<'_>, error: &IngestError) -> fmt::Result {
    match error {
        IngestError::NotPlainSmf { offset } => write!(
            f,
            "the file is not a plain SMF: the chunk at byte {offset} is not its header or a track"
        ),
        IngestError::Invalid(message) => write!(f, "the file is not an SMF: {message}"),
        IngestError::Malformed(message) => write!(f, "the SMF is malformed: {message}"),
        IngestError::TrackCountMismatch { declared, found } => write!(
            f,
            "the SMF header declares {declared} tracks and the file holds {found}"
        ),
        IngestError::SingleTrackFormat { tracks } => write!(
            f,
            "the SMF is format 0, a single track, and holds {tracks} tracks"
        ),
        IngestError::SmpteTiming => {
            write!(f, "the SMF is timed in SMPTE frames, not ticks per quarter")
        }
        IngestError::SmpteOffset { track, tick } => {
            write!(f, "track {track} has an SMPTE offset at tick {tick}")
        }
        IngestError::SequentialFormat => {
            write!(f, "the SMF is format 2, independent sequences")
        }
        IngestError::TooManyTracks => write!(f, "the SMF has more tracks than a u16 indexes"),
        IngestError::TickOverflow { track } => {
            write!(f, "track {track}'s ticks do not fit a u64")
        }
        IngestError::ConflictingTempo { tick } => {
            write!(f, "two tracks set different tempos at tick {tick}")
        }
        IngestError::ConflictingMeter { tick } => {
            write!(f, "two tracks set different meters at tick {tick}")
        }
        IngestError::OrphanNoteOff {
            track,
            channel,
            pitch,
            tick,
        } => write!(
            f,
            "track {track} channel {channel} releases pitch {pitch} at tick {tick} with no note \
             sounding"
        ),
        IngestError::UnterminatedNote {
            track,
            channel,
            pitch,
            start_tick,
        } => write!(
            f,
            "track {track} channel {channel} pitch {pitch} starts at tick {start_tick} and is \
             still sounding when the track ends"
        ),
        IngestError::ZeroLengthNote {
            track,
            channel,
            pitch,
            tick,
        } => write!(
            f,
            "track {track} channel {channel} pitch {pitch} ends at tick {tick}, the tick it starts"
        ),
        IngestError::Model(error) => model_reason(f, error),
    }
}

impl fmt::Display for IngestRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IngestRefusal::Law(refusal) => write!(f, "{refusal}"),
            IngestRefusal::Licence(refusal) => {
                write!(f, "score refused by the licence predicate: ")?;
                licence_reason(f, refusal)
            }
            IngestRefusal::SmfCount { count: 0 } => {
                write!(f, "score refused: the receipt lists no SMF file")
            }
            IngestRefusal::SmfCount { count } => write!(
                f,
                "score refused: the receipt lists {count} SMF files, and the ingest verb reads one"
            ),
            IngestRefusal::Smf(error) => {
                write!(f, "score refused by the SMF reader: ")?;
                smf_reason(f, error)
            }
        }
    }
}
