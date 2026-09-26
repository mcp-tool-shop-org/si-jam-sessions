//! The law: a score, a take, and a transport that steps whether or not
//! anything is proposed.

use alloc::string::String;
use alloc::vec::Vec;

use provenance::{Media, Receipt, Supplied, Tier};
use score_model::IngestedScore;
use sha2::{Digest, Sha256};

use crate::frames::{self, Frames, LiveLength};
use crate::grade::{self, Naming, Verdict};
use crate::live::{self, LiveNote, LiveNoteOff};
use crate::refusal::{IngestRefusal, Refusal};
use crate::score::LawScore;
use crate::snapshot;
use crate::take::{ScoreNoteId, TakeNote};
use crate::wire;
use crate::{CLOSE_SAMPLES, HORIZON_QUANTA, MAX_SAMPLE, QUANTUM_SAMPLES};

/// Where an ingested score came from: the tier the licence predicate admitted
/// it into, and the SHA-256 of its receipt's canonical encoding. The receipt
/// records every file's SHA-256, so the digest also names the files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub tier: Tier,
    pub receipt_digest: [u8; 32],
}

/// The law's whole state.
///
/// # The transport
///
/// `steps` counts the quanta stepped since the score was loaded; the host calls
/// [`Law::step`] once per [`crate::QUANTUM_SAMPLES`] samples of audio time,
/// whether or not anything is proposed.
///
/// - **Stopped** (`steps == 0`): nothing is committed. A whole take (the
///   constructed take, or a recorded take played back) is admitted here,
///   through the same [`Law::admit`] as any proposal.
/// - **Running** (`steps = s >= 1`): the playhead is quantum `s - 1`, and the
///   committed horizon is quantum `s - 1 + H`, so the playhead and the H
///   quanta after it are committed. A proposal for quantum `q <= s - 1 + H` is
///   refused as late by `s + H - q` quanta; the first open quantum is `s + H`.
///
/// A committed quantum never changes for a proposal: every quantum at or before
/// the horizon is refused, and the horizon only moves forward. A live note is
/// not a proposal but a record of what a person played ([`Law::live`]): it is
/// admitted at its own onset, at or before the horizon, and never refused for
/// lateness. The committed frames ([`Law::frames`]) are read out of committed
/// quanta only.
///
/// # Live takes
///
/// The first step decides what the take is ([`TakeKind`]): a take admitted
/// before the transport ran is a proposed take, graded whole as law version 3
/// graded every take; a transport that starts with the take empty makes a live
/// take, graded as it is played. In a live take, each score note closes once
/// the playhead, the first sample of the playhead quantum, has passed its
/// onset plus [`CLOSE_SAMPLES`]; a closed score note's verdict is final, and
/// the verdicts and rows show only final ones ([`Law::verdicts`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Law {
    score: LawScore,
    /// Strictly increasing in [`TakeNote::key`].
    take: Vec<TakeNote>,
    /// The lengths of the take notes the live verb admitted, strictly
    /// increasing in their keys, which are keys of `take`. Not hashed.
    live: Vec<LiveLength>,
    steps: u64,
    kind: TakeKind,
    /// `None` for a score loaded as bytes ([`Law::load`]), which no receipt
    /// admitted; the snapshot says so.
    provenance: Option<Provenance>,
}

/// What a take is, decided when the transport first steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeKind {
    /// No note, and the transport has not run. Read as law version 3 reads an
    /// empty take: every score note never played. Those rows are not
    /// committed: nothing is committed before the first step, and the first
    /// step, which makes the take live, withdraws them.
    Empty,
    /// Its notes were admitted before the transport ran ([`Law::admit`]): the
    /// constructed take, or a recorded take played back. Graded whole, as law
    /// version 3 graded every take, whatever the transport's position; a note
    /// admitted into it later, proposed or live, is graded the same way.
    Proposed,
    /// The transport started with the take empty: a take made while it runs,
    /// of live notes and proposals. Its score notes close from the playhead
    /// ([`CLOSE_SAMPLES`]) and only final verdicts are shown; a live note never
    /// cites a closed score note, and a proposal that does is refused.
    Live,
}

impl Law {
    /// Loads a score and stops the transport: an empty take, zero steps.
    /// See [`LawScore::from_ingested`] for what is refused.
    ///
    /// No receipt comes with the score, so no licence predicate has admitted
    /// it, and the snapshot records it as unreceipted. [`Law::ingest`] is the
    /// verb that admits a score.
    pub fn load(score: &IngestedScore) -> Result<Self, Refusal> {
        Ok(Law {
            score: LawScore::from_ingested(score)?,
            take: Vec::new(),
            live: Vec::new(),
            steps: 0,
            kind: TakeKind::Empty,
            provenance: None,
        })
    }

