//! The live verbs and the frame export.
//!
//! Most scores here run at 70,000 microseconds per quarter with source PPQ
//! 3360, where one law tick is exactly one sample: 3,360 ticks × 70,000 us ×
//! 48,000 / 3.36e9 = 3,360 samples. An onset written in ticks is then the
//! onset in samples, and every boundary below is placed to the sample.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use score_model::{IngestedNote, IngestedScore, MeterChange, TempoChange};

use crate::frames::{Beat, FrameNote, Frames, Voice};
use crate::refusal::{Refusal, WireFault};
use crate::take::{ScoreNoteId, TakeNote};
use crate::{
    GATE_SAMPLES, HORIZON_QUANTA, LIVE_REACH_SAMPLES, Law, LiveNote, LiveNoteOff, QUANTUM_SAMPLES,
    wire,
};

const H: u64 = HORIZON_QUANTA as u64;
const Q: u64 = QUANTUM_SAMPLES as u64;
const REACH: u64 = LIVE_REACH_SAMPLES as u64;
const GATE: u64 = GATE_SAMPLES as u64;

/// One tick is one sample at this tempo and source PPQ 3360.
const TICK_IS_SAMPLE: u32 = 70_000;

fn scored(notes: &[(u64, u8, u64, u16)], meter: &[(u64, u8, u8)]) -> IngestedScore {
    let mut notes: Vec<IngestedNote> = notes
        .iter()
        .map(|&(start, pitch, length, track)| IngestedNote {
            start_tick: start,
            pitch,
            track,
            channel: 0,
            end_tick: start + length,
            velocity: 80,
        })
        .collect();
    notes.sort();
    IngestedScore {
        source_ppq: 3_360,
        tempo: vec![TempoChange {
            tick: 0,
            us_per_quarter: TICK_IS_SAMPLE,
        }],
        meter: meter
            .iter()
            .map(|&(tick, numerator, denominator_pow2)| MeterChange {
                tick,
                numerator,
                denominator_pow2,
            })
            .collect(),
        notes,
    }
}

/// Notes at the given onsets, 1,000 samples long, on track 1, in 4/4.
fn law_of(notes: &[(u64, u8)]) -> Law {
    let notes: Vec<(u64, u8, u64, u16)> = notes.iter().map(|&(o, p)| (o, p, 1_000, 1)).collect();
    Law::load(&scored(&notes, &[(0, 4, 2)])).unwrap()
}

/// Steps until quantum `q` is committed.
fn commit_through(law: &mut Law, q: u64) {
    while law.committed_horizon().unwrap().is_none_or(|h| h < q) {
        law.step().unwrap();
    }
}

fn played(onset: i64, pitch: u32) -> LiveNote {
    LiveNote {
        onset_sample: onset,
        pitch,
        velocity: 90,
    }
}

fn off(pitch: u32, off_sample: i64) -> LiveNoteOff {
    LiveNoteOff { pitch, off_sample }
}

/// Plays one live note-on and returns what the law made it cite.
fn cites(law: &mut Law, onset: u64, pitch: u8) -> Option<u32> {
    commit_through(law, onset / Q);
    law.live(played(onset as i64, u32::from(pitch)))
        .unwrap()
        .cites
        .map(|id| id.0)
}

/// What one live note cites on a fresh law of `notes`.
fn fresh(notes: &[(u64, u8)], onset: u64, pitch: u8) -> Option<u32> {
    cites(&mut law_of(notes), onset, pitch)
}

fn words(law: &Law) -> Vec<&'static str> {
    law.verdicts().unwrap().iter().map(|v| v.word()).collect()
}

fn rows(law: &Law) -> Vec<String> {
    law.rows().unwrap()
}

// --- The live verb: when it admits ------------------------------------------

#[test]
fn a_live_note_needs_a_running_transport() {
    let mut law = law_of(&[(0, 60)]);
    assert_eq!(law.live(played(0, 60)), Err(Refusal::LiveStopped));
    assert!(law.take().is_empty());
    law.step().unwrap();
    assert!(law.live(played(0, 60)).is_ok());
}

/// A live note behind the horizon is admitted; the same note proposed as a
/// take is refused as late. The horizon refuses late proposals only.
#[test]
fn a_live_note_is_never_refused_for_lateness() {
    let mut law = law_of(&[(0, 60), (24_000, 62)]);
    for _ in 0..10_000 {
        law.step().unwrap();
    }
    let horizon = 9_999 + H;
    assert_eq!(law.committed_horizon(), Ok(Some(horizon)));
    let as_take = TakeNote {
        onset_sample: 0,
        pitch: 60,
        velocity: 90,
        cites: Some(ScoreNoteId(0)),
    };
    assert_eq!(
        law.admit(&[as_take]),
        Err(Refusal::Late {
            index: 0,
            quantum: 0,
            horizon,
            lateness: horizon + 1,
        })
    );
    assert_eq!(law.live(played(0, 60)), Ok(as_take));
    assert_eq!(law.take(), [as_take]);
}

/// The last sample of the horizon's quantum is admitted; the first sample of
/// the next quantum has not been reached.
#[test]
fn a_live_note_past_the_horizon_is_refused() {
    let mut law = law_of(&[(0, 60)]);
    law.step().unwrap();
    let horizon = H;
    let last = (horizon + 1) * Q - 1;
    assert_eq!(
        law.live(played(last as i64 + 1, 60)),
        Err(Refusal::LiveAhead {
            sample: last + 1,
            quantum: horizon + 1,
            horizon,
        })
    );
    assert!(law.live(played(last as i64, 60)).is_ok());
}

