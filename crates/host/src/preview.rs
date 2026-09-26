//! `host preview`: a MIDI file rendered straight through the piano, to
//! audition an arrangement draft.
//!
//! **A preview is not the law's committed frames.** The product's path is the
//! law, then the frames it commits, then the host: a score is ingested under
//! the licence predicate, the law steps and commits it, and the host plays
//! what was committed ([`crate::schedule`]). A preview takes none of that
//! path. It reads the file with `ingest`, the law's own SMF reader, turns each
//! note's ticks into samples, and plays every note on the piano. No receipt is
//! read, no licence is judged, nothing is stepped, committed or hashed, and
//! the file it writes says it is a preview. It is for listening to a draft
//! before it is a score.
//!
//! - **Time.** A tick becomes a sample as the law's tempo map does it
//!   (`floor(N / D)` of the exact rational sum, the remainder carried across
//!   tempo changes, in `u128`), with `D` the file's own PPQ times 10^6: a
//!   preview does not rescale to PPQ 3,360, so a tick the law would refuse as
//!   inexact still plays.
//! - **Notes.** As `ingest` pairs them. Its refusals stand: a file it refuses
//!   is not previewed.
//! - **No pedal.** Sustain-pedal messages (controller 64) are counted and
//!   ignored, as the law ignores them: a note lasts from its note-on to its
//!   note-off.
//! - **No click.** A preview plays the notes alone.

use std::sync::Arc;

use rtrb::RingBuffer;
use score_model::IngestedScore;

use crate::RING_EVENTS;
use crate::event::{Event, Voice};
use crate::piano::{Bank, HIGHEST, LOWEST, Needs};
use crate::synth::{Counts, RATE, Synth};

/// How long a preview runs past its last note's end at most, for the
/// releases to die away: 6 s, longer than the undamped keys' 5 s release.
pub const TAIL: u64 = 6 * RATE as u64;

/// A MIDI file, read for a preview.
#[derive(Debug)]
pub struct Draft {
    /// Every note, as a note-on for the piano, in onset order.
    pub events: Vec<Event>,
    /// The file's ticks per quarter note.
    pub ppq: u16,
    /// The tempo changes, the one at tick 0 among them, and the first tempo.
    pub tempos: usize,
    pub first_us_per_quarter: u32,
    /// Sustain-pedal messages, which are ignored.
    pub pedal: usize,
    /// Notes off the piano's keyboard, which are not played.
    pub outside: usize,
    /// The sample after the last note ends.
    pub end: u64,
    /// The samples the notes need.
    pub needs: Needs,
}

/// Reads a MIDI file for a preview, or says why it cannot.
pub fn read(bytes: &[u8]) -> Result<Draft, String> {
    let score =
        ingest::ingest_smf(bytes).map_err(|e| format!("the SMF reader refused it: {e:?}"))?;
    let clock = Clock::new(&score)?;
    let mut events = Vec::with_capacity(score.notes.len());
    let mut needs = Needs::default();
    let (mut outside, mut end) = (0usize, 0u64);
    for note in &score.notes {
        let onset = clock.sample(note.start_tick)?;
        let off = clock.sample(note.end_tick)?;
        end = end.max(off);
        if !(LOWEST..=HIGHEST).contains(&note.pitch) {
            outside += 1;
        }
        needs.note(note.pitch, note.velocity);
        events.push(Event::Note {
            onset,
            voice: Voice::Score,
            pitch: note.pitch,
            velocity: note.velocity,
            duration: off - onset,
        });
    }
    events.sort_by_key(|e| e.onset());
    Ok(Draft {
        events,
        ppq: score.source_ppq,
        tempos: score.tempo.len(),
        first_us_per_quarter: score.tempo.first().map_or(0, |t| t.us_per_quarter),
        pedal: pedal_messages(bytes),
        outside,
        end,
        needs,
    })
}

