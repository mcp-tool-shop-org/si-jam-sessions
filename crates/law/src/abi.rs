//! The C ABI of the wasm law.
//!
//! Every export is `#[unsafe(no_mangle)] extern "C"`. Every argument and result
//! is 64 bits or narrower; a 128-bit value would cross as two halves, and none
//! crosses today. Every export that can refuse returns a status: `0` for done,
//! otherwise [`Refusal::code`]. The reason for the most recent refusal is
//! readable through [`law_refusal_ptr`] and [`law_refusal_len`]; a call that
//! succeeds leaves it in place, so freeing the input buffer after a refused
//! load does not erase why it was refused. Status codes cross the
//! boundary and panics do not: the law's own code is written without a
//! reachable panic, and the profiles set `panic = "abort"`, so a panic that
//! did happen would be an `unreachable` trap in wasm, never an unwind into the
//! host.
//!
//! The law has no clock, no file and no device. The host passes bytes in and
//! reads bytes out, and the module imports nothing (`tests/wasm_artifact.rs`).
//!
//! # Passing bytes in
//!
//! 1. `law_alloc(len)` returns a pointer to `len` bytes of linear memory (null
//!    when `len` is 0 or the allocation fails).
//! 2. The host writes the bytes there: a container, a score or a take in the
//!    layout of [`crate::wire`].
//! 3. `law_ingest(ptr, len)`, `law_load_score(ptr, len)` or
//!    `law_admit_take(ptr, len)`.
//! 4. `law_free(ptr, len)` returns the buffer.
//!
//! `law_ingest` is the ingest verb: the container holds a receipt and every
//! file it receipts, and the licence predicate admits the score before the SMF
//! reader reads it ([`Law::ingest`]). `law_load_score` takes a score already in
//! the law's wire layout, which no receipt admitted; its snapshot says so.
//!
//! # Reading bytes out
//!
//! `law_snapshot()` builds the snapshot, its SHA-256 and the rows. Then
//! `law_snapshot_ptr/len`, `law_hash_ptr` (32 bytes) and `law_rows_ptr/len`
//! (the rows joined by line feeds) read them. A pointer is valid until the
//! next call that loads, ingests, admits, passes a live note-on or note-off,
//! steps a live take's record forward (below), snapshots or builds frames.
//! Any call may grow linear memory, which detaches a JavaScript host's views,
//! so a host re-creates its views after each call and copies bytes out before
//! the next one.
//!
//! Every call that changes the record clears the snapshot, the hash and the
//! rows, so a stale hash is never read as current: loading or ingesting a
//! score, admitting a take, a live note-on or note-off, and, in a live take, a
//! step that makes a row final (a score note closes, or an addition becomes
//! final). The first step with the take empty clears them too: it makes the
//! take live and withdraws the rows a stopped law shows for an empty take,
//! which were never committed. Any other step leaves them, because it does not
//! change the record; a step never changes a proposed take's record.
//!
//! # The transport, live notes and frames
//!
//! `law_step()` steps one quantum and `law_horizon()` reads the committed
//! horizon. `law_live_note(onset, pitch, velocity)` and
//! `law_live_note_off(pitch, off)` are the live verbs ([`Law::live`] and
//! [`Law::live_off`]): a note a person played, passed as scalars when its key
//! goes down and comes up, admitted as a record at its own onset and never
//! refused for lateness. `law_frames(first, last)` builds the committed frames
//! of the quanta `first..=last` ([`Law::frames`], in the layout of
//! [`crate::wire::decode_frames`]), and `law_frames_ptr/len` read them.
//! Loading, ingesting, admitting and the live verbs clear the frames as they
//! clear the snapshot, because a live note can land in a quantum that was
//! already read. A step leaves the frames: it changes no committed frame.
//!
//! These verbs came with law version 4 (`law_version()`); version 3 has none
//! of them. In a live take (the transport started with the take empty),
//! `law_rows` and the snapshot hold only verdicts that can no longer change,
//! in the order they became final ([`Law::verdicts`]).
//!
//! # Threads
//!
//! The exports share one state. wasm32-unknown-unknown has no threads, but a
//! native caller might: a call that arrives while another is running is
//! refused with [`Refusal::Busy`] (a getter answers null or 0), so two calls
//! never touch the state at once.
//!
//! The same flag fences off a broken instance. If a call ever traps inside the
//! law, the flag stays set, and the instance refuses every later call with
//! `Busy` instead of running on half-updated state; the host drops it and
//! instantiates the module again.

// This module is the one place the law uses `unsafe`: exports, the state they
// share, and the host's bytes.
#![allow(unsafe_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::law::digest;
use crate::refusal::{IngestRefusal, Refusal};
use crate::{LAW_VERSION, Law, LiveNote, LiveNoteOff, wire};

struct State {
    law: Option<Law>,
    snapshot: Vec<u8>,
    hash: [u8; 32],
    rows: Vec<u8>,
    refusal: Vec<u8>,
    frames: Vec<u8>,
}

impl State {
    /// Clears what `law_snapshot` builds.
    fn clear_outputs(&mut self) {
        self.snapshot.clear();
        self.rows.clear();
        self.hash = [0; 32];
    }