#[test]
fn each_malformed_live_note_is_refused_by_name() {
    let mut law = law_of(&[(0, 60)]);
    law.step().unwrap();
    let base = played(100, 60);
    let refused = |law: &mut Law, note: LiveNote| law.live(note).unwrap_err();

    for pitch in [128, 255, 256, u32::MAX] {
        assert_eq!(
            refused(&mut law, LiveNote { pitch, ..base }),
            Refusal::LivePitch { pitch }
        );
    }
    for velocity in [0, 128, 256, u32::MAX] {
        assert_eq!(
            refused(&mut law, LiveNote { velocity, ..base }),
            Refusal::LiveVelocity { velocity }
        );
    }
    for onset_sample in [-1, -48, i64::MIN] {
        assert_eq!(
            refused(
                &mut law,
                LiveNote {
                    onset_sample,
                    ..base
                }
            ),
            Refusal::LiveBeforeStart {
                sample: onset_sample
            }
        );
    }
    // The checks run in order: pitch before velocity before the onset.
    assert_eq!(
        refused(
            &mut law,
            LiveNote {
                onset_sample: -1,
                pitch: 128,
                velocity: 0,
            }
        ),
        Refusal::LivePitch { pitch: 128 }
    );
    assert_eq!(
        refused(
            &mut law,
            LiveNote {
                onset_sample: -1,
                velocity: 0,
                ..base
            }
        ),
        Refusal::LiveVelocity { velocity: 0 }
    );
    assert!(law.take().is_empty(), "a refused note changes nothing");

    // The edges that are admitted.
    for (pitch, velocity) in [(0, 1), (127, 127)] {
        let note = LiveNote {
            pitch,
            velocity,
            ..base
        };
        assert!(law.live(note).is_ok(), "pitch {pitch} velocity {velocity}");
    }
}

/// The live verb runs the take's admission, and it counts the take's
/// citations: a score note a take note cites is not cited again, and two
/// additions struck at one onset and pitch are a duplicate.
#[test]
fn the_live_verb_shares_the_takes_admission() {
    let mut law = law_of(&[(24_000, 60)]);
    let take = TakeNote {
        onset_sample: 24_100,
        pitch: 60,
        velocity: 30,
        cites: Some(ScoreNoteId(0)),
    };
    law.admit(&[take]).unwrap();
    commit_through(&mut law, 1_000);
    let again = LiveNote {
        onset_sample: 24_100,
        pitch: 60,
        velocity: 100,
    };
    // Score note 0 is cited by the take note, so the live note is an addition.
    assert_eq!(law.live(again).map(|t| t.cites), Ok(None));
    assert_eq!(
        law.live(again),
        Err(Refusal::LiveDuplicate {
            onset_sample: 24_100,
            pitch: 60,
            cites: None
        })
    );
    assert_eq!(law.take().len(), 2);
    assert_eq!(words(&law), ["match", "addition"]);
}

// --- What a live note cites ---------------------------------------------------

/// A note of the score note's pitch answers it from up to twice the gate away
/// on either side; one sample further, it is an addition.
#[test]
fn a_same_pitch_note_answers_within_the_reach() {
    let score = [(100_000, 60)];
    assert_eq!(fresh(&score, 100_000 + REACH, 60), Some(0));
    assert_eq!(fresh(&score, 100_000 - REACH, 60), Some(0));
    assert_eq!(fresh(&score, 100_000 + REACH + 1, 60), None);
    assert_eq!(fresh(&score, 100_000 - REACH - 1, 60), None);
    let mut law = law_of(&score);
    cites(&mut law, 100_000 - REACH, 60);
    assert_eq!(words(&law), ["early"]);
    let mut law = law_of(&score);
    cites(&mut law, 100_000 + REACH, 60);
    assert_eq!(words(&law), ["late"]);
}

/// Of two notes of the pitch, the nearer; at equal distance, the one before
/// the live note; at one onset (a doubled pitch), the lower id.
#[test]
fn a_same_pitch_note_answers_the_nearest() {
    // Notes 0 and 1 at 100,000 on tracks 1 and 2, note 2 at 103,000.
    let score = scored(
        &[
            (100_000, 60, 500, 1),
            (100_000, 60, 500, 2),
            (103_000, 60, 500, 1),
        ],
        &[(0, 4, 2)],
    );
    let on = |onset| cites(&mut Law::load(&score).unwrap(), onset, 60);
    assert_eq!(on(101_499), Some(0), "nearer the first");
    assert_eq!(on(101_501), Some(2), "nearer the second");
    assert_eq!(on(101_500), Some(0), "equidistant: the one before");
    assert_eq!(on(100_000), Some(0), "the doubled pitch: the lower id");
}

/// Another pitch answers a score note only within the gate: the nearest onset,
/// then the nearest pitch, then the lower pitch, then the lowest id.
#[test]
fn another_pitch_answers_within_the_gate() {
    // A chord of 57, 60 and 64 at 200,000 (ids 0, 1, 2); 62 alone at 202,000
    // (id 3).
    let chord = [(200_000, 57), (200_000, 60), (200_000, 64), (202_000, 62)];
    // 61 on the chord: 60 is one semitone away; 62 is too, but 2,000 samples
    // off, outside the gate.
    assert_eq!(fresh(&chord, 200_000, 61), Some(1));
    // 58 on the chord: 57 is one away, 60 two.
    assert_eq!(fresh(&chord, 200_000, 58), Some(0));
    // 62 on the chord: its own pitch is 2,000 samples off, inside the reach,
    // so it answers note 3, before any wrong-pitch reading.
    assert_eq!(fresh(&chord, 200_000, 62), Some(3));
    // 59, 1,000 samples before the chord: 60 is one away, 57 two.
    assert_eq!(fresh(&chord, 199_000, 59), Some(1));
    // 60 between 58 and 62 ties on pitch distance: the lower pitch.
    assert_eq!(fresh(&[(50_000, 58), (50_000, 62)], 50_000, 60), Some(0));
    // Two notes of one pitch at one onset tie on everything: the lower id.
    let doubled = scored(&[(50_000, 62, 500, 1), (50_000, 62, 500, 2)], &[(0, 4, 2)]);
    assert_eq!(
        cites(&mut Law::load(&doubled).unwrap(), 50_000, 61),
        Some(0)
    );
    // Another pitch one sample past the gate is an addition.
    let lone = [(50_000, 60)];
    assert_eq!(fresh(&lone, 50_000 + GATE, 61), Some(0));
    assert_eq!(fresh(&lone, 50_000 + GATE + 1, 61), None);
    assert_eq!(fresh(&lone, 50_000 - GATE - 1, 61), None);
    let mut law = law_of(&lone);
    cites(&mut law, 50_000 + GATE, 61);
    assert_eq!(words(&law), ["wrong pitch"]);
}

