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
//! next call that loads, admits or snapshots. Any call may grow linear memory,
//! which detaches a JavaScript host's views, so a host re-creates its views
//! after each call and copies bytes out before the next one.
//!
//! Loading a score or admitting a take clears the snapshot, the hash and the
//! rows, so a stale hash is never read as current. Stepping leaves them: a
//! step does not change the record.
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
use crate::{LAW_VERSION, Law, wire};

struct State {
    law: Option<Law>,
    snapshot: Vec<u8>,
    hash: [u8; 32],
    rows: Vec<u8>,
    refusal: Vec<u8>,
}

impl State {
    fn clear_outputs(&mut self) {
        self.snapshot.clear();
        self.rows.clear();
        self.hash = [0; 32];
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
/// [`law_snapshot`], which refuses one longer than `u32::MAX`, so the fallback
/// is unreachable; it keeps the export total.
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
                state.clear_outputs();
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
                state.clear_outputs();
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
            state.clear_outputs();
        }
        status(state, result)
    })
}

/// Steps one quantum ([`Law::step`]).
#[unsafe(no_mangle)]
pub extern "C" fn law_step() -> u32 {
    with_status(|state| {
        let result = match state.law.as_mut() {
            None => Err(Refusal::NoScore),
            Some(law) => law.step(),
        };
        status(state, result)
    })
}

/// Quanta stepped since the score was loaded; 0 when no score is loaded.
#[unsafe(no_mangle)]
pub extern "C" fn law_steps() -> u64 {
    with_state(|state| state.law.as_ref().map_or(0, Law::steps)).unwrap_or(0)
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
            state.clear_outputs();
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
}
