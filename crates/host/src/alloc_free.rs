//! The audio callback allocates nothing: a counting global allocator watches
//! many calls of its body, `Callback::run`, which renders through
//! `Synth::render`, fed from the ring with the committed events of *The
//! Entertainer* and with live notes through the monitor, after a first fill
//! that the silent pre-roll renders.
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
    use crate::callback::{Callback, Shared};
    use crate::event::{Event, Monitor};
    use crate::offline;
    use crate::score::{Piece, root};
    use crate::synth::Synth;
    use rtrb::RingBuffer;
    use std::hint::black_box;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    /// Thirty seconds of *The Entertainer*, its take and its clicks, with a
    /// live note held and released through the monitor, through the callback's
    /// body in callbacks of irregular sizes (some longer than one pass), after
    /// a first fill longer than the ring's cover, which the pre-roll renders as
    /// silence: no allocation.
    #[test]
    fn the_callback_allocates_nothing() {
        let piece = Piece::entertainer(&root()).unwrap();
        let events: Vec<Event> = {
            let mut law = Law::acquire();
            law.ingest(&piece.container).unwrap();
            law.admit_take(piece.take.as_deref().unwrap()).unwrap();
            offline::events(&mut law, 48_000 * 30).unwrap()
        };
        assert!(events.len() > 500, "{}", events.len());
        let (mut producer, consumer) = RingBuffer::new(events.len());
        for e in &events {
            producer.push(*e).unwrap();
        }
        let (mut live, monitor) = RingBuffer::new(8);
        let (mut hush_in, hush) = RingBuffer::new(8);
        let (readings, _readings_out) = RingBuffer::new(8_192);
        let shared = Arc::new(Shared::default());
        // The cover at the start, law samples 0..4,848.
        shared.covered.store(4_848, Ordering::Release);
        let mut callback = Callback::new(
            Synth::new(0),
            consumer,
            Some(monitor),
            readings,
            Arc::clone(&shared),
            false,
        )
        .with_hush(hush);
        let mut out = vec![0.0f32; 2 * 8_192];
        let sizes = [480usize, 441, 1, 1_024, 4_096, 97, 512, 2_000];

        let ((), allocations) = counted(|| {
            // The first fill, past the cover: the pre-roll's silence.
            callback.run(&mut out, 2, 1_000_000, Some(10_000_000));
            let mut i = 0;
            while shared.frames.load(Ordering::Acquire) < 48_000 * 30 {
                if i == 100 {
                    let _ = live.push(Monitor::On {
                        pitch: 60,
                        velocity: 80,
                    });
                }
                if i == 400 {
                    let _ = live.push(Monitor::Off { pitch: 60 });
                }
                if i == 500 {
                    let _ = live.push(Monitor::On {
                        pitch: 62,
                        velocity: 80,
                    });
                }
                // The law thread's note-off, through the second ring.
                if i == 600 {
                    let _ = hush_in.push(Monitor::Off { pitch: 62 });
                }
                let frames = sizes[i % sizes.len()];
                let buffer = &mut out[..2 * frames];
                callback.run(buffer, 2, 2_000_000 + i as u64, Some(10_000_000));
                black_box(&buffer[0]);
                i += 1;
            }
        });
        assert_eq!(allocations, 0);
        assert_eq!(shared.preroll_frames.load(Ordering::Relaxed), 8_192);
        let count = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
        assert_eq!(
            count(&shared.notes) + count(&shared.beats),
            events.len() as u64
        );
        assert_eq!(
            (
                count(&shared.late),
                count(&shared.dropped),
                count(&shared.monitored)
            ),
            (0, 0, 2)
        );
        assert!(out.iter().any(|s| *s != 0.0));
    }

    /// The same, with the score on the piano: thirty seconds of *The
    /// Entertainer*'s committed events through the callback's body, every
    /// score note started on a sampled key, pitched between keys, released
    /// with its hammer noise, some voices ending partway through a pass, with
    /// the take, the click and a live note beside them: no allocation. The
    /// samples are the synthetic fixture's, long enough to sound through each
    /// note, and were decoded before the measured span, as a host loads them
    /// before its stream.
    #[test]
    fn the_callback_with_the_piano_allocates_nothing() {
        use crate::fixture;
        use crate::piano::{File, Needs};

        let piece = Piece::entertainer(&root()).unwrap();
        let events: Vec<Event> = {
            let mut law = Law::acquire();
            law.ingest(&piece.container).unwrap();
            law.admit_take(piece.take.as_deref().unwrap()).unwrap();
            offline::events(&mut law, 48_000 * 30).unwrap()
        };
        let mut needs = Needs::default();
        for n in piece.score.notes() {
            needs.note(n.pitch, n.velocity);
        }
        let bank = std::sync::Arc::new(fixture::bank(&needs, |file| match file {
            File::Note { .. } => 24_000,
            File::Release { .. } => 480,
        }));
        let (mut producer, consumer) = RingBuffer::new(events.len());
        for e in &events {
            producer.push(*e).unwrap();
        }
        let (mut live, monitor) = RingBuffer::new(8);
        let (readings, _readings_out) = RingBuffer::new(8_192);
        let shared = Arc::new(Shared::default());
        shared.covered.store(4_848, Ordering::Release);
        let mut callback = Callback::new(
            Synth::new(0).with_piano(bank),
            consumer,
            Some(monitor),
            readings,
            Arc::clone(&shared),
            false,
        );
        let mut out = vec![0.0f32; 2 * 8_192];
        let sizes = [480usize, 441, 1, 1_024, 4_096, 97, 512, 2_000];

        let ((), allocations) = counted(|| {
            callback.run(&mut out, 2, 1_000_000, Some(10_000_000));
            let mut i = 0;
            while shared.frames.load(Ordering::Acquire) < 48_000 * 30 {
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
                callback.run(buffer, 2, 2_000_000 + i as u64, Some(10_000_000));
                black_box(&buffer[0]);
                i += 1;
            }
        });
        assert_eq!(allocations, 0);
        let count = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
        assert_eq!(
            count(&shared.notes) + count(&shared.beats),
            events.len() as u64
        );
        assert_eq!((count(&shared.late), count(&shared.dropped)), (0, 0));
        assert!(out.iter().any(|s| *s != 0.0));
    }

    /// The piano's negative control: a sampler that decoded its samples as it
    /// needed them, in the callback, would allocate. Decoding one fixture
    /// sample, or making a piano's voices, moves the counter.
    #[test]
    fn decoding_a_sample_or_making_a_piano_allocates() {
        use crate::fixture;
        use crate::piano::{Bank, File, Needs, decode};
        use crate::sampler::Piano;

        let file = File::Note { zone: 13, layer: 7 };
        let bytes = fixture::sample_bytes(file, 480);
        let (sample, decoding) = counted(|| decode(&file.name(), &bytes));
        assert!(sample.is_ok());
        assert!(decoding >= 1, "{decoding}");
        let bank = std::sync::Arc::new(Bank::load(&Needs::default(), |_| Ok(Vec::new())).unwrap());
        let (piano, making) = counted(|| Piano::new(bank));
        black_box(&piano);
        assert!(making >= 1, "{making}");
    }

    /// The WinMM callback's work after its clock read, for a thousand
    /// note-ons and note-offs and a thousand clock ticks, into rings too small
    /// for them all: no allocation. Each note message is delivered while there
    /// is room, a clock tick is dropped, and a full ring drops the rest.
    #[cfg(windows)]
    #[test]
    fn the_midi_callback_allocates_nothing() {
        use crate::live::{Dropped, Press, Stamp};
        let (mut monitor, mut heard) = RingBuffer::new(256);
        let (mut input, mut presses) = RingBuffer::new(512);
        let dropped = Dropped::default();
        let ((), allocations) = counted(|| {
            for i in 0..1_000usize {
                let status = if i % 2 == 0 { 0x90 } else { 0x80 };
                let message = status | (60 << 8) | (100 << 16);
                crate::winmm::deliver(&mut monitor, &mut input, &dropped, 7_000, message, i);
                crate::winmm::deliver(&mut monitor, &mut input, &dropped, 7_000, 0xF8, i);
            }
        });
        assert_eq!(allocations, 0);
        assert_eq!((heard.slots(), presses.slots()), (256, 512));
        // The rings were full for the rest: every note message past them is
        // counted, and a clock tick is not a note message.
        assert_eq!(dropped.counts(), (1_000 - 512, 1_000 - 256));
        assert_eq!(
            heard.pop(),
            Ok(Monitor::On {
                pitch: 60,
                velocity: 100
            })
        );
        let second = presses.pop().and_then(|_| presses.pop());
        assert_eq!(
            second,
            Ok(Press {
                down: false,
                pitch: 60,
                velocity: 100,
                stamp: Stamp::Midi {
                    micros: 1_000,
                    arrived: 7_000
                }
            })
        );
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