    /// The ingest verb: a receipt and every file it receipts, in the
    /// [`wire`] container layout, become a loaded score, or a refusal that
    /// names the layer that stopped them. The transport is stopped: an empty
    /// take, zero steps.
    ///
    /// The layers run in this order, and the first refusal is returned:
    /// 1. the container's bytes decode ([`wire::decode_container`]);
    /// 2. the receipt loads from its JSON (`provenance::Receipt::from_json`);
    /// 3. the licence predicate admits the score (`provenance::admit`): every
    ///    receipted file is supplied once with its SHA-256 and size, the
    ///    composition, the arrangement and the source edition pass, and the
    ///    files' own licence statements agree with the host page;
    /// 4. the receipt lists exactly one SMF file;
    /// 5. the SMF reader reads it (`ingest::ingest_smf`);
    /// 6. the law rescales it to PPQ 3360 and places it on the sample clock
    ///    ([`LawScore::from_ingested`]).
    ///
    /// The admitted tier and the receipt's digest are kept, and the snapshot
    /// commits them.
    pub fn ingest(container: &[u8]) -> Result<Self, IngestRefusal> {
        let container = wire::decode_container(container).map_err(IngestRefusal::Law)?;
        let receipt = Receipt::from_json(container.receipt)
            .map_err(|e| IngestRefusal::Licence(provenance::Refusal::Receipt(e)))?;
        let mut supplied = Vec::new();
        supplied
            .try_reserve_exact(container.files.len())
            .map_err(|_| IngestRefusal::Law(Refusal::OutOfMemory))?;
        supplied.extend(container.files.iter().map(|f| Supplied {
            name: f.name,
            bytes: f.bytes,
        }));
        let admitted = provenance::admit(&receipt, &supplied).map_err(IngestRefusal::Licence)?;

        let mut smf_files = receipt.files.iter().filter(|f| f.media == Media::Smf);
        let (Some(smf), None) = (smf_files.next(), smf_files.next()) else {
            let count = receipt
                .files
                .iter()
                .filter(|f| f.media == Media::Smf)
                .count();
            return Err(IngestRefusal::SmfCount { count });
        };
        // The predicate has checked that every receipted file was supplied, so
        // this cannot miss; a miss is refused as the predicate would refuse it.
        let bytes = container.file(&smf.name).ok_or_else(|| {
            IngestRefusal::Licence(provenance::Refusal::MissingFile {
                name: smf.name.clone(),
            })
        })?;
        let score = ingest::ingest_smf(bytes).map_err(IngestRefusal::Smf)?;
        let score = LawScore::from_ingested(&score).map_err(IngestRefusal::Law)?;
        Ok(Law {
            score,
            take: Vec::new(),
            live: Vec::new(),
            steps: 0,
            kind: TakeKind::Empty,
            provenance: Some(Provenance {
                tier: admitted.tier,
                receipt_digest: admitted.receipt_digest,
            }),
        })
    }

    /// Where the score came from: `Some` when [`Law::ingest`] admitted it,
    /// `None` when it was loaded as bytes.
    pub fn provenance(&self) -> Option<&Provenance> {
        self.provenance.as_ref()
    }

    /// The score in law form.
    pub fn score(&self) -> &LawScore {
        &self.score
    }

    /// The admitted take, in key order. A note's index here is its take index.
    pub fn take(&self) -> &[TakeNote] {
        &self.take
    }

    /// Quanta stepped since the score was loaded.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// What the take is, decided when the transport first steps.
    pub fn take_kind(&self) -> TakeKind {
        self.kind
    }

    /// The playhead: the first sample of the playhead quantum, `(steps - 1) *
    /// Q`, or `None` while the transport is stopped. Past `u64::MAX` it reads
    /// `u64::MAX`, which is past every close point.
    pub fn playhead_sample(&self) -> Option<u64> {
        self.steps
            .checked_sub(1)
            .map(|playhead| playhead.saturating_mul(u64::from(QUANTUM_SAMPLES)))
    }

    /// A score note's close point: its onset plus [`CLOSE_SAMPLES`]. In a live
    /// take the note is closed once the playhead has passed it.
    pub fn close_point(&self, note: ScoreNoteId) -> Result<u64, Refusal> {
        let n = self.score.note(note).ok_or(Refusal::TakeCitation {
            index: 0,
            cites: note.0,
            notes: self.score.notes().len(),
        })?;
        n.onset_sample
            .checked_add(u64::from(CLOSE_SAMPLES))
            .ok_or(Refusal::Overflow)
    }

    /// Whether a score note is closed: the take is live and the playhead has
    /// passed the note's close point. Its verdict is then final.
    pub fn is_closed(&self, note: ScoreNoteId) -> Result<bool, Refusal> {
        if self.kind != TakeKind::Live {
            return Ok(false);
        }
        let close = self.close_point(note)?;
        Ok(self.playhead_sample().is_some_and(|at| at > close))
    }

    /// The earliest onset a score note may have and still be open to a new
    /// citation: in a live take, the playhead less [`CLOSE_SAMPLES`]; in a
    /// proposed take, 0.
    fn open_from(&self) -> Result<u64, Refusal> {
        if self.kind == TakeKind::Proposed {
            return Ok(0);
        }
        Ok(self
            .playhead_sample()
            .map_or(0, |at| at.saturating_sub(u64::from(CLOSE_SAMPLES))))
    }

    /// The last committed quantum, or `None` while the transport is stopped.
    pub fn committed_horizon(&self) -> Result<Option<u64>, Refusal> {
        match self.steps.checked_sub(1) {
            None => Ok(None),
            Some(playhead) => playhead
                .checked_add(u64::from(HORIZON_QUANTA))
                .map(Some)
                .ok_or(Refusal::Overflow),
        }
    }

    /// Steps one quantum. Nothing is proposed or needed; the horizon moves
    /// forward by one quantum. A first step with the take empty makes it a
    /// live take ([`TakeKind`]).
    ///
    /// Returns whether the step changed the record, the verdicts and rows:
    /// in a live take, when it made a row final (a score note closed, or an
    /// addition became final); and on the first step that makes the take live,
    /// which withdraws the rows a stopped law with an empty take shows. A step
    /// never changes a proposed take's record.
    pub fn step(&mut self) -> Result<bool, Refusal> {
        let refused = Refusal::StepOverflow { steps: self.steps };
        let next = self.steps.checked_add(1).ok_or(refused)?;
        // After this step the first open quantum is next + H; it must be a
        // u64, so the horizon and every lateness stay representable.
        next.checked_add(u64::from(HORIZON_QUANTA)).ok_or(refused)?;
        self.steps = next;
        Ok(match self.kind {
            TakeKind::Empty => {
                self.kind = TakeKind::Live;
                !self.score.notes().is_empty()
            }
            TakeKind::Live => grade::finalizes_at(&self.score, &self.take, &self.live, next),
            TakeKind::Proposed => false,
        })
    }