// --- At most once ---------------------------------------------------------------

/// A score note is cited at most once: a second live note in its reach, with
/// nothing else to answer, is an addition.
#[test]
fn a_score_note_is_cited_at_most_once() {
    let mut law = law_of(&[(100_000, 60)]);
    assert_eq!(cites(&mut law, 100_000, 60), Some(0));
    assert_eq!(cites(&mut law, 100_500, 60), None);
    assert_eq!(
        rows(&law),
        [
            "note 0: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 60 vs 60: match",
            "take note 1: onset 100500 samples, pitch 60, cites no score note: addition",
        ]
    );
}

/// A second live note in a cited note's reach takes the next candidate, in the
/// rule's order: the next score note of its pitch, then, once those are gone,
/// the next of another pitch within the gate, and then nothing.
#[test]
fn a_second_note_takes_the_next_eligible_score_note() {
    // Two 60s, 2,000 samples apart.
    let mut law = law_of(&[(100_000, 60), (102_000, 60)]);
    assert_eq!(cites(&mut law, 100_900, 60), Some(0), "900 away, not 1,100");
    assert_eq!(cites(&mut law, 101_000, 60), Some(1), "0 is cited");
    assert_eq!(words(&law), ["match", "match"]);
    // A chord of 57, 60 and 64, and four 61s: 60, then 64, then 57, by pitch
    // distance, and the fourth is an addition.
    let mut law = law_of(&[(200_000, 57), (200_000, 60), (200_000, 64)]);
    assert_eq!(cites(&mut law, 200_000, 61), Some(1));
    assert_eq!(cites(&mut law, 200_010, 61), Some(2));
    assert_eq!(cites(&mut law, 200_020, 61), Some(0));
    assert_eq!(cites(&mut law, 200_030, 61), None);
    assert_eq!(
        words(&law),
        ["wrong pitch", "wrong pitch", "wrong pitch", "addition"]
    );
}

/// A pitch doubled on two tracks takes two live notes of the pitch, one each.
#[test]
fn a_doubled_pitch_gives_each_of_its_notes_one_live_note() {
    let score = scored(
        &[(100_000, 60, 500, 1), (100_000, 60, 500, 2)],
        &[(0, 4, 2)],
    );
    let mut law = Law::load(&score).unwrap();
    assert_eq!(cites(&mut law, 100_000, 60), Some(0));
    assert_eq!(cites(&mut law, 100_010, 60), Some(1));
    assert_eq!(cites(&mut law, 100_020, 60), None);
    assert_eq!(words(&law), ["match", "match", "addition"]);
}

/// Every permutation of `0..n`, in a fixed order.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for rest in permutations(n - 1) {
        for at in 0..=rest.len() {
            let mut p = rest.clone();
            p.insert(at, n - 1);
            out.push(p);
        }
    }
    out
}

/// A chord of four notes, its keys going down in each of the 24 orders a few
/// samples apart: each finds its own score note, because a note's own pitch is
/// sought before any other.
#[test]
fn chords_find_their_own_notes_in_any_order() {
    let chord = [(50_000, 60), (50_000, 64), (50_000, 67), (50_000, 72)];
    let orders = permutations(4);
    assert_eq!(orders.len(), 24);
    for order in orders {
        let mut law = law_of(&chord);
        for (k, &i) in order.iter().enumerate() {
            let onset = 50_000 + 7 * k as u64;
            assert_eq!(
                cites(&mut law, onset, chord[i].1),
                Some(i as u32),
                "order {order:?}"
            );
        }
        assert_eq!(words(&law), ["match"; 4], "order {order:?}");
    }
}

/// The placeholder follower's known limit: it does not revise a citation. A
/// wrong key that goes down before the right one takes the chord note nearest
/// its pitch, and each right key after it finds its note cited and takes the
/// next, down to an addition.
#[test]
fn a_wrong_key_before_the_right_one_takes_its_note() {
    // A chord of 60, 64 and 67; 65 (F) goes down first, then the chord.
    let mut law = law_of(&[(50_000, 60), (50_000, 64), (50_000, 67)]);
    assert_eq!(cites(&mut law, 50_000, 65), Some(1), "64 is nearest to 65");
    assert_eq!(
        cites(&mut law, 50_005, 64),
        Some(2),
        "64 is cited: 67 is next"
    );
    assert_eq!(
        cites(&mut law, 50_010, 67),
        Some(0),
        "67 is cited: 60 is left"
    );
    assert_eq!(cites(&mut law, 50_015, 60), None);
    assert_eq!(
        words(&law),
        ["wrong pitch", "wrong pitch", "wrong pitch", "addition"]
    );
}

/// Because a cited note is passed over, the order two competing notes arrive
/// in decides which cites the score note: the verb takes them in the order
/// their keys went down.
#[test]
fn the_order_notes_arrive_in_decides_their_citations() {
    let run = |first: u64, second: u64| {
        let mut law = law_of(&[(100_000, 60)]);
        let a = cites(&mut law, first, 60);
        let b = cites(&mut law, second, 60);
        (a, b, law.snapshot_bytes().unwrap())
    };
    let (a, b, early_first) = run(99_000, 100_500);
    assert_eq!((a, b), (Some(0), None));
    let (a, b, late_first) = run(100_500, 99_000);
    assert_eq!((a, b), (Some(0), None));
    assert_ne!(early_first, late_first, "the take cites a different note");
}

// --- Never played, in a live session -------------------------------------------