/// The sustain-pedal messages in a file `ingest` has read, so a plain,
/// metrical SMF, which midly parses without the header it would panic on.
fn pedal_messages(bytes: &[u8]) -> usize {
    let Ok((_, tracks)) = midly::parse(bytes) else {
        return 0;
    };
    tracks
        .flatten()
        .flat_map(|events| events.flatten())
        .filter(|e| {
            matches!(
                e.kind,
                midly::TrackEventKind::Midi {
                    message: midly::MidiMessage::Controller { controller, .. },
                    ..
                } if controller.as_int() == 64
            )
        })
        .count()
}

/// Ticks to samples, exactly, at the file's own PPQ.
struct Clock {
    /// Each tempo change: (tick, microseconds per quarter, whole samples to
    /// it, the remainder carried into it).
    segments: Vec<(u64, u32, u64, u128)>,
    denominator: u128,
}

impl Clock {
    fn new(score: &IngestedScore) -> Result<Clock, String> {
        let denominator = u128::from(score.source_ppq) * 1_000_000;
        if denominator == 0 {
            return Err(String::from("a PPQ of 0"));
        }
        let mut clock = Clock {
            segments: Vec::with_capacity(score.tempo.len()),
            denominator,
        };
        for change in &score.tempo {
            let (sample, remainder) = match clock.segments.last() {
                None => (0, 0),
                Some(_) => clock.position(change.tick)?,
            };
            clock
                .segments
                .push((change.tick, change.us_per_quarter, sample, remainder));
        }
        Ok(clock)
    }

    /// `(floor(N(tick) / D), N(tick) mod D)`.
    fn position(&self, tick: u64) -> Result<(u64, u128), String> {
        let index = self.segments.partition_point(|s| s.0 <= tick);
        let &(start, us, sample, remainder) = index
            .checked_sub(1)
            .and_then(|i| self.segments.get(i))
            .ok_or_else(|| String::from("no tempo at tick 0"))?;
        let numerator = u128::from(tick - start) * u128::from(us) * u128::from(RATE) + remainder;
        let whole = u64::try_from(numerator / self.denominator)
            .ok()
            .and_then(|w| sample.checked_add(w))
            .ok_or_else(|| format!("tick {tick} is past the last sample a u64 holds"))?;
        Ok((whole, numerator % self.denominator))
    }

    fn sample(&self, tick: u64) -> Result<u64, String> {
        self.position(tick).map(|(sample, _)| sample)
    }
}