    /// Admits `notes` into the take, all of them or none.
    ///
    /// The notes are checked in order, and the refusal names the first note
    /// that fails and the first check it fails:
    /// 1. velocity in 1..=127, pitch at most 127, a citation that names a
    ///    score note, and an onset at most [`MAX_SAMPLE`];
    /// 2. strictly after the note before it in the order
    ///    [`TakeNote::key`] (a total order: no ties);
    /// 3. not in a committed quantum ([`Refusal::Late`], with the lateness);
    /// 4. in a live take, no citation of a closed score note
    ///    ([`Refusal::TakeCitesClosed`]): its verdict is final;
    /// 5. not the key of a note already admitted.
    ///
    /// On success the notes merge into the take, which stays in key order, so
    /// the take does not depend on how its notes were batched. Notes admitted
    /// before the transport has run make a proposed take ([`TakeKind`]).
    pub fn admit(&mut self, notes: &[TakeNote]) -> Result<(), Refusal> {
        let horizon = self.committed_horizon()?;
        self.admit_under(notes, horizon)?;
        if self.kind == TakeKind::Empty && !notes.is_empty() {
            self.kind = TakeKind::Proposed;
        }
        Ok(())
    }

    /// Admission, as [`Law::admit`] states it. The live verb shares it: a
    /// proposal passes the committed horizon, and check 3 refuses it when it
    /// is late; a live note, a record, passes `None`, and check 3 is skipped.
    fn admit_under(&mut self, notes: &[TakeNote], horizon: Option<u64>) -> Result<(), Refusal> {
        let score_notes = self.score.notes().len();
        let mut previous: Option<&TakeNote> = None;
        for (index, note) in notes.iter().enumerate() {
            if note.velocity == 0 || note.velocity > 127 {
                return Err(Refusal::TakeVelocity {
                    index,
                    velocity: note.velocity,
                });
            }
            if note.pitch > 127 {
                return Err(Refusal::TakePitch {
                    index,
                    pitch: note.pitch,
                });
            }
            if let Some(id) = note.cites
                && self.score.note(id).is_none()
            {
                return Err(Refusal::TakeCitation {
                    index,
                    cites: id.0,
                    notes: score_notes,
                });
            }
            if note.onset_sample > MAX_SAMPLE {
                return Err(Refusal::TakeOnsetOutOfRange {
                    index,
                    onset_sample: note.onset_sample,
                });
            }
            if let Some(prev) = previous
                && prev.key() >= note.key()
            {
                return Err(Refusal::TakeOrder { index });
            }
            if let Some(horizon) = horizon {
                let quantum = note.quantum()?;
                if quantum <= horizon {
                    let lateness = horizon
                        .checked_sub(quantum)
                        .and_then(|l| l.checked_add(1))
                        .ok_or(Refusal::Overflow)?;
                    return Err(Refusal::Late {
                        index,
                        quantum,
                        horizon,
                        lateness,
                    });
                }
                if let Some(id) = note.cites
                    && self.is_closed(id)?
                {
                    return Err(Refusal::TakeCitesClosed {
                        index,
                        cites: id.0,
                        close_sample: self.close_point(id)?,
                    });
                }
            }
            if self
                .take
                .binary_search_by(|t| t.key().cmp(&note.key()))
                .is_ok()
            {
                return Err(Refusal::TakeDuplicate { index });
            }
            previous = Some(note);
        }

        let count = self
            .take
            .len()
            .checked_add(notes.len())
            .ok_or(Refusal::Overflow)?;
        if u32::try_from(count).is_err() {
            return Err(Refusal::TakeTooLong { count });
        }
        let mut merged = Vec::new();
        merged
            .try_reserve_exact(count)
            .map_err(|_| Refusal::OutOfMemory)?;
        let mut held = self.take.iter().peekable();
        let mut new = notes.iter().peekable();
        loop {
            let next = match (held.peek(), new.peek()) {
                (Some(a), Some(b)) if a.key() < b.key() => held.next(),
                (Some(_), Some(_)) => new.next(),
                (Some(_), None) => held.next(),
                (None, Some(_)) => new.next(),
                (None, None) => break,
            };
            if let Some(note) = next {
                merged.push(*note);
            }
        }
        self.take = merged;
        Ok(())
    }

    /// A live sample: at or after sample 0, where the take starts, and in a
    /// quantum at or before the committed horizon. A record is of a quantum
    /// already reached, and it is never refused as late.
    fn live_sample(sample: i64, horizon: u64) -> Result<u64, Refusal> {
        let sample = u64::try_from(sample).map_err(|_| Refusal::LiveBeforeStart { sample })?;
        let quantum = sample
            .checked_div(u64::from(crate::QUANTUM_SAMPLES))
            .ok_or(Refusal::Overflow)?;
        if quantum > horizon {
            return Err(Refusal::LiveAhead {
                sample,
                quantum,
                horizon,
            });
        }
        Ok(sample)
    }

