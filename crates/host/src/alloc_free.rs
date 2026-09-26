//! The render allocates nothing: a counting global allocator watches many
//! callbacks' worth of `Synth::render`, fed from the ring with the committed
//! events of *The Entertainer* and with live notes through the monitor.
//!
//! The allocator counts every allocation, reallocation and free made by the
//! thread that is measuring, and only while it measures, so the other tests
//! running beside this one do not move the count. The negative controls show
//! the counter moves: a `Vec::push` does, and so does creating a ring, which is
//! why the host creates its rings before it builds the stream.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn note() {
    let _ = MEASURING.try_with(|on| {
        if on.get() {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        }
    });
}

// SAFETY: every call is passed straight to the system allocator, unchanged;
// the only addition is a thread-local counter that allocates nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        // SAFETY: as above.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note();
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Runs `f` and returns what it returned and how many times this thread
/// allocated, reallocated or freed while it ran.
fn counted<R>(f: impl FnOnce() -> R) -> (R, u64) {
    COUNT.with(|c| c.set(0));
    MEASURING.with(|on| on.set(true));
    let result = f();
    MEASURING.with(|on| on.set(false));
    (result, COUNT.with(Cell::get))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Law;
    use crate::event::{Event, Monitor};
    use crate::offline;
    use crate::score::{Piece, root};
    use crate::synth::Synth;
    use rtrb::RingBuffer;
    use std::hint::black_box;

    /// Thirty seconds of *The Entertainer*, its take and its clicks, with a
    /// live note held and released through the monitor, rendered in callbacks
    /// of irregular sizes (some longer than one pass): no allocation.
    #[test]
    fn the_render_allocates_nothing() {
        let piece = Piece::entertainer(&root()).unwrap();
        let events: Vec<Event> = {
            let mut law = Law::acquire();
            law.ingest(&piece.container).unwrap();
            law.admit_take(&piece.take).unwrap();
            offline::events(&mut law, 48_000 * 30).unwrap()
        };
        assert!(events.len() > 500, "{}", events.len());
        let (mut producer, mut consumer) = RingBuffer::new(events.len());
        for e in &events {
            producer.push(*e).unwrap();
        }
        let (mut live, mut monitor) = RingBuffer::new(8);
        let mut synth = Synth::new(0);
        let mut out = vec![0.0f32; 2 * 4_096];
        let sizes = [480usize, 441, 1, 1_024, 4_096, 97, 512, 2_000];

        let ((), allocations) = counted(|| {
            let mut i = 0;
            while synth.frame() < 48_000 * 30 {
                if i == 100 {
                    let _ = live.push(Monitor::On {
                        pitch: 60,
                        velocity: 80,
                    });
                }
                if i == 400 {
                    let _ = live.push(Monitor::Off { pitch: 60 });
                }
                let frames = sizes[i % sizes.len()];
                let buffer = &mut out[..2 * frames];
                synth.render(buffer, 2, &mut consumer, Some(&mut monitor));
                black_box(&buffer[0]);
                i += 1;
            }
        });
        assert_eq!(allocations, 0);
        let counts = synth.counts();
        assert_eq!(counts.notes + counts.beats, events.len() as u64);
        assert_eq!((counts.late, counts.dropped, counts.monitored), (0, 0, 1));
        assert!(out.iter().any(|s| *s != 0.0));
    }

    /// The counter counts: a `Vec` growing inside the measured span moves it,
    /// and so does making a ring, which allocates its slots.
    #[test]
    fn the_counter_moves_when_something_allocates() {
        let ((), pushes) = counted(|| {
            let mut v: Vec<u64> = Vec::new();
            black_box(&mut v);
            v.push(1);
            black_box(&v);
        });
        assert!(pushes >= 1, "{pushes}");
        let (ring, making) = counted(|| RingBuffer::<Event>::new(64));
        assert!(making >= 1, "{making}");
        let (mut producer, mut consumer) = ring;
        let ((), using) = counted(|| {
            for onset in 0..1_000 {
                let _ = producer.push(Event::Beat {
                    onset,
                    downbeat: false,
                });
                black_box(consumer.pop().ok());
            }
        });
        assert_eq!(using, 0, "pushing and popping allocates nothing");
    }
}