/// In a live session, an uncited score note is never played once its reach
/// window has passed the committed horizon, and its row says so; before that
/// it has no row. The window passes the horizon about 20 ms before the note
/// is due (the reach, 80 ms, is shorter than H, 100 ms), so a live note that
/// arrives afterwards, as live notes do, can still cite it.
#[test]
fn in_a_live_session_a_note_is_never_played_once_its_window_passes_the_horizon() {
    let mut law = law_of(&[(100_000, 60), (200_000, 62), (300_000, 64)]);
    commit_through(&mut law, 100_000 / Q);
    assert_eq!(cites(&mut law, 100_000, 60), Some(0));
    let match_0 = "note 0: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 60 vs 60: match";
    assert_eq!(rows(&law), [match_0], "notes 1 and 2 are not reached");

    // Note 1's window ends on sample 203,840. The horizon's last sample
    // reaches it at quantum 4,246: (4,246 + 1) × 48 - 1 = 203,855.
    commit_through(&mut law, 4_245);
    assert_eq!(law.committed_horizon(), Ok(Some(4_245)));
    assert_eq!(rows(&law), [match_0]);
    let before = law.snapshot_bytes().unwrap();
    law.step().unwrap();
    let never_1 = "note 1: onset 200000 samples, pitch 62, no take note cites it: never played";
    assert_eq!(rows(&law), [match_0, never_1]);
    assert_ne!(law.snapshot_bytes().unwrap(), before, "the rows are hashed");
    // The playhead is 992 samples (20.7 ms) short of note 1.
    assert_eq!(200_000 - (law.steps() - 1) * Q, 992);

    // A live note for note 1 arrives now, behind the horizon, and cites it.
    assert_eq!(cites(&mut law, 200_100, 62), Some(1));
    assert_eq!(
        rows(&law),
        [
            match_0,
            "note 1: onset +100 samples (+2.1 ms) vs gate \u{b1}1920, pitch 62 vs 62: match"
        ]
    );
    // Note 2 is reached only when its own window passes.
    commit_through(&mut law, 303_840 / Q);
    assert_eq!(rows(&law).len(), 3);
    assert!(rows(&law)[2].ends_with("never played"));
}

/// A window passes the horizon on the sample: when its last sample, the
/// onset plus the reach, is the horizon's last sample.
#[test]
fn a_window_passes_the_horizon_on_its_last_sample() {
    // Quantum 4,245 ends on sample 203,807, where this note's window ends.
    let onset = (4_245 + 1) * Q - 1 - REACH;
    assert_eq!(onset, 199_967);
    let mut law = law_of(&[(50_000, 60), (onset, 62)]);
    assert_eq!(cites(&mut law, 50_000, 60), Some(0), "a live session");
    commit_through(&mut law, 4_244);
    assert_eq!(law.committed_horizon(), Ok(Some(4_244)));
    assert_eq!(words(&law), ["match"]);
    law.step().unwrap();
    assert_eq!(words(&law), ["match", "never played"]);
}

/// A take admitted as a batch, without the live verb, is graded as version 3
/// graded it: every uncited score note is never played, stopped or running,
/// and stepping does not change the record.
#[test]
fn a_take_admitted_as_a_batch_is_graded_as_version_3() {
    let mut law = law_of(&[(100_000, 60), (200_000, 62), (300_000, 64)]);
    law.admit(&[TakeNote {
        onset_sample: 100_000,
        pitch: 60,
        velocity: 90,
        cites: Some(ScoreNoteId(0)),
    }])
    .unwrap();
    let stopped = law.snapshot_bytes().unwrap();
    assert_eq!(words(&law), ["match", "never played", "never played"]);
    for _ in 0..3_000 {
        law.step().unwrap();
    }
    assert_eq!(words(&law), ["match", "never played", "never played"]);
    assert_eq!(law.snapshot_bytes().unwrap(), stopped);
}

// --- The note-off ----------------------------------------------------------------

/// A note-off ends the latest held note of its pitch, and the frames carry
/// the length from the note-on to it.
#[test]
fn a_note_off_ends_the_latest_held_note_of_its_pitch() {
    let mut law = law_of(&[(1_000, 60)]);
    commit_through(&mut law, 100);
    let first = law.live(played(1_000, 60)).unwrap();
    let second = law.live(played(2_000, 60)).unwrap();
    assert_eq!(first.cites, Some(ScoreNoteId(0)));
    assert_eq!(second.cites, None, "0 is cited");
    assert_eq!(law.live_off(off(60, 2_500)), Ok(second));
    assert_eq!(law.live_off(off(60, 3_000)), Ok(first));
    assert_eq!(
        law.live_off(off(60, 3_500)),
        Err(Refusal::LiveNotHeld { pitch: 60 })
    );
    let lengths: Vec<(u64, Voice, u64)> = law
        .frames(0, 100)
        .unwrap()
        .notes
        .iter()
        .map(|n| (n.onset_sample, n.voice, n.duration_samples))
        .collect();
    assert_eq!(
        lengths,
        [
            (1_000, Voice::Score, 1_000),
            (1_000, Voice::Live, 2_000),
            (2_000, Voice::Live, 500),
        ]
    );
}

#[test]
fn each_bad_note_off_is_refused_by_name() {
    let mut law = law_of(&[(1_000, 60)]);
    assert_eq!(law.live_off(off(60, 10)), Err(Refusal::LiveStopped));
    law.step().unwrap();
    // In order: the pitch, then the release's place, then a held note.
    assert_eq!(
        law.live_off(off(128, -1)),
        Err(Refusal::LivePitch { pitch: 128 })
    );
    assert_eq!(
        law.live_off(off(60, -5)),
        Err(Refusal::LiveBeforeStart { sample: -5 })
    );
    let ahead = (H + 1) * Q;
    assert_eq!(
        law.live_off(off(60, ahead as i64)),
        Err(Refusal::LiveAhead {
            sample: ahead,
            quantum: H + 1,
            horizon: H
        })
    );
    assert_eq!(
        law.live_off(off(60, 10)),
        Err(Refusal::LiveNotHeld { pitch: 60 })
    );
    law.live(played(1_000, 60)).unwrap();
    for release in [1_000u64, 999] {
        assert_eq!(
            law.live_off(off(60, release as i64)),
            Err(Refusal::LiveEmpty {
                onset_sample: 1_000,
                off_sample: release
            })
        );
    }
    // A refused note-off leaves the note held.
    assert!(law.live_off(off(60, 1_001)).is_ok());
}