    /// Which score notes a take note already cites, by score note id.
    fn cited(&self) -> Result<Vec<bool>, Refusal> {
        let mut cited = Vec::new();
        cited
            .try_reserve_exact(self.score.notes().len())
            .map_err(|_| Refusal::OutOfMemory)?;
        cited.resize(self.score.notes().len(), false);
        for t in &self.take {
            if let Some(slot) = t
                .cites
                .and_then(|id| usize::try_from(id.0).ok())
                .and_then(|i| cited.get_mut(i))
            {
                *slot = true;
            }
        }
        Ok(cited)
    }

    /// The live verb: a note-on a person played, admitted into the take as a
    /// record at its own onset. The note is held until [`Law::live_off`] ends
    /// it.
    ///
    /// The checks run in this order, and the first that fails is the refusal:
    /// 1. the transport runs ([`Refusal::LiveStopped`]);
    /// 2. the pitch is at most 127 and the velocity is in 1..=127;
    /// 3. the onset is at or after sample 0, where the take starts;
    /// 4. the onset's quantum is at or before the committed horizon: a record
    ///    is of a quantum already reached, and it is never refused as late;
    /// 5. no admitted note has its onset, its pitch and its citation.
    ///
    /// The law decides what the note cites, by the rule in `live::cite`: the
    /// host decides nothing, a score note is cited at most once, and in a live
    /// take a closed score note is not cited: a note delivered after the note
    /// it answers has closed cites the next open candidate, or is an addition.
    /// The note then goes through the take's admission without the horizon
    /// check and is graded with the take, so a note played live gets the
    /// verdict and the row it would get admitted as a take. Its length, set by
    /// the note-off, is kept for the committed frames and is not hashed: the
    /// snapshot holds the take as a take.
    ///
    /// Returns the take note as admitted, with its citation.
    pub fn live(&mut self, note: LiveNote) -> Result<TakeNote, Refusal> {
        let horizon = self.committed_horizon()?.ok_or(Refusal::LiveStopped)?;
        let pitch = u8::try_from(note.pitch)
            .ok()
            .filter(|&p| p <= 127)
            .ok_or(Refusal::LivePitch { pitch: note.pitch })?;
        let velocity = u8::try_from(note.velocity)
            .ok()
            .filter(|&v| (1..=127).contains(&v))
            .ok_or(Refusal::LiveVelocity {
                velocity: note.velocity,
            })?;
        let onset_sample = Law::live_sample(note.onset_sample, horizon)?;

        let admitted = TakeNote {
            onset_sample,
            pitch,
            velocity,
            cites: live::cite(
                &self.score,
                &self.cited()?,
                self.open_from()?,
                onset_sample,
                pitch,
            )?,
        };
        // Room for the length first, so nothing can fail once the note is in
        // the take.
        self.live.try_reserve(1).map_err(|_| Refusal::OutOfMemory)?;
        self.admit_under(&[admitted], None)
            .map_err(|refusal| match refusal {
                Refusal::TakeDuplicate { .. } => Refusal::LiveDuplicate {
                    onset_sample,
                    pitch,
                    cites: admitted.cites.map(|id| id.0),
                },
                other => other,
            })?;
        let key = admitted.key();
        let at = self.live.partition_point(|l| l.key < key);
        self.live.insert(
            at,
            LiveLength {
                key,
                duration_samples: None,
                admitted_at: self.steps,
            },
        );
        Ok(admitted)
    }

    /// The live note-off: ends the earliest held live note of the pitch, the
    /// first in the take's order (onset, pitch, citation), whose length
    /// becomes `off_sample` minus its onset. Two notes of one pitch held at
    /// once end in the order they began, as keys released in the order they
    /// were pressed do.
    ///
    /// The checks run in this order:
    /// 1. the transport runs ([`Refusal::LiveStopped`]);
    /// 2. the pitch is at most 127;
    /// 3. the release is at or after sample 0 and in a quantum at or before the
    ///    committed horizon, as a note-on's onset is;
    /// 4. a live note of the pitch is held ([`Refusal::LiveNotHeld`]);
    /// 5. the release is after that note's onset ([`Refusal::LiveEmpty`]; the
    ///    note stays held).
    ///
    /// A length is not graded and not hashed; it goes to the committed frames.
    /// Returns the take note that was ended.
    pub fn live_off(&mut self, off: LiveNoteOff) -> Result<TakeNote, Refusal> {
        let horizon = self.committed_horizon()?.ok_or(Refusal::LiveStopped)?;
        let pitch = u8::try_from(off.pitch)
            .ok()
            .filter(|&p| p <= 127)
            .ok_or(Refusal::LivePitch { pitch: off.pitch })?;
        let off_sample = Law::live_sample(off.off_sample, horizon)?;
        let (at, key) = self
            .live
            .iter()
            .enumerate()
            .filter(|(_, l)| l.key.1 == pitch && l.duration_samples.is_none())
            .min_by_key(|(_, l)| l.key)
            .map(|(at, l)| (at, l.key))
            .ok_or(Refusal::LiveNotHeld { pitch })?;
        let onset_sample = key.0;
        let length = off_sample
            .checked_sub(onset_sample)
            .filter(|&n| n > 0)
            .ok_or(Refusal::LiveEmpty {
                onset_sample,
                off_sample,
            })?;
        // Every check before the write: a refusal leaves the note held.
        let ended = self
            .take
            .binary_search_by(|t| t.key().cmp(&key))
            .ok()
            .and_then(|i| self.take.get(i))
            .copied()
            .ok_or(Refusal::Overflow)?;
        let held = self.live.get_mut(at).ok_or(Refusal::Overflow)?;
        held.duration_samples = Some(length);
        Ok(ended)
    }