    /// Clears every output, after a call that changed the record: the
    /// snapshot, the hash, the rows and the frames.
    fn record_changed(&mut self) {
        self.clear_outputs();
        self.frames.clear();
    }
}

/// The state the exports share, and the flag that makes access to it
/// exclusive.
struct Shared {
    busy: AtomicBool,
    state: UnsafeCell<State>,
}

// SAFETY: `state` is reached only through `with_state`, which holds `busy` for
// the whole access, so no two threads ever reach it at once; and `State` is
// `Send`, so handing it from one thread's call to the next is sound.
unsafe impl Sync for Shared {}

const _: () = {
    const fn send<T: Send>() {}
    send::<State>();
};

static SHARED: Shared = Shared {
    busy: AtomicBool::new(false),
    state: UnsafeCell::new(State {
        law: None,
        snapshot: Vec::new(),
        hash: [0; 32],
        rows: Vec::new(),
        refusal: Vec::new(),
        frames: Vec::new(),
    }),
};

/// Runs `f` with the only reference to the state, or returns `None` when
/// another call holds it. The flag is released when `f` returns; a panic in `f`
/// aborts (`panic = "abort"`), so no unwind can leave it set and the state
/// reachable.
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    if SHARED.busy.swap(true, Ordering::Acquire) {
        return None;
    }
    // SAFETY: `busy` was false and is now true, so no other reference to the
    // state exists until it is released below, and none outlives `f`.
    let state = unsafe { &mut *SHARED.state.get() };
    let result = f(state);
    SHARED.busy.store(false, Ordering::Release);
    Some(result)
}

/// A status export: `f`'s status, or [`Refusal::Busy`]'s code.
fn with_status(f: impl FnOnce(&mut State) -> u32) -> u32 {
    with_state(f).unwrap_or(Refusal::Busy.code())
}

/// Records the outcome of an export. A refusal replaces the stored reason and
/// returns its code; a success returns 0 and leaves the reason as it was.
fn status(state: &mut State, result: Result<(), Refusal>) -> u32 {
    match result {
        Ok(()) => 0,
        Err(refusal) => refused(state, refusal.code(), &refusal),
    }
}

/// [`status`] for the ingest verb, whose refusals carry the licence
/// predicate's and the SMF reader's.
fn ingest_status(state: &mut State, result: Result<(), IngestRefusal>) -> u32 {
    match result {
        Ok(()) => 0,
        Err(refusal) => refused(state, refusal.code(), &refusal),
    }
}

/// Stores a refusal's text as the reason and returns its code.
fn refused(state: &mut State, code: u32, reason: &dyn core::fmt::Display) -> u32 {
    let mut text = String::new();
    state.refusal.clear();
    if write!(text, "{reason}").is_ok() {
        state.refusal = text.into_bytes();
    }
    code
}

/// A `u32` length for the host. Every buffer read out is built by
/// [`law_snapshot`] or [`law_frames`], each of which refuses one longer than
/// `u32::MAX`, so the fallback is unreachable; it keeps the export total.
fn length(bytes: &[u8]) -> u32 {
    u32::try_from(bytes.len()).unwrap_or(0)
}

/// The host's bytes as a slice.
///
/// # Safety
///
/// When `ptr` is not null it must point to `len` bytes that are readable and
/// not written for the rest of the call.
unsafe fn input<'a>(ptr: *const u8, len: u32) -> Result<&'a [u8], Refusal> {
    if ptr.is_null() {
        return Err(Refusal::NullPointer);
    }
    let len = usize::try_from(len).map_err(|_| Refusal::Overflow)?;
    if isize::try_from(len).is_err() {
        return Err(Refusal::Overflow);
    }
    // SAFETY: `ptr` is not null, `u8` has alignment 1, `len` is at most
    // `isize::MAX`, and the caller guarantees the bytes are readable and not
    // written during the call.
    Ok(unsafe { core::slice::from_raw_parts(ptr, len) })
}

/// The law version, [`LAW_VERSION`].
#[unsafe(no_mangle)]
pub extern "C" fn law_version() -> u32 {
    LAW_VERSION
}

/// Allocates `len` bytes for the host to write into. Returns null when `len`
/// is 0 or the allocation fails.
#[unsafe(no_mangle)]
pub extern "C" fn law_alloc(len: u32) -> *mut u8 {
    let Ok(size) = usize::try_from(len) else {
        return null_mut();
    };
    if size == 0 {
        return null_mut();
    }
    let Ok(layout) = Layout::from_size_align(size, 1) else {
        return null_mut();
    };
    // SAFETY: the layout's size is not zero.
    unsafe { alloc::alloc::alloc(layout) }
}

/// Frees a buffer from [`law_alloc`]. Returns 0, or the code of
/// [`Refusal::NullPointer`] for a null pointer or a zero length, which
/// `law_alloc` never hands out.
///
/// # Safety
///
/// `ptr` must come from `law_alloc(len)` with this same `len`, and must not
/// have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn law_free(ptr: *mut u8, len: u32) -> u32 {
    with_status(|state| {
        let result = usize::try_from(len)
            .map_err(|_| Refusal::Overflow)
            .and_then(|size| {
                if ptr.is_null() || size == 0 {
                    return Err(Refusal::NullPointer);
                }
                let layout = Layout::from_size_align(size, 1).map_err(|_| Refusal::Overflow)?;
                // SAFETY: the caller guarantees `ptr` came from
                // `law_alloc(len)`, which allocated exactly this layout, and
                // has not been freed.
                unsafe { alloc::alloc::dealloc(ptr, layout) };
                Ok(())
            });
        status(state, result)
    })
}