// --- The same notes, live or as a take --------------------------------------------

/// The slice-1 pattern played live, in reverse order, is the take admitted
/// whole: the same citations, verdicts, rows and snapshot.
#[test]
fn live_notes_grade_as_the_same_notes_admitted_as_a_take() {
    let quarters: Vec<(u64, u8)> = (0..8u64)
        .map(|q| (24_000 + q * 24_000, 60 + 2 * q as u8))
        .collect();
    // (note, onset delta in samples, semitones up)
    let pattern: [(usize, i64, u8); 5] = [
        (1, 1_440, 0),
        (2, 2_160, 0),
        (4, 2_880, 0),
        (5, -2_160, 0),
        (6, 0, 1),
    ];
    let take: Vec<TakeNote> = quarters
        .iter()
        .enumerate()
        .map(|(id, &(onset, pitch))| {
            let (delta, up) = pattern
                .iter()
                .find(|p| p.0 == id)
                .map_or((0, 0), |p| (p.1, p.2));
            TakeNote {
                onset_sample: (onset as i64 + delta) as u64,
                pitch: pitch + up,
                velocity: 70 + id as u8,
                cites: Some(ScoreNoteId(id as u32)),
            }
        })
        .collect();

    let mut admitted = law_of(&quarters);
    admitted.admit(&take).unwrap();

    let mut live = law_of(&quarters);
    commit_through(&mut live, 300_000 / Q);
    for t in take.iter().rev() {
        let note = LiveNote {
            onset_sample: t.onset_sample as i64,
            pitch: u32::from(t.pitch),
            velocity: u32::from(t.velocity),
        };
        assert_eq!(live.live(note), Ok(*t));
        let release = off(u32::from(t.pitch), t.onset_sample as i64 + 1_234);
        assert_eq!(live.live_off(release), Ok(*t));
    }
    assert_eq!(live.take(), admitted.take());
    assert_eq!(live.verdicts(), admitted.verdicts());
    assert_eq!(live.rows(), admitted.rows());
    assert_eq!(live.snapshot_bytes(), admitted.snapshot_bytes());
    assert_eq!(
        words(&live),
        [
            "match",
            "match",
            "late",
            "match",
            "late",
            "early",
            "wrong pitch",
            "match"
        ]
    );
}

// --- The frame export --------------------------------------------------------

#[test]
fn frames_come_only_from_committed_quanta() {
    let mut law = law_of(&[(0, 60)]);
    assert_eq!(law.frames(0, 0), Err(Refusal::FramesStopped));
    law.step().unwrap();
    assert_eq!(
        law.frames(5, 4),
        Err(Refusal::FramesWindow { first: 5, last: 4 })
    );
    assert!(law.frames(0, H).is_ok(), "the horizon itself is committed");
    assert_eq!(
        law.frames(0, H + 1),
        Err(Refusal::FramesNotCommitted {
            last: H + 1,
            horizon: H
        })
    );
    assert_eq!(
        law.frames(H + 1, H + 1),
        Err(Refusal::FramesNotCommitted {
            last: H + 1,
            horizon: H
        })
    );
    law.step().unwrap();
    assert!(
        law.frames(H + 1, H + 1).is_ok(),
        "one step commits one more"
    );
}

/// A live note the test admitted: the take note the law made of it, and its
/// length.
type Admitted = (TakeNote, u64);

/// Every event in quanta `0..=last`, computed without the export: each score
/// note and take note by its onset, and each beat by walking the meter beat by
/// beat through the tempo map.
fn expected(law: &Law, last: u64, live: &[Admitted]) -> (Vec<FrameNote>, Vec<Beat>) {
    let end = (last + 1) * Q;
    let mut notes = Vec::new();
    for (id, n) in law.score().notes().iter().enumerate() {
        if n.onset_sample < end {
            notes.push(FrameNote {
                onset_sample: n.onset_sample,
                voice: Voice::Score,
                note: Some(ScoreNoteId(id as u32)),
                pitch: n.pitch,
                velocity: n.velocity,
                duration_samples: n.duration_samples,
            });
        }
    }
    for t in law.take() {
        if t.onset_sample < end {
            let played = live.iter().find(|(note, _)| note == t);
            notes.push(FrameNote {
                onset_sample: t.onset_sample,
                voice: if played.is_some() {
                    Voice::Live
                } else {
                    Voice::Take
                },
                note: t.cites,
                pitch: t.pitch,
                velocity: t.velocity,
                duration_samples: match (played, t.cites) {
                    (Some((_, length)), _) => *length,
                    (None, Some(id)) => law.score().note(id).unwrap().duration_samples,
                    (None, None) => 0,
                },
            });
        }
    }
    notes.sort_by_key(FrameNote::order);

    let mut beats = Vec::new();
    let meter = law.score().meter();
    let map = law.score().tempo_map();
    let mut bar = 0u64;
    for (k, m) in meter.iter().enumerate() {
        let unit = 13_440u64 >> m.denominator_pow2;
        let stop = meter.get(k + 1).map_or(u64::MAX, |n| n.tick);
        let mut tick = m.tick;
        let mut j = 0u64;
        while tick < stop {
            let sample = map.sample_at(tick).unwrap();
            if sample >= end && k + 1 == meter.len() {
                break;
            }
            if sample < end {
                let beat = j % u64::from(m.numerator);
                beats.push(Beat {
                    onset_sample: sample,
                    bar: bar + j / u64::from(m.numerator),
                    beat: beat as u8,
                    downbeat: beat == 0,
                });
            }
            j += 1;
            tick += unit;
        }
        bar += j.div_ceil(u64::from(m.numerator));
    }
    (notes, beats)
}