    /// The committed frames of the quanta `first..=last`: every note-on of the
    /// score, the take and the live take, and every beat, whose onset falls in
    /// one of them (see [`Frames`]).
    ///
    /// Refused while the transport is stopped, for a window whose first
    /// quantum is after its last, and for a window that reaches past the
    /// committed horizon: only committed quanta are read. Reading changes
    /// nothing.
    pub fn frames(&self, first: u64, last: u64) -> Result<Frames, Refusal> {
        let horizon = self.committed_horizon()?.ok_or(Refusal::FramesStopped)?;
        if first > last {
            return Err(Refusal::FramesWindow { first, last });
        }
        if last > horizon {
            return Err(Refusal::FramesNotCommitted { last, horizon });
        }
        frames::collect(&self.score, &self.take, &self.live, first, last)
    }

    /// The graded record.
    ///
    /// **A proposed take** (the constructed take among them) is graded as law
    /// version 3 graded every take: each score note in id order with the take
    /// notes that cite it or never played, then the additions, whatever the
    /// transport's position.
    ///
    /// **A live take** shows only verdicts that can no longer change, in the
    /// order they became final. A score note's verdicts, its citations or
    /// never played, are final once it closes: when the playhead passes its
    /// onset plus [`CLOSE_SAMPLES`], the reach and then the host's delivery
    /// allowance. An addition is final once the playhead passes its own onset
    /// plus the same, or, delivered later than that, from the next step. A
    /// score note still open has no verdict yet. So the verdicts after any
    /// step begin with the verdicts after every earlier step, unchanged: a row
    /// once shown is a printout of what the law committed, and it stays as it
    /// was shown.
    ///
    /// What stays is the row's text and the verdict's kind, score note, onset,
    /// difference and pitches. A verdict's `take` field is the take note's
    /// index in the take's key order, and a live note delivered past its
    /// allowance, with an onset before notes already shown, moves the indices
    /// after it; a live take's rows name an addition by onset and pitch, never
    /// by index, so no row moves with them.
    pub fn verdicts(&self) -> Result<Vec<Verdict>, Refusal> {
        match self.kind {
            TakeKind::Live => {
                grade::final_verdicts(&self.score, &self.take, &self.live, self.steps)
            }
            TakeKind::Empty | TakeKind::Proposed => grade::verdicts(&self.score, &self.take),
        }
    }

    /// How the rows name an addition's take note: by index in a proposed take,
    /// as version 3 does; by onset and pitch in a live take, whose indices a
    /// late live note can move.
    fn naming(&self) -> Naming {
        match self.kind {
            TakeKind::Live => Naming::Onset,
            TakeKind::Empty | TakeKind::Proposed => Naming::Index,
        }
    }

    /// One row per verdict, in verdict order, each stating its comparison in
    /// digits.
    pub fn rows(&self) -> Result<Vec<String>, Refusal> {
        grade::rows(&self.verdicts()?, self.naming())
    }

    /// The canonical snapshot: the pins, where the score came from, the score
    /// in law ticks with its sample positions, the tempo map, the take, the
    /// verdicts and the rows, as explicit little-endian bytes in a declared
    /// order (see [`crate::SNAPSHOT_FORMAT`]).
    ///
    /// The transport position is not in it: for a proposed take, the snapshot
    /// is the graded record, the same however many quanta the host stepped to
    /// reach it. For a live take the position decides one thing only, which
    /// verdicts are final and so held ([`Law::verdicts`]).
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Refusal> {
        let verdicts = self.verdicts()?;
        let rows = grade::rows(&verdicts, self.naming())?;
        snapshot::encode(
            &self.score,
            self.provenance.as_ref(),
            &self.take,
            &verdicts,
            &rows,
        )
    }

    /// SHA-256 of [`Law::snapshot_bytes`].
    pub fn hash(&self) -> Result<[u8; 32], Refusal> {
        Ok(digest(&self.snapshot_bytes()?))
    }
}