/// Renders a draft on the piano: stereo frames, interleaved, from sample 0
/// until every voice has died away after the last note (at most [`TAIL`]
/// past its end), and what the synth counted.
pub fn render(draft: &Draft, bank: Arc<Bank>) -> (Vec<f32>, Counts) {
    const BLOCK: u64 = 480;
    let mut synth = Synth::new(0).with_piano(bank);
    let (mut producer, mut consumer) = RingBuffer::new(RING_EVENTS);
    let mut pending = draft.events.iter().peekable();
    let mut out = Vec::new();
    let mut block = [0.0f32; 2 * BLOCK as usize];
    loop {
        let end = synth.frame() + BLOCK;
        while let Some(&&event) = pending.peek() {
            if event.onset() >= end || producer.push(event).is_err() {
                break;
            }
            pending.next();
        }
        synth.render(&mut block, 2, &mut consumer, None);
        out.extend_from_slice(&block);
        let done = pending.peek().is_none() && synth.frame() >= draft.end;
        if (done && synth.sounding() == 0) || synth.frame() >= draft.end + TAIL {
            break;
        }
    }
    (out, synth.counts())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::piano::File;

    /// A format-0 SMF at `ppq` of `events`: (delta, bytes) pairs, a tempo of
    /// `us` per quarter first.
    fn smf(ppq: u16, us: u32, events: &[(u32, &[u8])]) -> Vec<u8> {
        let mut track = Vec::new();
        let tempo = us.to_be_bytes();
        track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, tempo[1], tempo[2], tempo[3]]);
        for (delta, bytes) in events {
            // Variable-length delta.
            let mut v = vec![u8::try_from(delta & 0x7F).unwrap()];
            let mut d = delta >> 7;
            while d > 0 {
                v.insert(0, 0x80 | u8::try_from(d & 0x7F).unwrap());
                d >>= 7;
            }
            track.extend_from_slice(&v);
            track.extend_from_slice(bytes);
        }
        track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        let mut out = Vec::new();
        out.extend_from_slice(b"MThd");
        out.extend_from_slice(&6u32.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&ppq.to_be_bytes());
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&u32::try_from(track.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&track);
        out
    }

    /// Notes land on the samples the law's formula gives at the file's PPQ:
    /// at 384 ticks and 500,000 us a quarter, tick 96 is exactly 6,000
    /// samples, tick 1 is 62.5 and floors to 62. The sustain pedal is counted
    /// and does not lengthen a note, and a key off the keyboard is counted.
    #[test]
    fn notes_land_on_the_laws_samples_and_the_pedal_is_ignored() {
        let bytes = smf(
            384,
            500_000,
            &[
                (0, &[0xB0, 64, 127]),
                (1, &[0x90, 60, 100]),
                (95, &[0x80, 60, 0]),
                (0, &[0x90, 110, 80]),
                (96, &[0x80, 110, 0]),
                (0, &[0xB0, 64, 0]),
            ],
        );
        let draft = read(&bytes).unwrap();
        assert_eq!(
            (draft.ppq, draft.tempos, draft.pedal, draft.outside),
            (384, 1, 2, 1)
        );
        assert_eq!(
            draft.events,
            [
                Event::Note {
                    onset: 62,
                    voice: Voice::Score,
                    pitch: 60,
                    velocity: 100,
                    duration: 6_000 - 62,
                },
                Event::Note {
                    onset: 6_000,
                    voice: Voice::Score,
                    pitch: 110,
                    velocity: 80,
                    duration: 6_000,
                },
            ]
        );
        assert_eq!(draft.end, 12_000);
        assert_eq!(draft.needs.files().len(), 2, "C4's sample and its release");
    }

    /// At PPQ 3,360 the preview's clock is the law's tempo map, sample for
    /// sample, across tempo changes whose remainders carry.
    #[test]
    fn at_the_laws_ppq_the_clock_is_the_laws() {
        let tempo = [
            (0u64, 500_000u32),
            (1_001, 433_333),
            (7_777, 1_234_567),
            (20_000, 3),
        ];
        let score = IngestedScore {
            source_ppq: 3_360,
            tempo: tempo
                .iter()
                .map(|&(tick, us_per_quarter)| score_model::TempoChange {
                    tick,
                    us_per_quarter,
                })
                .collect(),
            meter: Vec::new(),
            notes: Vec::new(),
        };
        let ours = Clock::new(&score).unwrap();
        let law = law::TempoMap::new(&tempo).unwrap();
        for tick in (0..30_000u64)
            .step_by(7)
            .chain([1_001, 7_777, 20_000, 29_999])
        {
            assert_eq!(
                ours.sample(tick).unwrap(),
                law.sample_at(tick).unwrap(),
                "tick {tick}"
            );
        }
    }

    /// A preview renders its notes on the piano and runs until they have died
    /// away; the same draft renders to the same bits twice.
    #[test]
    fn a_preview_renders_on_the_piano_to_the_same_bits_twice() {
        let bytes = smf(
            480,
            600_000,
            &[
                (0, &[0x90, 60, 90]),
                (0, &[0x90, 64, 70]),
                (240, &[0x80, 60, 0]),
                (0, &[0x80, 64, 0]),
                (0, &[0x90, 67, 110]),
                (480, &[0x80, 67, 0]),
            ],
        );
        let draft = read(&bytes).unwrap();
        let bank = Arc::new(fixture::bank(&draft.needs, |file| match file {
            File::Note { .. } => 30_000,
            File::Release { .. } => 96,
        }));
        let (first, counts) = render(&draft, Arc::clone(&bank));
        let (second, _) = render(&draft, bank);
        assert_eq!(counts.notes, 3);
        assert_eq!((counts.late, counts.dropped, counts.outside), (0, 0, 0));
        assert!(first == second, "bit for bit");
        assert_eq!(first.len() % 2, 0);
        // The first note's first frame is at sample 0, and the piece rings
        // on after its last note ends at 43,200 samples until it dies away.
        assert_ne!(first[0], 0.0);
        let frames = first.len() / 2;
        assert!(
            frames > 43_200 && frames <= 43_200 + TAIL as usize + 480,
            "{frames}"
        );
    }

    /// The piano renders to the same bits on every machine: a pinned
    /// SHA-256 of a fixture render, which CI checks on Linux and on Windows.
    /// The sampler uses only IEEE +, -, × and ÷, so no platform's libm can
    /// move a bit.
    #[test]
    fn the_piano_renders_the_same_bits_on_every_machine() {
        let bytes = smf(
            480,
            500_000,
            &[
                (0, &[0x90, 21, 1]),
                (0, &[0x90, 59, 64]),
                (0, &[0x90, 61, 127]),
                (120, &[0x90, 89, 90]),
                (240, &[0x80, 21, 0]),
                (0, &[0x80, 59, 0]),
                (0, &[0x80, 89, 0]),
                (480, &[0x80, 61, 0]),
            ],
        );
        let draft = read(&bytes).unwrap();
        let bank = Arc::new(fixture::bank(&draft.needs, |file| match file {
            File::Note { .. } => 20_000,
            File::Release { .. } => 480,
        }));
        let (samples, counts) = render(&draft, bank);
        assert_eq!(counts.notes, 4);
        let raw: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(
            golden::run::hex(&golden::run::sha256(&raw)),
            "36aeb7e06b89825917a6d53154249437daea42749bab8dbbea71c56dea198f75"
        );
    }

    /// The real piano alone: The Entertainer's score notes, no take and no
    /// click, rendered on the samples in `SI_JAM_PIANO`. Its SHA-256 is
    /// printed to compare across machines; CI's dispatch-only piano job
    /// prints it on Linux.
    #[test]
    #[ignore = "needs the real samples: SI_JAM_PIANO=<dir>"]
    fn the_real_piano_alone_prints_its_hash() {
        use crate::score::{Piece, root};
        let dir = std::env::var_os("SI_JAM_PIANO").expect("SI_JAM_PIANO names the samples");
        let piece = Piece::entertainer(&root()).unwrap();
        let mut needs = Needs::default();
        let mut events = Vec::new();
        let mut end = 0;
        for n in piece.score.notes() {
            needs.note(n.pitch, n.velocity);
            end = end.max(n.onset_sample + n.duration_samples);
            events.push(Event::Note {
                onset: n.onset_sample,
                voice: Voice::Score,
                pitch: n.pitch,
                velocity: n.velocity,
                duration: n.duration_samples,
            });
        }
        let bank = Arc::new(Bank::open(std::path::Path::new(&dir), &needs).unwrap());
        let draft = Draft {
            events,
            ppq: 3_360,
            tempos: 1,
            first_us_per_quarter: 0,
            pedal: 0,
            outside: 0,
            end,
            needs,
        };
        let (samples, counts) = render(&draft, bank);
        assert_eq!((counts.notes, counts.dropped), (2_621, 0));
        let raw: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        eprintln!(
            "the real piano alone: {} frames, SHA-256 {}",
            samples.len() / 2,
            golden::run::hex(&golden::run::sha256(&raw))
        );
    }

    /// A file the SMF reader refuses is not previewed.
    #[test]
    fn a_file_the_reader_refuses_is_not_previewed() {
        let e = read(b"RIFF").err().unwrap();
        assert!(e.starts_with("the SMF reader refused it"), "{e}");
    }
}