/// The ingest verb ([`Law::ingest`]): a receipt and every file it receipts,
/// in the [`crate::wire`] container layout. The licence predicate admits the
/// score or refuses it, the receipt's SMF file is read and rescaled to the
/// law's PPQ, and the score is loaded with the transport stopped: the take is
/// emptied and the step count is zero. On a refusal the law keeps what it
/// held, and the code and reason name the layer that refused (see
/// [`IngestRefusal::code`]).
///
/// # Safety
///
/// `ptr` must point to `len` bytes that are readable and not written during
/// the call, or be null (which is refused).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn law_ingest(ptr: *const u8, len: u32) -> u32 {
    with_status(|state| {
        // SAFETY: the caller's contract is `input`'s.
        let result = unsafe { input(ptr, len) }
            .map_err(IngestRefusal::Law)
            .and_then(Law::ingest)
            .map(|law| {
                state.law = Some(law);
                state.record_changed();
            });
        ingest_status(state, result)
    })
}

/// Loads a score from bytes in the [`crate::wire`] score layout and stops the
/// transport: the take is emptied and the step count is zero. On a refusal the
/// law keeps what it held. No receipt admitted this score: its snapshot
/// records it as unreceipted, and [`law_ingest`] is the verb that admits one.
///
/// # Safety
///
/// `ptr` must point to `len` bytes that are readable and not written during
/// the call, or be null (which is refused).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn law_load_score(ptr: *const u8, len: u32) -> u32 {
    with_status(|state| {
        // SAFETY: the caller's contract is `input`'s.
        let result = unsafe { input(ptr, len) }
            .and_then(wire::decode_score)
            .and_then(|score| Law::load(&score))
            .map(|law| {
                state.law = Some(law);
                state.record_changed();
            });
        status(state, result)
    })
}

/// Admits a take, or part of one, from bytes in the [`crate::wire`] take
/// layout, through [`Law::admit`]: all of the notes or none.
///
/// # Safety
///
/// As [`law_load_score`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn law_admit_take(ptr: *const u8, len: u32) -> u32 {
    with_status(|state| {
        // SAFETY: the caller's contract is `input`'s.
        let result = unsafe { input(ptr, len) }
            .and_then(wire::decode_take)
            .and_then(|notes| match state.law.as_mut() {
                None => Err(Refusal::NoScore),
                Some(law) => law.admit(&notes),
            });
        if result.is_ok() {
            state.record_changed();
        }
        status(state, result)
    })
}

/// The live verb ([`Law::live`]): a note-on a person played, admitted into the
/// take as a record at its own onset, never refused for lateness. Its onset is
/// on the law's sample clock and may be negative (played before the take
/// started, which is refused); its pitch and velocity are wide so that a value
/// out of range is refused rather than cut. The law decides what the note
/// cites, each score note at most once, so the host passes notes in the order
/// their keys went down. The note is held until [`law_live_note_off`].
#[unsafe(no_mangle)]
pub extern "C" fn law_live_note(onset_sample: i64, pitch: u32, velocity: u32) -> u32 {
    with_status(|state| {
        let note = LiveNote {
            onset_sample,
            pitch,
            velocity,
        };
        let result = match state.law.as_mut() {
            None => Err(Refusal::NoScore),
            Some(law) => law.live(note).map(|_| ()),
        };
        if result.is_ok() {
            state.record_changed();
        }
        status(state, result)
    })
}

/// The live note-off ([`Law::live_off`]): the key of `pitch` came up at
/// `off_sample`, which ends the latest held live note of that pitch. The
/// length is kept for the committed frames; it is not graded or hashed.
#[unsafe(no_mangle)]
pub extern "C" fn law_live_note_off(pitch: u32, off_sample: i64) -> u32 {
    with_status(|state| {
        let off = LiveNoteOff { pitch, off_sample };
        let result = match state.law.as_mut() {
            None => Err(Refusal::NoScore),
            Some(law) => law.live_off(off).map(|_| ()),
        };
        if result.is_ok() {
            state.record_changed();
        }
        status(state, result)
    })
}

/// Steps one quantum ([`Law::step`]). A step that changes the record, in a
/// live take, clears the snapshot, the hash and the rows (see the module
/// documentation); the frames stay.
#[unsafe(no_mangle)]
pub extern "C" fn law_step() -> u32 {
    with_status(|state| {
        let result = match state.law.as_mut() {
            None => Err(Refusal::NoScore),
            Some(law) => law.step(),
        };
        let result = result.map(|changed| {
            if changed {
                state.clear_outputs();
            }
        });
        status(state, result)
    })
}

/// Quanta stepped since the score was loaded; 0 when no score is loaded.
#[unsafe(no_mangle)]
pub extern "C" fn law_steps() -> u64 {
    with_state(|state| state.law.as_ref().map_or(0, Law::steps)).unwrap_or(0)
}