/// Concatenates the frames of consecutive windows of the given sizes, which
/// together cover quanta `0..=last`, checking each event is in its window.
fn in_windows(law: &Law, last: u64, sizes: &[u64]) -> (Vec<FrameNote>, Vec<Beat>) {
    let (mut notes, mut beats) = (Vec::new(), Vec::new());
    let mut first = 0;
    let mut i = 0;
    while first <= last {
        let size = sizes[i % sizes.len()];
        let end = (first + size - 1).min(last);
        let f = law.frames(first, end).unwrap();
        assert_eq!((f.first_quantum, f.last_quantum), (first, end));
        for n in &f.notes {
            let q = n.onset_sample / Q;
            assert!((first..=end).contains(&q), "{n:?} outside {first}..={end}");
        }
        for b in &f.beats {
            let q = b.onset_sample / Q;
            assert!((first..=end).contains(&q), "{b:?} outside {first}..={end}");
        }
        notes.extend(f.notes);
        beats.extend(f.beats);
        first = end + 1;
        i += 1;
    }
    (notes, beats)
}

/// Events on both sides of quantum boundaries, in a score, a take and a live
/// take, read through windows of many sizes: each appears exactly once, in
/// order, and the whole matches what is computed without the export.
#[test]
fn consecutive_windows_hand_over_every_event_exactly_once() {
    // Onsets on the last sample of a quantum and on the first of the next, a
    // chord, and a doubled pitch.
    let notes = [
        (0, 60, 10, 1),
        (47, 61, 10, 1),
        (48, 62, 10, 1),
        (95, 63, 10, 1),
        (96, 64, 10, 1),
        (4_799, 65, 10, 1),
        (4_800, 66, 10, 1),
        (9_600, 67, 10, 1),
        (9_600, 67, 10, 2),
        (9_600, 71, 10, 1),
        (13_439, 72, 10, 1),
    ];
    // 2/4, then 3/8 from tick 6,719: beats every 3,360 samples on the first
    // sample of a quantum, then every 1,680 on the last sample of one.
    let score = scored(&notes, &[(0, 2, 2), (6_719, 3, 3)]);
    let mut law = Law::load(&score).unwrap();
    law.admit(&[
        TakeNote {
            onset_sample: 47,
            pitch: 61,
            velocity: 50,
            cites: Some(ScoreNoteId(1)),
        },
        TakeNote {
            onset_sample: 96,
            pitch: 90,
            velocity: 51,
            cites: None,
        },
        TakeNote {
            onset_sample: 13_440,
            pitch: 72,
            velocity: 52,
            cites: Some(ScoreNoteId(10)),
        },
    ])
    .unwrap();
    let last = 300;
    commit_through(&mut law, last);
    let mut live = Vec::new();
    for (onset, pitch) in [(48, 62), (4_799, 50), (4_800, 66), (14_399, 72)] {
        let note = law
            .live(LiveNote {
                onset_sample: onset,
                pitch,
                velocity: 99,
            })
            .unwrap();
        law.live_off(off(pitch, onset + 7)).unwrap();
        live.push((note, 7));
    }

    let (notes, beats) = expected(&law, last, &live);
    assert_eq!(notes.len(), 11 + 3 + 4);
    assert_eq!(beats.len(), 7);
    assert!(
        beats
            .iter()
            .any(|b| b.onset_sample > 0 && b.onset_sample % Q == 0)
    );
    assert!(beats.iter().any(|b| b.onset_sample % Q == Q - 1));
    let whole = law.frames(0, last).unwrap();
    assert_eq!(whole.notes, notes);
    assert_eq!(whole.beats, beats);
    for sizes in [
        &[1][..],
        &[2],
        &[3],
        &[7],
        &[48],
        &[100],
        &[301],
        &[1_000],
        &[1, 99, 2, 50, 17],
        &[5, 1, 1, 3],
    ] {
        let (n, b) = in_windows(&law, last, sizes);
        assert_eq!(n, notes, "windows {sizes:?}");
        assert_eq!(b, beats, "windows {sizes:?}");
    }
}

/// The two sides of a quantum boundary are read in different windows.
#[test]
fn a_boundary_splits_the_last_sample_of_a_quantum_from_the_next() {
    let mut law = law_of(&[(47, 60), (48, 61)]);
    law.step().unwrap();
    let pitches = |f: &Frames| -> Vec<u8> { f.notes.iter().map(|n| n.pitch).collect() };
    assert_eq!(pitches(&law.frames(0, 0).unwrap()), [60]);
    assert_eq!(pitches(&law.frames(1, 1).unwrap()), [61]);
    assert_eq!(pitches(&law.frames(0, 1).unwrap()), [60, 61]);
    assert_eq!(pitches(&law.frames(2, H).unwrap()), Vec::<u8>::new());
}

