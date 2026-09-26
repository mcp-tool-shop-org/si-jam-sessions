//! Moving committed frames into the ring, ahead of the playhead.
//!
//! The scheduler runs on a thread that is not the audio callback's, and the
//! callback never calls the law. Each [`Scheduler::pump`]:
//! 1. steps the law to the step count [`steps_for`] gives: one step per 48
//!    samples of the audio clock;
//! 2. reads the committed frames of every quantum it has not read yet, up to
//!    the committed horizon, H = 100 quanta (100 ms) past the playhead;
//! 3. pushes their events into the ring in order, as far as the ring has room,
//!    and keeps the rest, in order, for the next pump.
//!
//! So the ring holds each committed event before its quantum is rendered, and
//! each exactly once: windows follow one another with no quantum shared and
//! none skipped.
//!
//! # Where the playhead is
//!
//! In a jam the law's playhead is the sample being heard ([`steps_for`] with
//! the audio clock's reading), not the next frame the callback renders. The
//! law closes a score note when its playhead has passed the note's onset by
//! the reach and the delivery allowance, and a note a person plays is handed
//! over a few milliseconds after it is heard; with the playhead on what is
//! heard, that is all the allowance has to cover. The playhead never falls so
//! far behind the render that the horizon misses the frames the next callbacks
//! ask for. Without an audio clock (`render`, `play`) the playhead is the
//! render position.

use std::collections::VecDeque;

use law::{HORIZON_QUANTA, QUANTUM_SAMPLES};
use rtrb::{Producer, PushError};

use crate::bridge::{Law, Refused};
use crate::event::{Event, from_frames};

/// What one [`Scheduler::pump`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pumped {
    /// Quanta the law stepped.
    pub steps: u64,
    /// Events pushed into the ring.
    pub pushed: usize,
    /// Events read but still waiting for room in the ring.
    pub pending: usize,
    /// The law sample before which every committed event is in the ring: the
    /// first waiting event's onset, or, with none waiting, the first sample of
    /// the first quantum not yet read. The callback's pre-roll waits for it
    /// ([`crate::callback`]).
    pub covered: u64,
}

/// The non-real-time side of the host's clock.
pub struct Scheduler {
    /// The first quantum not yet read from the law.
    next: u64,
    /// Events read but not yet pushed, in order: the ring was full.
    pending: VecDeque<Event>,
    scratch: Vec<Event>,
}

impl Scheduler {
    /// A scheduler that reads from quantum `first_quantum` on.
    pub fn new(first_quantum: u64) -> Scheduler {
        Scheduler {
            next: first_quantum,
            pending: VecDeque::new(),
            scratch: Vec::new(),
        }
    }

    /// Steps the law to `target` steps ([`steps_for`]), reads what it newly
    /// committed, and pushes as many events into `ring` as fit (see the module
    /// documentation). The law never steps back.
    pub fn pump(
        &mut self,
        law: &mut Law,
        target: u64,
        ring: &mut Producer<Event>,
    ) -> Result<Pumped, Refused> {
        let mut pumped = Pumped::default();
        while law.steps() < target {
            law.step()?;
            pumped.steps += 1;
        }
        if let Some(horizon) = law.horizon()
            && self.next <= horizon
        {
            let frames = law.frames(self.next, horizon)?;
            from_frames(&frames, &mut self.scratch);
            self.pending.extend(self.scratch.drain(..));
            self.next = horizon + 1;
        }
        while let Some(event) = self.pending.pop_front() {
            match ring.push(event) {
                Ok(()) => pumped.pushed += 1,
                Err(PushError::Full(event)) => {
                    self.pending.push_front(event);
                    break;
                }
            }
        }
        pumped.pending = self.pending.len();
        pumped.covered = match self.pending.front() {
            Some(event) => event.onset(),
            None => self.next.saturating_mul(u64::from(QUANTUM_SAMPLES)),
        };
        Ok(pumped)
    }
}