/// The committed horizon, the last committed quantum; 0 while nothing is
/// committed (no score, or the transport stopped). A running transport's
/// horizon is at least H, so 0 is never a horizon.
#[unsafe(no_mangle)]
pub extern "C" fn law_horizon() -> u64 {
    with_state(|state| {
        state
            .law
            .as_ref()
            .and_then(|law| law.committed_horizon().ok().flatten())
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn build_snapshot(state: &mut State) -> Result<(), Refusal> {
    let law = state.law.as_ref().ok_or(Refusal::NoScore)?;
    let snapshot = law.snapshot_bytes()?;
    let rows = law.rows()?;
    let mut joined = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let extra = row.len().checked_add(1).ok_or(Refusal::Overflow)?;
        joined
            .try_reserve(extra)
            .map_err(|_| Refusal::OutOfMemory)?;
        if i > 0 {
            joined.push(b'\n');
        }
        joined.extend_from_slice(row.as_bytes());
    }
    if u32::try_from(snapshot.len()).is_err() || u32::try_from(joined.len()).is_err() {
        return Err(Refusal::Overflow);
    }
    state.hash = digest(&snapshot);
    state.snapshot = snapshot;
    state.rows = joined;
    Ok(())
}

/// Builds the snapshot, its SHA-256 and the rows for the getters below.
#[unsafe(no_mangle)]
pub extern "C" fn law_snapshot() -> u32 {
    with_status(|state| {
        let result = build_snapshot(state);
        if result.is_err() {
            state.clear_outputs();
        }
        status(state, result)
    })
}

/// The snapshot [`law_snapshot`] built.
#[unsafe(no_mangle)]
pub extern "C" fn law_snapshot_ptr() -> *const u8 {
    with_state(|state| state.snapshot.as_ptr()).unwrap_or(null())
}

/// The snapshot's length in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn law_snapshot_len() -> u32 {
    with_state(|state| length(&state.snapshot)).unwrap_or(0)
}

/// The snapshot's SHA-256: 32 bytes, all zero until [`law_snapshot`] succeeds.
#[unsafe(no_mangle)]
pub extern "C" fn law_hash_ptr() -> *const u8 {
    with_state(|state| state.hash.as_ptr()).unwrap_or(null())
}

/// The rows [`law_snapshot`] built, UTF-8, joined by line feeds.
#[unsafe(no_mangle)]
pub extern "C" fn law_rows_ptr() -> *const u8 {
    with_state(|state| state.rows.as_ptr()).unwrap_or(null())
}

/// The rows' length in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn law_rows_len() -> u32 {
    with_state(|state| length(&state.rows)).unwrap_or(0)
}

/// The reason for the most recent refusal, UTF-8; empty until the first one.
/// A success does not clear it, so read it after a non-zero status.
#[unsafe(no_mangle)]
pub extern "C" fn law_refusal_ptr() -> *const u8 {
    with_state(|state| state.refusal.as_ptr()).unwrap_or(null())
}

/// The reason's length in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn law_refusal_len() -> u32 {
    with_state(|state| length(&state.refusal)).unwrap_or(0)
}

fn build_frames(state: &mut State, first: u64, last: u64) -> Result<(), Refusal> {
    let law = state.law.as_ref().ok_or(Refusal::NoScore)?;
    let bytes = wire::encode_frames(&law.frames(first, last)?)?;
    if u32::try_from(bytes.len()).is_err() {
        return Err(Refusal::Overflow);
    }
    state.frames = bytes;
    Ok(())
}

/// Builds the committed frames of the quanta `first_quantum..=last_quantum`
/// ([`Law::frames`]) for the getters below. Reading them changes nothing in
/// the law. On a refusal the frames are empty.
#[unsafe(no_mangle)]
pub extern "C" fn law_frames(first_quantum: u64, last_quantum: u64) -> u32 {
    with_status(|state| {
        let result = build_frames(state, first_quantum, last_quantum);
        if result.is_err() {
            state.frames.clear();
        }
        status(state, result)
    })
}

/// The frames [`law_frames`] built.
#[unsafe(no_mangle)]
pub extern "C" fn law_frames_ptr() -> *const u8 {
    with_state(|state| state.frames.as_ptr()).unwrap_or(null())
}

/// The frames' length in bytes; 0 until [`law_frames`] succeeds.
#[unsafe(no_mangle)]
pub extern "C" fn law_frames_len() -> u32 {
    with_state(|state| length(&state.frames)).unwrap_or(0)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used
)]
mod tests {
    extern crate std;

    use super::*;
    use crate::score::tests::ingested;
    use crate::take::{ScoreNoteId, TakeNote};
    use crate::{HORIZON_QUANTA, QUANTUM_SAMPLES};
    use std::sync::Mutex;

    /// The exports share one state; tests that call them take turns.
    static TURN: Mutex<()> = Mutex::new(());

    fn reset() {
        with_state(|state| {
            state.law = None;
            state.record_changed();
            state.refusal.clear();
        })
        .unwrap();
    }