#[test]
fn a_frame_carries_each_voices_length_and_note() {
    let mut law = law_of(&[(1_000, 60), (2_000, 62)]);
    law.admit(&[
        TakeNote {
            onset_sample: 1_010,
            pitch: 60,
            velocity: 40,
            cites: Some(ScoreNoteId(0)),
        },
        TakeNote {
            onset_sample: 1_500,
            pitch: 90,
            velocity: 41,
            cites: None,
        },
    ])
    .unwrap();
    commit_through(&mut law, 100);
    law.live(LiveNote {
        onset_sample: 2_020,
        pitch: 62,
        velocity: 42,
    })
    .unwrap();
    law.live_off(off(62, 2_353)).unwrap();
    // Still held: its length is 0 until its note-off.
    law.live(LiveNote {
        onset_sample: 3_000,
        pitch: 77,
        velocity: 43,
    })
    .unwrap();
    let f = law.frames(0, 100).unwrap();
    let got: Vec<(u64, Voice, Option<u32>, u8, u8, u64)> = f
        .notes
        .iter()
        .map(|n| {
            (
                n.onset_sample,
                n.voice,
                n.note.map(|id| id.0),
                n.pitch,
                n.velocity,
                n.duration_samples,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (1_000, Voice::Score, Some(0), 60, 80, 1_000),
            (1_010, Voice::Take, Some(0), 60, 40, 1_000),
            (1_500, Voice::Take, None, 90, 41, 0),
            (2_000, Voice::Score, Some(1), 62, 80, 1_000),
            (2_020, Voice::Live, Some(1), 62, 42, 333),
            (3_000, Voice::Live, None, 77, 43, 0),
        ]
    );
    assert_eq!(
        [Voice::Score, Voice::Take, Voice::Live].map(Voice::code),
        [0, 1, 2]
    );
}

/// Reading frames changes nothing, and a live note's length is not hashed:
/// two live takes that differ only in length have one snapshot.
#[test]
fn frames_change_nothing_and_lengths_are_not_hashed() {
    let build = |length: i64| {
        let mut law = law_of(&[(1_000, 60)]);
        commit_through(&mut law, 1_000);
        law.live(LiveNote {
            onset_sample: 1_000,
            pitch: 60,
            velocity: 64,
        })
        .unwrap();
        law.live_off(off(60, 1_000 + length)).unwrap();
        law
    };
    let short = build(10);
    let long = build(20_000);
    assert_eq!(short.snapshot_bytes(), long.snapshot_bytes());
    assert_ne!(short.frames(0, 100), long.frames(0, 100));
    let before = short.clone();
    let _ = short.frames(0, 100).unwrap();
    let _ = short.frames(3, 7).unwrap();
    assert_eq!(short, before);
}

// --- Beats -------------------------------------------------------------------

fn beats_of(law: &Law, first: u64, last: u64) -> Vec<(u64, u64, u8, bool)> {
    law.frames(first, last)
        .unwrap()
        .beats
        .iter()
        .map(|b| (b.onset_sample, b.bar, b.beat, b.downbeat))
        .collect()
}

fn one_note_score(source_ppq: u16, tempo: &[(u64, u32)], meter: &[(u64, u8, u8)]) -> Law {
    Law::load(&IngestedScore {
        source_ppq,
        tempo: tempo
            .iter()
            .map(|&(tick, us_per_quarter)| TempoChange {
                tick,
                us_per_quarter,
            })
            .collect(),
        meter: meter
            .iter()
            .map(|&(tick, numerator, denominator_pow2)| MeterChange {
                tick,
                numerator,
                denominator_pow2,
            })
            .collect(),
        notes: vec![IngestedNote {
            start_tick: 0,
            pitch: 60,
            track: 1,
            channel: 0,
            end_tick: 10,
            velocity: 80,
        }],
    })
    .unwrap()
}

/// 2/4 at 120 BPM: a quarter-note beat every 24,000 samples, a downbeat every
/// second one.
#[test]
fn beats_follow_the_meter_and_the_tempo_map() {
    let mut law = one_note_score(480, &[(0, 500_000)], &[(0, 2, 2)]);
    commit_through(&mut law, 2_500);
    // Quantum 2,499 ends on sample 119,999, one before the sixth beat.
    assert_eq!(
        beats_of(&law, 0, 2_499),
        [
            (0, 0, 0, true),
            (24_000, 0, 1, false),
            (48_000, 1, 0, true),
            (72_000, 1, 1, false),
            (96_000, 2, 0, true),
        ]
    );
    assert_eq!(beats_of(&law, 2_500, 2_500), [(120_000, 2, 1, false)]);
    // A window that starts on a beat's quantum, and one that starts after it.
    assert_eq!(beats_of(&law, 500, 999), [(24_000, 0, 1, false)]);
    assert_eq!(beats_of(&law, 501, 999), []);
}

/// A meter change starts a bar, even mid-bar; 3/8 counts eighth-note beats.
#[test]
fn a_meter_change_starts_a_bar() {
    // 4/4 for two beats (a bar cut short), then 3/8.
    let score = scored(&[(0, 60, 10, 1)], &[(0, 4, 2), (6_720, 3, 3)]);
    let mut law = Law::load(&score).unwrap();
    commit_through(&mut law, 400);
    assert_eq!(
        beats_of(&law, 0, 400),
        [
            (0, 0, 0, true),
            (3_360, 0, 1, false),
            (6_720, 1, 0, true),
            (8_400, 1, 1, false),
            (10_080, 1, 2, false),
            (11_760, 2, 0, true),
            (13_440, 2, 1, false),
            (15_120, 2, 2, false),
            (16_800, 3, 0, true),
            (18_480, 3, 1, false),
        ]
    );
}

/// Beats land where the tempo map puts their ticks, through tempo changes and
/// a meter change, and match the beat-by-beat walk in any windows.
#[test]
fn beats_land_where_the_tempo_map_puts_them() {
    let mut law = one_note_score(
        3_360,
        &[(0, 500_000), (5_000, 433_333), (20_000, 1_000_001)],
        &[(0, 3, 2), (16_800, 7, 4)],
    );
    let last = 5_000;
    commit_through(&mut law, last);
    let (_, beats) = expected(&law, last, &[]);
    assert!(beats.len() >= 15, "{}", beats.len());
    assert!(beats.iter().any(|b| b.bar >= 3 && b.beat == 6));
    assert_eq!(law.frames(0, last).unwrap().beats, beats);
    let (_, windowed) = in_windows(&law, last, &[1, 13, 40]);
    assert_eq!(windowed, beats);
}

// --- The frame layout --------------------------------------------------------

#[test]
fn frames_round_trip_through_their_layout() {
    let mut law = law_of(&[(0, 60), (48, 64)]);
    law.admit(&[TakeNote {
        onset_sample: 3_000,
        pitch: 70,
        velocity: 5,
        cites: None,
    }])
    .unwrap();
    commit_through(&mut law, 200);
    law.live(played(100, 64)).unwrap();
    let frames = law.frames(0, 200).unwrap();
    assert_eq!((frames.notes.len(), frames.beats.len()), (4, 3));
    let bytes = wire::encode_frames(&frames).unwrap();
    assert_eq!(
        bytes.len(),
        32 + frames.notes.len() * 24 + frames.beats.len() * 18
    );
    assert_eq!(&bytes[..4], b"SJFR");
    assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
    assert_eq!(&bytes[8..16], &0u64.to_le_bytes());
    assert_eq!(&bytes[16..24], &200u64.to_le_bytes());
    assert_eq!(wire::decode_frames(&bytes), Ok(frames));
}

#[test]
fn the_frame_decoder_refuses_bad_values_at_their_offsets() {
    let mut law = law_of(&[(0, 60)]);
    law.admit(&[TakeNote {
        onset_sample: 10,
        pitch: 61,
        velocity: 5,
        cites: None,
    }])
    .unwrap();
    law.step().unwrap();
    let good = wire::encode_frames(&law.frames(0, 0).unwrap()).unwrap();
    let fault = |bytes: &[u8]| wire::decode_frames(bytes).map(|_| ());
    let wire_at = |fault, offset| Err(Refusal::Wire { fault, offset });
    // Two notes (the score's, then the take's addition) and one beat.
    let first_note = 28;
    let voice = first_note + 8;
    let second_note = first_note + 24;
    let beats = second_note + 24;
    let flag = beats + 4 + 8 + 8 + 1;
    assert_eq!(good.len(), flag + 1);

    let mut bad = good.clone();
    bad[voice] = 3;
    assert_eq!(fault(&bad), wire_at(WireFault::Voice, voice));
    // A score frame that names no note.
    let mut bad = good.clone();
    bad[voice + 1] = 0;
    bad[voice + 2..voice + 6].copy_from_slice(&[0; 4]);
    assert_eq!(fault(&bad), wire_at(WireFault::Voice, voice));
    // Tag 2, and tag 0 with an id.
    let mut bad = good.clone();
    bad[voice + 1] = 2;
    assert_eq!(fault(&bad), wire_at(WireFault::CitationTag, voice + 1));
    let mut bad = good.clone();
    bad[second_note + 10] = 7;
    assert_eq!(
        fault(&bad),
        wire_at(WireFault::CitationTag, second_note + 9)
    );
    // The downbeat flag on beat 0: 0 and 2 are refused.
    for wrong in [0, 2] {
        let mut bad = good.clone();
        bad[flag] = wrong;
        assert_eq!(fault(&bad), wire_at(WireFault::Downbeat, flag));
    }
    // Beat 1 with the downbeat flag set.
    let mut bad = good.clone();
    bad[flag - 1] = 1;
    assert_eq!(fault(&bad), wire_at(WireFault::Downbeat, flag));
    let mut bad = good.clone();
    bad[4] = 2;
    assert_eq!(fault(&bad), wire_at(WireFault::Version, 4));
    let mut bad = good.clone();
    bad.push(0);
    assert_eq!(fault(&bad), wire_at(WireFault::Trailing, good.len()));
    assert!(matches!(
        fault(&good[..good.len() - 1]),
        Err(Refusal::Wire {
            fault: WireFault::Truncated,
            ..
        })
    ));
    // A take's bytes are not frames.
    assert_eq!(
        fault(&wire::encode_take(&[]).unwrap()),
        wire_at(WireFault::Magic, 0)
    );
}

// --- Codes ---------------------------------------------------------------------

/// Every refusal has its own code, and the codes the live verb and the frame
/// export added come after every code already in use, the ingest verb's
/// included.
#[test]
fn the_new_codes_are_appended_and_distinct() {
    use score_model::ModelError;
    let refusals = [
        Refusal::Model(ModelError::ZeroPpq),
        Refusal::TooManyNotes { count: 0 },
        Refusal::InexactTick {
            event: crate::Event::Tempo(0),
            tick: 0,
            source_ppq: 1,
        },
        Refusal::TickOverflow {
            event: crate::Event::Tempo(0),
            tick: 0,
        },
        Refusal::SampleOverflow { tick: 0 },
        Refusal::SampleOutOfRange { tick: 0, sample: 0 },
        Refusal::NoScore,
        Refusal::TakeVelocity {
            index: 0,
            velocity: 0,
        },
        Refusal::TakePitch { index: 0, pitch: 0 },
        Refusal::TakeCitation {
            index: 0,
            cites: 0,
            notes: 0,
        },
        Refusal::TakeOnsetOutOfRange {
            index: 0,
            onset_sample: 0,
        },
        Refusal::TakeOrder { index: 0 },
        Refusal::Late {
            index: 0,
            quantum: 0,
            horizon: 0,
            lateness: 0,
        },
        Refusal::TakeDuplicate { index: 0 },
        Refusal::TakeTooLong { count: 0 },
        Refusal::StepOverflow { steps: 0 },
        Refusal::Wire {
            fault: WireFault::Magic,
            offset: 0,
        },
        Refusal::NullPointer,
        Refusal::OutOfMemory,
        Refusal::Overflow,
        Refusal::Busy,
        Refusal::LiveStopped,
        Refusal::LiveBeforeStart { sample: -1 },
        Refusal::LiveAhead {
            sample: 0,
            quantum: 0,
            horizon: 0,
        },
        Refusal::LivePitch { pitch: 0 },
        Refusal::LiveVelocity { velocity: 0 },
        Refusal::LiveEmpty {
            onset_sample: 0,
            off_sample: 0,
        },
        Refusal::LiveDuplicate {
            onset_sample: 0,
            pitch: 0,
            cites: None,
        },
        Refusal::LiveNotHeld { pitch: 0 },
        Refusal::FramesWindow { first: 1, last: 0 },
        Refusal::FramesStopped,
        Refusal::FramesNotCommitted {
            last: 0,
            horizon: 0,
        },
    ];
    let codes: Vec<u32> = refusals.iter().map(Refusal::code).collect();
    let mut sorted = codes.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), codes.len(), "a code names two refusals");
    let (old, new) = codes.split_at(codes.len() - 11);
    assert_eq!(new, [160, 161, 162, 163, 164, 165, 167, 168, 170, 171, 172]);
    assert!(old.iter().all(|&c| c <= 63));
    // The ingest verb's own codes run from 70 to 154.
    assert!(new.iter().all(|&c| c > 154));
    // Each new refusal says what it refused.
    for r in &refusals[refusals.len() - 11..] {
        let text = alloc::format!("{r}");
        assert!(
            text.starts_with("live note refused: ")
                || text.starts_with("live note-off refused: ")
                || text.starts_with("frames refused: "),
            "{text}"
        );
    }
}