/// The step count for the law, with `frame` the next frame the callback
/// renders and `lookahead` the frames the next callbacks may ask for before
/// the law thread pumps again.
///
/// - With `heard`, the law sample heard now: the playhead is the quantum that
///   holds it.
/// - Without it (no audio clock yet, or none at all): the quantum that holds
///   `frame`, the render position.
///
/// Either way, never fewer steps than put the committed horizon, playhead plus
/// H, on the quantum of the last frame the next callbacks render: `frame +
/// lookahead - 1`. So the ring always holds what the device asks for next, and
/// a device whose latency is larger than H keeps its playhead that much ahead
/// of what is heard.
pub fn steps_for(frame: u64, lookahead: u64, heard: Option<i64>) -> u64 {
    let quantum = u64::from(QUANTUM_SAMPLES);
    let at = match heard {
        Some(sample) => u64::try_from(sample).unwrap_or(0) / quantum,
        None => frame / quantum,
    };
    let needed = frame.saturating_add(lookahead.saturating_sub(1)) / quantum;
    let floor = needed
        .saturating_add(1)
        .saturating_sub(u64::from(HORIZON_QUANTA));
    at.saturating_add(1).max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::from_frames;
    use crate::score::Piece;
    use rtrb::RingBuffer;

    /// Every event of the constructed take's piece, read in one window with the
    /// law's Rust API: what the ring must carry, in order.
    fn expected(piece: &Piece, last_quantum: u64) -> Vec<Event> {
        let mut law = law::Law::ingest(&piece.container).unwrap();
        law.admit(&law::wire::decode_take(&piece.take).unwrap())
            .unwrap();
        while law
            .committed_horizon()
            .unwrap()
            .is_none_or(|h| h < last_quantum)
        {
            law.step().unwrap();
        }
        let mut out = Vec::new();
        from_frames(&law.frames(0, last_quantum).unwrap(), &mut out);
        out
    }

    /// Runs the scheduler as the host runs it, against a consumer that takes
    /// events block by block as the synth does (every event whose onset is
    /// before the block's end), through the whole piece. Returns the events in
    /// the order they left the ring, how many were taken after their block had
    /// begun, and the most that ever waited for room.
    fn run(capacity: usize, blocks: &[u64], end: u64) -> (Vec<Event>, usize, usize) {
        let piece = Piece::entertainer(&crate::score::root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        law.admit_take(&piece.take).unwrap();
        let (mut producer, mut consumer) = RingBuffer::new(capacity);
        let mut scheduler = Scheduler::new(0);
        let (mut taken, mut late, mut most_pending) = (Vec::new(), 0, 0);
        let mut frame = 0u64;
        let mut i = 0;
        while frame < end {
            let target = steps_for(frame, 0, None);
            let pumped = scheduler.pump(&mut law, target, &mut producer).unwrap();
            most_pending = most_pending.max(pumped.pending);
            let block_end = frame + blocks[i % blocks.len()];
            while let Ok(event) = consumer.peek() {
                if event.onset() >= block_end {
                    break;
                }
                if event.onset() < frame {
                    late += 1;
                }
                taken.push(consumer.pop().unwrap());
            }
            frame = block_end;
            i += 1;
        }
        // What was still waiting for room when the render reached the end.
        loop {
            let target = steps_for(frame, 0, None);
            let pumped = scheduler.pump(&mut law, target, &mut producer).unwrap();
            let mut moved = false;
            while let Ok(event) = consumer.peek() {
                if event.onset() >= frame {
                    break;
                }
                late += 1;
                taken.push(consumer.pop().unwrap());
                moved = true;
            }
            if pumped.pending == 0 && !moved {
                break;
            }
        }
        (taken, late, most_pending)
    }

    /// With the ring the host uses and device-sized blocks, every committed
    /// event of the whole piece leaves the ring once, in order, before its
    /// block begins: none lost, none doubled, none late, at any block or
    /// window boundary.
    #[test]
    fn every_committed_event_reaches_the_ring_once_and_on_time() {
        let piece = Piece::entertainer(&crate::score::root()).unwrap();
        let end = piece.end + 48_000;
        let last = end / u64::from(QUANTUM_SAMPLES);
        let all = expected(&piece, last + u64::from(HORIZON_QUANTA));
        for blocks in [&[480][..], &[441, 512, 97, 1_024, 7]] {
            let (taken, late, _) = run(crate::RING_EVENTS, blocks, end);
            let want: Vec<Event> = all.iter().copied().filter(|e| e.onset() < end).collect();
            assert!(want.len() > 2 * 2_621, "{}", want.len());
            assert_eq!(taken.len(), want.len(), "blocks {blocks:?}");
            assert!(taken == want, "blocks {blocks:?}: the events differ");
            assert_eq!(late, 0, "blocks {blocks:?}");
        }
    }

    /// Before the stream starts, the ring holds the first H + 1 quanta, and
    /// the cover says so; with events waiting for room, the cover stops at the
    /// first of them.
    #[test]
    fn the_cover_is_how_far_the_ring_is_complete() {
        let piece = Piece::entertainer(&crate::score::root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        law.admit_take(&piece.take).unwrap();
        let (mut producer, mut consumer) = RingBuffer::new(crate::RING_EVENTS);
        let mut scheduler = Scheduler::new(0);
        let pumped = scheduler.pump(&mut law, 1, &mut producer).unwrap();
        let lead = (u64::from(HORIZON_QUANTA) + 1) * u64::from(QUANTUM_SAMPLES);
        assert_eq!((pumped.pending, pumped.covered), (0, lead));
        while consumer.pop().is_ok() {}

        // A ring with room for the events at sample 0 only: the first event
        // after them waits, and the cover is its onset.
        let all = expected(&piece, 10_000 + u64::from(HORIZON_QUANTA));
        let at_zero = all.iter().position(|e| e.onset() > 0).unwrap();
        let (mut producer, _consumer) = RingBuffer::new(at_zero);
        let mut scheduler = Scheduler::new(0);
        let target = steps_for(480_000, 0, None);
        let pumped = scheduler.pump(&mut law, target, &mut producer).unwrap();
        assert_eq!(pumped.pending, all.len() - at_zero);
        assert_eq!(pumped.covered, all[at_zero].onset());
        assert!(pumped.covered > 0);
    }

    /// The step count: the render position without an audio clock; the heard
    /// sample's quantum with one; and never so few steps that the horizon
    /// misses the frames the next callbacks render.
    #[test]
    fn the_playhead_follows_what_is_heard_within_the_horizon() {
        let h = u64::from(HORIZON_QUANTA);
        assert_eq!(steps_for(48_000, 2_112, None), 1_001, "the render position");
        let frame = 480_000u64;
        // A wired output, heard 53.8 ms (2,582 samples) behind the render.
        let heard = frame as i64 - 2_582;
        assert_eq!(steps_for(frame, 2_112, Some(heard)), heard as u64 / 48 + 1);
        // A Bluetooth output, heard 186.4 ms (8,948 samples) behind, more than
        // H: the horizon reaches the last frame of the next two callbacks.
        let heard = frame as i64 - 8_948;
        let steps = steps_for(frame, 2 * 1_124, Some(heard));
        assert_eq!(steps - 1 + h, (frame + 2 * 1_124 - 1) / 48);
        assert!(steps > heard as u64 / 48 + 1);
        // Before sample 0 is heard: one step.
        assert_eq!(steps_for(1_056, 2_112, Some(-1_526)), 1);
    }

    /// With the playhead on the heard sample, the ring still holds every
    /// event before its block begins, on a wired output (heard 53.8 ms behind
    /// the render, 1,056-frame callbacks) and on a Bluetooth one (186.4 ms,
    /// more than H, 1,124-frame callbacks), pumped between callbacks with the
    /// next two callbacks as lookahead, as `jam` pumps. On the wired output the
    /// playhead is the heard quantum; on the Bluetooth one it is ahead of it
    /// only as far as the horizon needs.
    #[test]
    fn a_playhead_on_the_heard_clock_still_fills_the_ring_in_time() {
        let piece = Piece::entertainer(&crate::score::root()).unwrap();
        let end = 48_000 * 20;
        let all = expected(&piece, end / u64::from(QUANTUM_SAMPLES) + 200);
        let want: Vec<Event> = all.iter().copied().filter(|e| e.onset() < end).collect();
        for (latency, block) in [(2_582u64, 1_056u64), (8_948, 1_124)] {
            let mut law = Law::acquire();
            law.ingest(&piece.container).unwrap();
            law.admit_take(&piece.take).unwrap();
            let (mut producer, mut consumer) = RingBuffer::new(crate::RING_EVENTS);
            let mut scheduler = Scheduler::new(0);
            let (mut taken, mut late, mut lead) = (Vec::new(), 0, 0i64);
            let mut frame = 0u64;
            while frame < end {
                let heard = frame as i64 - latency as i64;
                let target = steps_for(frame, 2 * block, Some(heard));
                scheduler.pump(&mut law, target, &mut producer).unwrap();
                if heard >= 0 {
                    let playhead = ((law.steps() - 1) * 48) as i64;
                    lead = lead.max(playhead - heard);
                }
                while let Ok(event) = consumer.peek() {
                    if event.onset() >= frame + block {
                        break;
                    }
                    if event.onset() < frame {
                        late += 1;
                    }
                    taken.push(consumer.pop().unwrap());
                }
                frame += block;
            }
            taken.retain(|e| e.onset() < end);
            assert_eq!(late, 0, "latency {latency}");
            assert!(taken == want, "latency {latency}: the events differ");
            let beyond = (latency + 2 * block) as i64 - (u64::from(HORIZON_QUANTA) * 48) as i64;
            assert!(lead <= beyond.max(0) + 48, "latency {latency}: lead {lead}");
            if beyond < 0 {
                assert!(lead <= 0, "latency {latency}: lead {lead}");
            }
        }
    }

    /// A ring too small for a chord: the scheduler keeps what does not fit and
    /// pushes it as room frees, so nothing is lost or doubled and the order
    /// holds; the events that could not fit in time arrive late, and are
    /// counted as late.
    #[test]
    fn a_full_ring_holds_events_back_without_losing_or_doubling_one() {
        let piece = Piece::entertainer(&crate::score::root()).unwrap();
        let end = 48_000 * 20;
        let all = expected(&piece, end / u64::from(QUANTUM_SAMPLES) + 200);
        let want: Vec<Event> = all.iter().copied().filter(|e| e.onset() < end).collect();
        let (taken, _late, most_pending) = run(3, &[480], end);
        assert!(most_pending > 0, "the full path was taken");
        assert_eq!(taken, want);
    }
}