/// SHA-256, the one digest the law computes; the C ABI hashes through this
/// same function.
pub(crate) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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
    use crate::score::tests::{ingested, note};
    use crate::take::ScoreNoteId;
    use crate::{LiveNote, LiveNoteOff, QUANTUM_SAMPLES, SNAPSHOT_FORMAT, SNAPSHOT_MAGIC};
    use alloc::vec;
    use score_model::{IngestedScore, MeterChange, TempoChange};

    const H: u64 = HORIZON_QUANTA as u64;
    const Q: u64 = QUANTUM_SAMPLES as u64;

    fn law() -> Law {
        Law::load(&ingested()).unwrap()
    }

    fn at(onset_sample: u64, pitch: u8, cites: Option<u32>) -> TakeNote {
        TakeNote {
            onset_sample,
            pitch,
            velocity: 64,
            cites: cites.map(ScoreNoteId),
        }
    }

    // --- Admission -----------------------------------------------------------

    #[test]
    fn velocity_edges() {
        let mut l = law();
        for v in [1, 127] {
            let mut n = at(u64::from(v) * 1_000, 60, Some(0));
            n.velocity = v;
            assert_eq!(l.admit(&[n]), Ok(()), "velocity {v}");
        }
        for v in [0, 128, 255] {
            let mut n = at(500, 60, Some(0));
            n.velocity = v;
            assert_eq!(
                l.admit(&[n]),
                Err(Refusal::TakeVelocity {
                    index: 0,
                    velocity: v
                })
            );
        }
    }

    #[test]
    fn pitch_citation_and_range() {
        let mut l = law();
        assert_eq!(
            l.admit(&[at(0, 128, None)]),
            Err(Refusal::TakePitch {
                index: 0,
                pitch: 128
            })
        );
        assert_eq!(
            l.admit(&[at(0, 60, Some(3)), at(1, 60, Some(4))]),
            Err(Refusal::TakeCitation {
                index: 1,
                cites: 4,
                notes: 4
            })
        );
        assert_eq!(
            l.admit(&[at(MAX_SAMPLE + 1, 60, None)]),
            Err(Refusal::TakeOnsetOutOfRange {
                index: 0,
                onset_sample: MAX_SAMPLE + 1
            })
        );
        assert_eq!(l.admit(&[at(MAX_SAMPLE, 127, None)]), Ok(()));
        assert!(l.take().len() == 1);
    }

    #[test]
    fn a_take_is_a_total_order() {
        let mut l = law();
        // Out of onset order.
        assert_eq!(
            l.admit(&[at(10, 60, Some(0)), at(9, 60, Some(0))]),
            Err(Refusal::TakeOrder { index: 1 })
        );
        // The same key twice.
        assert_eq!(
            l.admit(&[at(10, 60, Some(0)), at(10, 60, Some(0))]),
            Err(Refusal::TakeOrder { index: 1 })
        );
        // Same onset: pitch orders; same pitch: an addition before a
        // citation, then citations by id.
        assert_eq!(
            l.admit(&[
                at(10, 60, None),
                at(10, 60, Some(0)),
                at(10, 60, Some(1)),
                at(10, 61, None),
            ]),
            Ok(())
        );
        // Velocity is not in the key: the same key at another velocity is a
        // duplicate of an admitted note.
        let mut again = at(10, 60, Some(1));
        again.velocity = 100;
        assert_eq!(
            l.admit(&[at(5, 60, None), again]),
            Err(Refusal::TakeDuplicate { index: 1 })
        );
        assert_eq!(l.take().len(), 4, "a refused batch changes nothing");
    }

    #[test]
    fn batches_merge_into_one_canonical_take() {
        let notes = [
            at(0, 60, Some(0)),
            at(0, 64, Some(1)),
            at(24_000, 62, Some(2)),
            at(30_000, 70, None),
            at(48_000, 65, Some(3)),
        ];
        let mut whole = law();
        whole.admit(&notes).unwrap();
        let mut split = law();
        split.admit(&[notes[3], notes[4]]).unwrap();
        split.admit(&[notes[0], notes[2]]).unwrap();
        split.admit(&[notes[1]]).unwrap();
        assert_eq!(split.take(), whole.take());
        assert_eq!(split.snapshot_bytes(), whole.snapshot_bytes());
    }

    // --- The transport and the horizon ---------------------------------------

    #[test]
    fn a_stopped_transport_commits_nothing() {
        let mut l = law();
        assert_eq!(l.committed_horizon(), Ok(None));
        // Onset 0, quantum 0: admissible before the first step.
        assert_eq!(l.admit(&[at(0, 60, Some(0))]), Ok(()));
    }

    #[test]
    fn a_proposal_at_or_before_the_horizon_is_refused_with_its_lateness() {
        let mut l = law();
        for _ in 0..10 {
            l.step().unwrap();
        }
        assert_eq!(l.steps(), 10);
        // Playhead 9; committed through 9 + H.
        let horizon = 9 + H;
        assert_eq!(l.committed_horizon(), Ok(Some(horizon)));

        // The last sample of the horizon quantum: late by 1.
        let last = (horizon + 1) * Q - 1;
        assert_eq!(
            l.admit(&[at(last, 60, None)]),
            Err(Refusal::Late {
                index: 0,
                quantum: horizon,
                horizon,
                lateness: 1
            })
        );
        // The first sample of the first open quantum: admitted.
        assert_eq!(l.admit(&[at(last + 1, 60, None)]), Ok(()));
        // Onset 0, the first quantum: late by the horizon plus one.
        assert_eq!(
            l.admit(&[at(0, 60, Some(0))]),
            Err(Refusal::Late {
                index: 0,
                quantum: 0,
                horizon,
                lateness: horizon + 1
            })
        );
        // A quantum 37 before the horizon, in the second note of a batch: the
        // whole batch is refused and the take is unchanged.
        let q = horizon - 37;
        assert_eq!(
            l.admit(&[at((horizon + 5) * Q, 61, None), at(q * Q + 7, 62, None)]),
            Err(Refusal::TakeOrder { index: 1 })
        );
        assert_eq!(
            l.admit(&[at(q * Q + 7, 62, None), at((horizon + 5) * Q, 61, None)]),
            Err(Refusal::Late {
                index: 0,
                quantum: q,
                horizon,
                lateness: 38
            })
        );
        assert_eq!(l.take().len(), 1);

        // One more step moves the horizon by one quantum.
        l.step().unwrap();
        assert_eq!(l.committed_horizon(), Ok(Some(horizon + 1)));
        assert_eq!(
            l.admit(&[at(last + 1, 63, None)]),
            Err(Refusal::Late {
                index: 0,
                quantum: horizon + 1,
                horizon: horizon + 1,
                lateness: 1
            })
        );
    }

    #[test]
    fn the_first_step_commits_the_playhead_and_h_quanta() {
        let mut l = law();
        l.step().unwrap();
        assert_eq!(l.committed_horizon(), Ok(Some(H)));
        assert!(matches!(
            l.admit(&[at(H * Q, 60, None)]),
            Err(Refusal::Late { lateness: 1, .. })
        ));
        assert_eq!(l.admit(&[at((H + 1) * Q, 60, None)]), Ok(()));
    }

    #[test]
    fn the_transport_refuses_to_step_past_its_last_quantum() {
        let mut l = law();
        l.steps = u64::MAX - H - 1;
        assert_eq!(
            l.step(),
            Ok(true),
            "the first step makes the empty take live"
        );
        assert_eq!(l.committed_horizon(), Ok(Some(u64::MAX - 1)));
        assert_eq!(
            l.step(),
            Err(Refusal::StepOverflow {
                steps: u64::MAX - H
            })
        );
        assert_eq!(l.steps(), u64::MAX - H, "a refused step does not move");
    }

    /// A note-off checks everything before it writes: when the take note of
    /// the held note cannot be found (no path reaches this today), the refusal
    /// leaves the note held, as every refused note-off does.
    #[test]
    fn a_refused_note_off_writes_nothing() {
        let mut l = law();
        l.step().unwrap();
        l.live(LiveNote {
            onset_sample: 100,
            pitch: 60,
            velocity: 64,
        })
        .unwrap();
        // Break the invariant by hand: the take no longer holds the live note.
        l.take.clear();
        assert_eq!(
            l.live_off(LiveNoteOff {
                pitch: 60,
                off_sample: 200
            }),
            Err(Refusal::Overflow)
        );
        assert_eq!(l.live[0].duration_samples, None, "still held");
    }

    /// A live note can sound to the law's last sample. A note-off's release is
    /// an `i64` on the law's clock, so it cannot pass [`MAX_SAMPLE`], and the
    /// last sample itself ends a note; the frames carry its length.
    #[test]
    fn a_live_note_can_end_on_the_last_sample() {
        let mut l = law();
        l.steps = u64::MAX - H - 1;
        let onset = MAX_SAMPLE - 10;
        assert!(l.committed_horizon().unwrap().unwrap() >= onset / Q);
        let on = l
            .live(LiveNote {
                onset_sample: onset as i64,
                pitch: 60,
                velocity: 64,
            })
            .unwrap();
        assert_eq!(MAX_SAMPLE, i64::MAX as u64);
        assert_eq!(
            l.live_off(LiveNoteOff {
                pitch: 60,
                off_sample: i64::MAX
            }),
            Ok(on)
        );
        let q = onset / Q;
        let frames = l.frames(q, q).unwrap();
        let live: vec::Vec<u64> = frames
            .notes
            .iter()
            .filter(|n| n.voice == crate::Voice::Live)
            .map(|n| n.duration_samples)
            .collect();
        assert_eq!(live, [10]);
    }

    #[test]
    fn stepping_does_not_touch_the_record() {
        let mut l = law();
        l.admit(&[at(0, 60, Some(0))]).unwrap();
        let before = l.hash().unwrap();
        for _ in 0..1_000 {
            l.step().unwrap();
        }
        assert_eq!(l.hash().unwrap(), before);
    }

    // --- Snapshot and hash ---------------------------------------------------

    #[test]
    fn sha256_is_sha256() {
        // FIPS 180-2 test vector for "abc".
        let digest: [u8; 32] = Sha256::digest(b"abc").into();
        assert_eq!(
            digest,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
        let l = law();
        let bytes = l.snapshot_bytes().unwrap();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(l.hash().unwrap(), expected);
    }

    #[test]
    fn the_snapshot_header_is_the_pins_in_little_endian() {
        let bytes = law().snapshot_bytes().unwrap();
        let mut header = vec::Vec::new();
        header.extend_from_slice(&SNAPSHOT_MAGIC);
        for word in [
            SNAPSHOT_FORMAT,
            crate::LAW_VERSION,
            crate::PPQ,
            crate::SAMPLE_RATE,
            QUANTUM_SAMPLES,
            HORIZON_QUANTA,
            crate::GATE_SAMPLES,
            provenance::PREDICATE_VERSION,
            u32::from(provenance::RULES_YEAR),
            u32::from(provenance::US_LAST_PUBLIC_DOMAIN_PUBLICATION_YEAR),
            u32::from(provenance::EU_LAST_PUBLIC_DOMAIN_DEATH_YEAR),
            u32::from(provenance::LAST_OUT_OF_TERM_EDITION_YEAR),
        ] {
            header.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(header.len(), 56);
        assert_eq!(&bytes[..header.len()], header.as_slice());
        assert_eq!(&bytes[8..12], &[2, 0, 0, 0], "snapshot format 2");
        assert_eq!(&bytes[12..16], &[5, 0, 0, 0], "law version 5");
        assert_eq!(&bytes[16..20], &[0x20, 0x0D, 0, 0], "3360 little-endian");
        assert_eq!(&bytes[20..24], &[0x80, 0xBB, 0, 0], "48000 little-endian");
        assert_eq!(&bytes[36..40], &[3, 0, 0, 0], "predicate version 3");
        assert_eq!(&bytes[40..44], &[0xEA, 0x07, 0, 0], "rules year 2026");
        assert_eq!(&bytes[44..48], &[0x8A, 0x07, 0, 0], "US cut-off 1930");
        assert_eq!(&bytes[48..52], &[0xA3, 0x07, 0, 0], "EU cut-off 1955");
        assert_eq!(&bytes[52..56], &[0xD0, 0x07, 0, 0], "edition cut-off 2000");
        assert_eq!(&bytes[56..60], b"PROV");
        assert_eq!(
            &bytes[60..64],
            &[0, 0, 0, 0],
            "a score loaded as bytes has no receipt"
        );
        assert_eq!(&bytes[64..68], b"TMPO");
    }

    #[test]
    fn the_snapshot_is_deterministic() {
        let take = [at(0, 60, Some(0)), at(24_300, 62, Some(2)), at(1, 99, None)];
        let mut sorted = take;
        sorted.sort_by_key(TakeNote::key);
        let build = || {
            let mut l = law();
            l.admit(&sorted).unwrap();
            l
        };
        assert_eq!(build().snapshot_bytes(), build().snapshot_bytes());
        assert_eq!(build().hash(), build().hash());
    }

    /// A score and a take for the change tests: PPQ 3360, two tempo changes,
    /// two meter changes, five notes, and a take that plays each note 100 ×
    /// (i + 1) samples late.
    fn fixture() -> (IngestedScore, vec::Vec<TakeNote>) {
        let score = IngestedScore {
            source_ppq: 3_360,
            tempo: vec![
                TempoChange {
                    tick: 0,
                    us_per_quarter: 500_000,
                },
                TempoChange {
                    tick: 6_720,
                    us_per_quarter: 600_000,
                },
            ],
            meter: vec![
                MeterChange {
                    tick: 0,
                    numerator: 4,
                    denominator_pow2: 2,
                },
                MeterChange {
                    tick: 13_440,
                    numerator: 3,
                    denominator_pow2: 2,
                },
            ],
            notes: vec![
                note(0, 60, 1_680),
                note(3_360, 62, 5_040),
                note(6_720, 64, 8_400),
                note(10_080, 65, 11_760),
                note(13_440, 67, 16_800),
            ],
        };
        let law = Law::load(&score).unwrap();
        let take = law
            .score()
            .notes()
            .iter()
            .enumerate()
            .map(|(i, n)| {
                at(
                    n.onset_sample + 100 * (i as u64 + 1),
                    n.pitch,
                    Some(i as u32),
                )
            })
            .collect();
        (score, take)
    }

    fn hash_of(score: &IngestedScore, take: &[TakeNote]) -> [u8; 32] {
        let mut l = Law::load(score).unwrap();
        l.admit(take).unwrap();
        l.hash().unwrap()
    }

    /// A one-sample change to the take, a one-tick change to the score, a
    /// one-microsecond change to the tempo, or any one-unit change to any
    /// other field moves the hash.
    #[test]
    fn any_one_unit_change_to_any_input_moves_the_hash() {
        let (score, take) = fixture();
        let base = hash_of(&score, &take);
        let mut seen = vec![base];
        let mut check = |label: &str, s: &IngestedScore, t: &[TakeNote]| {
            let h = hash_of(s, t);
            assert!(!seen.contains(&h), "{label} did not move the hash");
            seen.push(h);
        };

        for i in 0..take.len() {
            let mut t = take.clone();
            t[i].onset_sample += 1;
            check("take onset +1 sample", &score, &t);
            let mut t = take.clone();
            t[i].onset_sample -= 1;
            check("take onset -1 sample", &score, &t);
            let mut t = take.clone();
            t[i].velocity += 1;
            check("take velocity", &score, &t);
            let mut t = take.clone();
            t[i].pitch += 1;
            check("take pitch", &score, &t);
            let mut t = take.clone();
            t[i].cites = None;
            check("take citation dropped", &score, &t);
        }
        let mut t = take.clone();
        t[0].cites = Some(ScoreNoteId(1));
        check("take citation moved", &score, &t);
        let mut t = take.clone();
        t.pop();
        check("take note removed", &score, &t);

        for i in 0..score.notes.len() {
            let mut s = score.clone();
            s.notes[i].end_tick += 1;
            check("score note end +1 tick", &s, &take);
            let mut s = score.clone();
            s.notes[i].velocity += 1;
            check("score note velocity", &s, &take);
            let mut s = score.clone();
            s.notes[i].track += 1;
            check("score note track", &s, &take);
            let mut s = score.clone();
            s.notes[i].channel += 1;
            check("score note channel", &s, &take);
            let mut s = score.clone();
            s.notes[i].pitch += 1;
            check("score note pitch", &s, &take);
        }
        let mut s = score.clone();
        s.notes[1].start_tick += 1;
        check("score note start +1 tick", &s, &take);
        let mut s = score.clone();
        s.tempo[1].us_per_quarter += 1;
        check("tempo +1 microsecond", &s, &take);
        let mut s = score.clone();
        s.tempo[0].us_per_quarter -= 1;
        check("first tempo -1 microsecond", &s, &take);
        let mut s = score.clone();
        s.tempo[1].tick += 1;
        check("tempo change +1 tick", &s, &take);
        let mut s = score.clone();
        s.meter[1].tick += 1;
        check("meter change +1 tick", &s, &take);
        let mut s = score.clone();
        s.meter[1].numerator += 1;
        check("meter numerator", &s, &take);
        let mut s = score.clone();
        s.meter[1].denominator_pow2 += 1;
        check("meter denominator", &s, &take);
    }

    /// A one-sample change that crosses the gate edge changes the verdict too,
    /// and the snapshot says so.
    #[test]
    fn a_one_sample_change_at_the_gate_edge_changes_the_verdict() {
        let (score, mut take) = fixture();
        let onset = Law::load(&score).unwrap().score().notes()[2].onset_sample;
        take[2].onset_sample = onset + 1_920;
        let mut l = Law::load(&score).unwrap();
        l.admit(&take).unwrap();
        assert_eq!(l.verdicts().unwrap()[2].word(), "match");
        take[2].onset_sample = onset + 1_921;
        let mut l = Law::load(&score).unwrap();
        l.admit(&take).unwrap();
        assert_eq!(l.verdicts().unwrap()[2].word(), "late");
    }
}
