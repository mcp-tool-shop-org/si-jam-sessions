//! The law: a score, a take, and a transport that steps whether or not
//! anything is proposed.

use alloc::string::String;
use alloc::vec::Vec;

use score_model::IngestedScore;
use sha2::{Digest, Sha256};

use crate::grade::{self, Verdict};
use crate::refusal::Refusal;
use crate::score::LawScore;
use crate::snapshot;
use crate::take::TakeNote;
use crate::{HORIZON_QUANTA, MAX_SAMPLE};

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
/// A committed quantum never changes: every quantum at or before the horizon
/// is refused, and the horizon only moves forward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Law {
    score: LawScore,
    /// Strictly increasing in [`TakeNote::key`].
    take: Vec<TakeNote>,
    steps: u64,
}

impl Law {
    /// Loads a score and stops the transport: an empty take, zero steps.
    /// See [`LawScore::from_ingested`] for what is refused.
    pub fn load(score: &IngestedScore) -> Result<Self, Refusal> {
        Ok(Law {
            score: LawScore::from_ingested(score)?,
            take: Vec::new(),
            steps: 0,
        })
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
    /// forward by one quantum.
    pub fn step(&mut self) -> Result<(), Refusal> {
        let refused = Refusal::StepOverflow { steps: self.steps };
        let next = self.steps.checked_add(1).ok_or(refused)?;
        // After this step the first open quantum is next + H; it must be a
        // u64, so the horizon and every lateness stay representable.
        next.checked_add(u64::from(HORIZON_QUANTA)).ok_or(refused)?;
        self.steps = next;
        Ok(())
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
    /// 4. not the key of a note already admitted.
    ///
    /// On success the notes merge into the take, which stays in key order, so
    /// the take does not depend on how its notes were batched.
    pub fn admit(&mut self, notes: &[TakeNote]) -> Result<(), Refusal> {
        let horizon = self.committed_horizon()?;
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

    /// Every note graded; see [`Verdict`] for the order.
    pub fn verdicts(&self) -> Result<Vec<Verdict>, Refusal> {
        grade::verdicts(&self.score, &self.take)
    }

    /// One row per verdict, in verdict order, each stating its comparison in
    /// digits.
    pub fn rows(&self) -> Result<Vec<String>, Refusal> {
        grade::rows(&self.verdicts()?)
    }

    /// The canonical snapshot: the pins, the score in law ticks with its
    /// sample positions, the tempo map, the take, the verdicts and the rows,
    /// as explicit little-endian bytes in a declared order (see
    /// [`crate::SNAPSHOT_FORMAT`]).
    ///
    /// The transport position is not in it: the snapshot is the graded record,
    /// the same however many quanta the host stepped to reach it.
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Refusal> {
        let verdicts = self.verdicts()?;
        let rows = grade::rows(&verdicts)?;
        snapshot::encode(&self.score, &self.take, &verdicts, &rows)
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
    use crate::{QUANTUM_SAMPLES, SNAPSHOT_FORMAT, SNAPSHOT_MAGIC};
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
        assert_eq!(l.step(), Ok(()));
        assert_eq!(l.committed_horizon(), Ok(Some(u64::MAX - 1)));
        assert_eq!(
            l.step(),
            Err(Refusal::StepOverflow {
                steps: u64::MAX - H
            })
        );
        assert_eq!(l.steps(), u64::MAX - H, "a refused step does not move");
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
        ] {
            header.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(&bytes[..header.len()], header.as_slice());
        assert_eq!(&bytes[8..12], &[1, 0, 0, 0]);
        assert_eq!(&bytes[16..20], &[0x20, 0x0D, 0, 0], "3360 little-endian");
        assert_eq!(&bytes[20..24], &[0x80, 0xBB, 0, 0], "48000 little-endian");
        assert_eq!(&bytes[36..40], b"TMPO");
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
