//! Moving committed frames into the ring, ahead of the playhead.
//!
//! The scheduler runs on a thread that is not the audio callback's, and the
//! callback never calls the law. Each [`Scheduler::pump`]:
//! 1. steps the law until its playhead is the quantum that holds the next
//!    frame the callback will render: one step per 48 samples of the audio
//!    clock;
//! 2. reads the committed frames of every quantum it has not read yet, up to
//!    the committed horizon, H = 100 quanta (100 ms) past the playhead;
//! 3. pushes their events into the ring in order, as far as the ring has room,
//!    and keeps the rest, in order, for the next pump.
//!
//! So the ring holds each committed event before its quantum is rendered, with
//! up to 100 ms of lead, and each exactly once: windows follow one another
//! with no quantum shared and none skipped.

use std::collections::VecDeque;

use law::QUANTUM_SAMPLES;
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

    /// Steps the law to the playhead of `frame`, the next frame the callback
    /// renders, reads what it newly committed, and pushes as many events into
    /// `ring` as fit (see the module documentation).
    pub fn pump(
        &mut self,
        law: &mut Law,
        frame: u64,
        ring: &mut Producer<Event>,
    ) -> Result<Pumped, Refused> {
        let mut pumped = Pumped::default();
        let target = frame / u64::from(QUANTUM_SAMPLES) + 1;
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
        Ok(pumped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::from_frames;
    use crate::score::Piece;
    use law::{HORIZON_QUANTA, QUANTUM_SAMPLES};
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
            let pumped = scheduler.pump(&mut law, frame, &mut producer).unwrap();
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
            let pumped = scheduler.pump(&mut law, frame, &mut producer).unwrap();
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