    /// Copies `bytes` into a `law_alloc` buffer, calls `f`, frees the buffer.
    fn call(bytes: &[u8], f: unsafe extern "C" fn(*const u8, u32) -> u32) -> u32 {
        let len = bytes.len() as u32;
        let ptr = law_alloc(len);
        assert!(!ptr.is_null());
        // SAFETY: `ptr` has room for `len` bytes, and `f`'s contract holds for
        // bytes the host wrote.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            let status = f(ptr, len);
            assert_eq!(law_free(ptr, len), 0);
            status
        }
    }

    fn read(ptr: *const u8, len: u32) -> std::vec::Vec<u8> {
        // SAFETY: the pointer and length come from the getters, which point
        // into the law's live buffers.
        unsafe { core::slice::from_raw_parts(ptr, len as usize).to_vec() }
    }

    fn refusal_text() -> std::string::String {
        std::string::String::from_utf8(read(law_refusal_ptr(), law_refusal_len())).unwrap()
    }

    fn take() -> std::vec::Vec<TakeNote> {
        std::vec![
            TakeNote {
                onset_sample: 0,
                pitch: 60,
                velocity: 64,
                cites: Some(ScoreNoteId(0)),
            },
            TakeNote {
                onset_sample: 26_160,
                pitch: 62,
                velocity: 64,
                cites: Some(ScoreNoteId(2)),
            },
        ]
    }

    #[test]
    fn the_abi_agrees_with_the_rust_law() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        let score = wire::encode_score(&ingested()).unwrap();
        let take_bytes = wire::encode_take(&take()).unwrap();
        assert_eq!(call(&score, law_load_score), 0);
        assert_eq!(call(&take_bytes, law_admit_take), 0);
        for _ in 0..5 {
            assert_eq!(law_step(), 0);
        }
        assert_eq!(law_steps(), 5);
        assert_eq!(law_snapshot(), 0);
        assert_eq!(law_refusal_len(), 0);

        let mut native = Law::load(&ingested()).unwrap();
        native.admit(&take()).unwrap();
        assert_eq!(
            read(law_snapshot_ptr(), law_snapshot_len()),
            native.snapshot_bytes().unwrap()
        );
        assert_eq!(read(law_hash_ptr(), 32), native.hash().unwrap());
        let rows = std::string::String::from_utf8(read(law_rows_ptr(), law_rows_len())).unwrap();
        assert_eq!(rows, native.rows().unwrap().join("\n"));
        assert!(rows.starts_with(
            "note 0: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 60 vs 60: match\n"
        ));
        assert_eq!(law_version(), LAW_VERSION);

        // Admitting clears the outputs, so a stale hash is never current.
        let late = TakeNote {
            onset_sample: 1_000_000,
            pitch: 70,
            velocity: 10,
            cites: None,
        };
        assert_eq!(
            call(&wire::encode_take(&[late]).unwrap(), law_admit_take),
            0
        );
        assert_eq!(law_snapshot_len(), 0);
        assert_eq!(read(law_hash_ptr(), 32), [0; 32]);
    }

    #[test]
    fn the_ingest_export_agrees_with_the_rust_law() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        let container = crate::ingest_tests::entertainer();
        assert_eq!(call(&container, law_ingest), 0);
        assert_eq!(law_refusal_len(), 0);
        assert_eq!(law_steps(), 0);

        let mut native = Law::ingest(&container).unwrap();
        let first = native.score().notes()[0];
        let take = [TakeNote {
            onset_sample: first.onset_sample + 1_440,
            pitch: first.pitch,
            velocity: 90,
            cites: Some(ScoreNoteId(0)),
        }];
        assert_eq!(call(&wire::encode_take(&take).unwrap(), law_admit_take), 0);
        for _ in 0..10 {
            assert_eq!(law_step(), 0);
        }
        assert_eq!(law_snapshot(), 0);
        native.admit(&take).unwrap();
        assert_eq!(
            read(law_snapshot_ptr(), law_snapshot_len()),
            native.snapshot_bytes().unwrap()
        );
        assert_eq!(read(law_hash_ptr(), 32), native.hash().unwrap());
        let rows = std::string::String::from_utf8(read(law_rows_ptr(), law_rows_len())).unwrap();
        assert!(rows.starts_with(
            "note 0: onset +1440 samples (+30.0 ms) vs gate \u{b1}1920, pitch 74 vs 74: match\n"
        ));
        assert_eq!(rows.lines().count(), 2621);
    }

    #[test]
    fn an_ingest_refusal_crosses_with_its_layers_code_and_reason() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        use crate::ingest_tests::{MID, RECEIPT, container, entertainer};
        assert_eq!(call(&entertainer(), law_ingest), 0);
        assert_eq!(law_snapshot(), 0);
        let held = read(law_hash_ptr(), 32);

        let missing = container(RECEIPT, &[("entertainer.mid", MID)]);
        assert_eq!(call(&missing, law_ingest), 101);
        assert_eq!(
            refusal_text(),
            "score refused by the licence predicate: the receipt lists entertainer.ly, which \
             was not supplied"
        );
        assert_eq!(call(b"SJIN", law_ingest), 50);
        assert_eq!(
            refusal_text(),
            "bytes refused at offset 4: the bytes end early"
        );
        // SAFETY: a null pointer is refused before anything is read.
        unsafe {
            assert_eq!(law_ingest(core::ptr::null(), 4), 60);
        }
        assert_eq!(law_snapshot(), 0);
        assert_eq!(
            read(law_hash_ptr(), 32),
            held,
            "a refused ingest keeps the score the law held"
        );
    }

    #[test]
    fn refusals_cross_as_codes_with_their_reasons() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        assert_eq!(law_refusal_len(), 0);

        // A call still holding the state (another thread's, or one that
        // trapped) fences it off: every export refuses or answers empty.
        SHARED.busy.store(true, Ordering::SeqCst);
        assert_eq!(law_step(), Refusal::Busy.code());
        assert_eq!(law_snapshot(), 63);
        // SAFETY: refused before the pointer is read.
        assert_eq!(unsafe { law_load_score(core::ptr::null(), 0) }, 63);
        assert!(law_hash_ptr().is_null());
        assert_eq!(law_steps(), 0);
        assert_eq!(law_refusal_len(), 0);
        SHARED.busy.store(false, Ordering::SeqCst);

        assert_eq!(law_step(), Refusal::NoScore.code());
        assert_eq!(refusal_text(), "refused: no score is loaded");
        assert_eq!(law_snapshot(), 30);
        assert_eq!(law_steps(), 0);

        assert_eq!(call(b"nonsense", law_load_score), 50);
        assert_eq!(
            refusal_text(),
            "bytes refused at offset 0: the magic is wrong"
        );
        // SAFETY: a null pointer is refused before anything is read.
        unsafe {
            assert_eq!(law_load_score(core::ptr::null(), 4), 60);
            assert_eq!(law_free(null_mut(), 4), 60);
        }
        assert!(law_alloc(0).is_null());

        let mut bad = ingested();
        bad.notes[2].start_tick = 385;
        assert_eq!(call(&wire::encode_score(&bad).unwrap(), law_load_score), 21);
        assert_eq!(
            refusal_text(),
            "score refused: tick 385 of the start of note 2 is not a whole law tick: 385 x 3360 \
             is not divisible by the source PPQ 384"
        );

        assert_eq!(
            call(&wire::encode_score(&ingested()).unwrap(), law_load_score),
            0
        );
        assert!(
            refusal_text().starts_with("score refused: tick 385"),
            "a success leaves the last reason in place"
        );
        for _ in 0..3 {
            assert_eq!(law_step(), 0);
        }
        // Playhead 2, committed through 2 + H; quantum 60 is late by H - 57.
        let q = 60u64;
        let late = TakeNote {
            onset_sample: q * u64::from(QUANTUM_SAMPLES),
            pitch: 60,
            velocity: 64,
            cites: Some(ScoreNoteId(0)),
        };
        assert_eq!(
            call(&wire::encode_take(&[late]).unwrap(), law_admit_take),
            36
        );
        let horizon = 2 + u64::from(HORIZON_QUANTA);
        assert_eq!(
            refusal_text(),
            std::format!(
                "take refused: note 0 is late by {} quanta: it falls in quantum 60, and quanta \
                 through {horizon} are committed",
                horizon - q + 1
            )
        );
    }

    /// Steps the C ABI's law and the Rust law together.
    fn step_both(native: &mut Law, steps: u64) {
        for _ in 0..steps {
            assert_eq!(law_step(), 0);
            native.step().unwrap();
        }
    }

    /// Passes live notes, `(onset, pitch, velocity, status, cites)`, to both
    /// laws: each status is the Rust law's, and each citation is the one
    /// expected.
    fn live_notes(native: &mut Law, notes: &[(i64, u32, u32, u32, Option<u32>)]) {
        for &(onset_sample, pitch, velocity, code, cites) in notes {
            let status = law_live_note(onset_sample, pitch, velocity);
            let rust = native.live(LiveNote {
                onset_sample,
                pitch,
                velocity,
            });
            let rust_code = rust.as_ref().map_or_else(|r| r.code(), |_| 0);
            assert_eq!((status, rust_code), (code, code), "onset {onset_sample}");
            if let Ok(admitted) = rust {
                assert_eq!(admitted.cites.map(|id| id.0), cites, "onset {onset_sample}");
            }
        }
    }

    /// In a live take a step that makes rows final changes the record: it
    /// clears the snapshot, the hash and the rows, so the old ones are never
    /// read as current. So does the first step, which withdraws the rows a
    /// stopped law shows for an empty take. A step that makes nothing final
    /// leaves them, and no step clears the frames, which a step does not
    /// change.
    #[test]
    fn a_step_that_makes_rows_final_clears_the_snapshot() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        assert_eq!(
            call(&wire::encode_score(&ingested()).unwrap(), law_load_score),
            0
        );
        let mut native = Law::load(&ingested()).unwrap();
        assert_eq!(same_record(&native).lines().count(), 4, "all never played");
        step_both(&mut native, 1);
        assert_eq!(law_snapshot_len(), 0, "the first step withdrew them");
        assert_eq!(law_rows_len(), 0);
        assert_eq!(same_record(&native), "");
        live_notes(&mut native, &[(0, 60, 64, 0, Some(0))]);
        // The playhead past 8,640: notes 0 and 1 have closed.
        step_both(&mut native, 200);
        let rows = same_record(&native);
        assert_eq!(rows.lines().count(), 2);
        let hash = read(law_hash_ptr(), 32);
        assert_ne!(hash, [0; 32]);
        assert_eq!(law_frames(0, 10), 0);
        let frames = read(law_frames_ptr(), law_frames_len());
        assert!(!frames.is_empty());

        // Nothing closes on the next step: the record stands.
        step_both(&mut native, 1);
        assert_eq!(read(law_hash_ptr(), 32), hash);
        assert_eq!(read(law_rows_ptr(), law_rows_len()), rows.as_bytes());

        // Note 2 closes when the playhead passes 32,640: on that step the
        // snapshot, the hash and the rows are cleared, and the frames stay.
        let q = u64::from(QUANTUM_SAMPLES);
        let before = read(law_snapshot_ptr(), law_snapshot_len());
        assert!(!before.is_empty());
        // Every step that leaves the playhead at or before 32,640 leaves the
        // snapshot as it was.
        while native.steps() * q <= 32_640 {
            step_both(&mut native, 1);
            assert_eq!(read(law_snapshot_ptr(), law_snapshot_len()), before);
        }
        step_both(&mut native, 1);
        assert!((native.steps() - 1) * q > 32_640);
        assert_eq!(law_snapshot_len(), 0);
        assert_eq!(law_rows_len(), 0);
        assert_eq!(read(law_hash_ptr(), 32), [0; 32]);
        assert_eq!(read(law_frames_ptr(), law_frames_len()), frames);
        let now = same_record(&native);
        assert_eq!(now.lines().count(), 3);
        assert!(now.starts_with(&rows));
    }

    /// The C ABI's snapshot and rows are the Rust law's; returns the rows.
    fn same_record(native: &Law) -> std::string::String {
        assert_eq!(law_snapshot(), 0);
        assert_eq!(
            read(law_snapshot_ptr(), law_snapshot_len()),
            native.snapshot_bytes().unwrap()
        );
        let rows = std::string::String::from_utf8(read(law_rows_ptr(), law_rows_len())).unwrap();
        assert_eq!(rows, native.rows().unwrap().join("\n"));
        rows
    }

    /// The live verbs, the horizon and the frame export through the C ABI: each
    /// status is the Rust law's, the frames are the Rust law's frames in their
    /// layout, and a live take's snapshot and rows are the Rust law's at every
    /// point, showing each score note's row only once it has closed.
    #[test]
    fn the_live_and_frame_exports_agree_with_the_rust_law() {
        let _turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        reset();
        assert_eq!(law_live_note(0, 60, 64), Refusal::NoScore.code());
        assert_eq!(law_live_note_off(60, 10), Refusal::NoScore.code());
        assert_eq!(law_frames(0, 0), Refusal::NoScore.code());
        assert_eq!(law_horizon(), 0);
        assert_eq!(law_frames_len(), 0);

        assert_eq!(
            call(&wire::encode_score(&ingested()).unwrap(), law_load_score),
            0
        );
        assert_eq!(law_horizon(), 0, "stopped: nothing is committed");
        assert_eq!(law_live_note(0, 60, 64), 160);
        assert_eq!(
            refusal_text(),
            "live note refused: the transport is stopped, so no quantum is committed"
        );
        assert_eq!(law_live_note_off(60, 10), 160);
        assert_eq!(law_frames(0, 0), 171);

        // The score: note 0 (sample 0, pitch 60), note 1 (0, 64), note 2
        // (24,000, 62), note 3 (48,000, 65). Each closes when the playhead
        // passes its onset plus 8,640.
        let q = u64::from(QUANTUM_SAMPLES);
        let h = u64::from(HORIZON_QUANTA);
        let mut native = Law::load(&ingested()).unwrap();
        step_both(&mut native, 100);
        let horizon = 99 + h;
        assert_eq!(law_horizon(), horizon);
        let ahead = (horizon + 1) * q;
        live_notes(
            &mut native,
            &[
                (0, 60, 64, 0, Some(0)),
                // Note 0 is cited, so the same key again cites note 1, then
                // nothing: an addition, which a third time is a duplicate.
                (0, 60, 90, 0, Some(1)),
                (0, 60, 90, 0, None),
                (0, 60, 90, 167, None),
                (-1, 60, 64, 161, None),
                (30, 128, 64, 163, None),
                (30, 61, 0, 164, None),
                (ahead as i64, 61, 64, 162, None),
                (i64::MAX - 5, 61, 64, 162, None),
            ],
        );
        assert_eq!(
            refusal_text(),
            std::format!(
                "live note refused: sample {} falls in quantum {}, after the committed horizon \
                 {horizon}; a live note records what was played",
                i64::MAX - 5,
                (i64::MAX as u64 - 5) / 48
            )
        );
        assert_eq!(same_record(&native), "", "nothing has closed");

        // The playhead at 28,752: notes 0 and 1 have closed, and so has the
        // addition at sample 0.
        step_both(&mut native, 500);
        assert_eq!(
            same_record(&native),
            "note 0: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 60 vs 60: match\n\
             note 1: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 60 vs 64: wrong pitch\n\
             take note at onset 0 samples, pitch 60, cites no score note: addition"
        );
        // Note 2 is open, so a note for it is graded against it.
        live_notes(&mut native, &[(24_050, 62, 80, 0, Some(2))]);

        // The playhead at 52,752: note 2 has closed, note 3 has not.
        step_both(&mut native, 500);
        let horizon = 1_099 + h;
        live_notes(
            &mut native,
            &[
                (48_000, 66, 1, 0, Some(3)),
                // Delivered long after note 2 closed: never refused, an
                // addition.
                (24_100, 62, 70, 0, None),
            ],
        );
        // A proposal into the live take may not cite the closed note 2.
        let proposal = TakeNote {
            onset_sample: (horizon + 1) * q,
            pitch: 62,
            velocity: 5,
            cites: Some(ScoreNoteId(2)),
        };
        let code = call(&wire::encode_take(&[proposal]).unwrap(), law_admit_take);
        assert_eq!(code, native.admit(&[proposal]).unwrap_err().code());
        assert_eq!(code, 173);
        assert_eq!(
            refusal_text(),
            "take refused: note 0 cites score note 2, which closed when the playhead passed \
             sample 32640; its verdict is final"
        );

        // Held: three notes of pitch 60 at sample 0, two of 62, one of 66. A
        // note-off ends the latest of its pitch by key: first the 60 that
        // cites note 1, then the 62 addition at 24,100.
        let ahead = (horizon + 1) * q;
        let offs: [(u32, i64, u32); 8] = [
            (60, 100, 0),
            (60, 0, 165),
            (61, 100, 168),
            (128, 100, 163),
            (60, -1, 161),
            (60, ahead as i64, 162),
            (62, 24_200, 0),
            (62, 24_300, 0),
        ];
        for (pitch, off_sample, code) in offs {
            let status = law_live_note_off(pitch, off_sample);
            let rust = native
                .live_off(LiveNoteOff { pitch, off_sample })
                .map_or_else(|r| r.code(), |_| 0);
            assert_eq!(
                (status, rust),
                (code, code),
                "pitch {pitch} off {off_sample}"
            );
        }
        // A refusal leaves the law as it was, so the Rust law needs no call.
        assert_eq!(law_live_note_off(60, -1), 161);
        assert_eq!(
            refusal_text(),
            "live note refused: sample -1 is before sample 0, where the take starts"
        );
        assert_eq!(law_live_note_off(60, 0), 165);
        assert_eq!(
            refusal_text(),
            "live note-off refused: sample 0 is not after the note-on at sample 0 it ends, so \
             the note stays held"
        );
        assert_eq!(law_live_note_off(61, 100), 168);
        assert_eq!(
            refusal_text(),
            "live note-off refused: no live note of pitch 61 is held"
        );

        assert_eq!(law_frames(0, horizon), 0);
        let frames = read(law_frames_ptr(), law_frames_len());
        assert_eq!(
            frames,
            wire::encode_frames(&native.frames(0, horizon).unwrap()).unwrap()
        );
        let decoded = wire::decode_frames(&frames).unwrap();
        assert_eq!(decoded.notes.len(), 4 + 6);
        assert_eq!(law_frames(horizon + 1, horizon + 1), 172);
        assert_eq!(
            refusal_text(),
            std::format!(
                "frames refused: the window ends at quantum {}, after the committed horizon \
                 {horizon}",
                horizon + 1
            )
        );
        assert_eq!(law_frames_len(), 0, "a refused export leaves no frames");
        assert_eq!(law_frames(2, 1), 170);

        // A live note clears the frames: it can land in a window already read.
        assert_eq!(law_frames(0, 10), 0);
        assert!(law_frames_len() > 0);
        assert_eq!(law_live_note(200, 64, 50), 0);
        native
            .live(LiveNote {
                onset_sample: 200,
                pitch: 64,
                velocity: 50,
            })
            .unwrap();
        assert_eq!(law_frames_len(), 0);
        // So does a note-off: it sets a length the frames carry.
        assert_eq!(law_frames(0, 10), 0);
        assert!(law_frames_len() > 0);
        assert_eq!(law_live_note_off(64, 203), 0);
        native
            .live_off(LiveNoteOff {
                pitch: 64,
                off_sample: 203,
            })
            .unwrap();
        assert_eq!(law_frames_len(), 0);

        SHARED.busy.store(true, Ordering::SeqCst);
        assert_eq!(law_live_note(0, 60, 64), 63);
        assert_eq!(law_live_note_off(60, 10), 63);
        assert_eq!(law_frames(0, 0), 63);
        assert!(law_frames_ptr().is_null());
        assert_eq!(law_frames_len(), 0);
        assert_eq!(law_horizon(), 0);
        SHARED.busy.store(false, Ordering::SeqCst);

        // Once every note has closed, every row is shown.
        step_both(&mut native, 1_000);
        let rows = same_record(&native);
        assert_eq!(rows.lines().count(), 4 + 3);
        assert!(rows.contains(
            "note 3: onset +0 samples (+0.0 ms) vs gate \u{b1}1920, pitch 66 vs 65: wrong pitch"
        ));
    }
}
