//! The law, driven through its C ABI.
//!
//! These are the `extern "C"` exports the wasm law has, called natively: bytes
//! in through `law_alloc`, statuses and bytes out, exactly as a JavaScript
//! engine drives the wasm. The golden already proves the native law and the
//! wasm law compute the same bytes, so this is the boundary that is hashed.
//!
//! The exports share one state per process, so a process holds one [`Law`]
//! at a time: [`Law::acquire`] waits for it.
//!
//! The host calls `law_frames`, `law_horizon`, `law_live_note` and
//! `law_live_note_off`, which came with law version 4; version 3 has none of
//! them. A host that loads a law reads `law_version()` first. This one links
//! the law it is built with, so it checks when it builds: a law before version
//! 4 fails the build at [`LAW_VERSION_NEEDED`], not at a call.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use law::Frames;
use law::abi;
use law::wire::decode_frames;

/// A call the law refused: the export, its status code and its reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub verb: &'static str,
    pub code: u32,
    pub reason: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} refused with status {}: {}",
            self.verb, self.code, self.reason
        )
    }
}

impl std::error::Error for Refused {}

/// The snapshot's SHA-256 and the rows, read after `law_snapshot`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub hash: [u8; 32],
    pub rows: String,
}

/// The first law version with the verbs the host calls.
pub const LAW_VERSION_NEEDED: u32 = 4;

const _: () = assert!(law::LAW_VERSION >= LAW_VERSION_NEEDED);

static HELD: AtomicBool = AtomicBool::new(false);

/// The process's one handle on the law's C ABI.
#[derive(Debug)]
pub struct Law {
    _only: (),
}

impl Law {
    /// Waits until no other handle is held, then takes it.
    pub fn acquire() -> Law {
        while HELD
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            thread::sleep(Duration::from_millis(1));
        }
        Law { _only: () }
    }

    fn check(verb: &'static str, status: u32) -> Result<(), Refused> {
        if status == 0 {
            return Ok(());
        }
        let reason = String::from_utf8_lossy(&read(abi::law_refusal_ptr(), abi::law_refusal_len()))
            .into_owned();
        Err(Refused {
            verb,
            code: status,
            reason,
        })
    }

    /// Copies `bytes` into a `law_alloc` buffer, calls `verb` on it and frees
    /// it, as a JavaScript host does.
    fn pass(
        verb: unsafe extern "C" fn(*const u8, u32) -> u32,
        name: &'static str,
        bytes: &[u8],
    ) -> Result<(), Refused> {
        let too_long = || Refused {
            verb: name,
            code: 0,
            reason: String::from("the input is longer than 4 GiB"),
        };
        let len = u32::try_from(bytes.len()).map_err(|_| too_long())?;
        let ptr = abi::law_alloc(len);
        if ptr.is_null() {
            return Err(Refused {
                verb: "law_alloc",
                code: 0,
                reason: format!("law_alloc({len}) returned null"),
            });
        }
        // SAFETY: `law_alloc` returned `len` writable bytes, and `bytes` has
        // exactly `len` bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        // SAFETY: `ptr` holds `len` bytes this thread wrote, unchanged during
        // the call.
        let status = unsafe { verb(ptr, len) };
        // SAFETY: `ptr` came from `law_alloc(len)` and is freed once, here.
        let freed = unsafe { abi::law_free(ptr, len) };
        Law::check(name, status)?;
        Law::check("law_free", freed)
    }

    /// The ingest verb: a receipt and the files it receipts, in the law's
    /// container layout. The transport stops and the take empties.
    pub fn ingest(&mut self, container: &[u8]) -> Result<(), Refused> {
        Law::pass(abi::law_ingest, "law_ingest", container)
    }

    /// Admits a take in the law's take layout.
    pub fn admit_take(&mut self, take: &[u8]) -> Result<(), Refused> {
        Law::pass(abi::law_admit_take, "law_admit_take", take)
    }

    /// Loads a score in the law's score layout, with no receipt: the tests'
    /// own scores. A host plays a score through [`Law::ingest`].
    #[cfg(test)]
    pub fn load_score(&mut self, score: &[u8]) -> Result<(), Refused> {
        Law::pass(abi::law_load_score, "law_load_score", score)
    }

    /// Steps one quantum.
    pub fn step(&mut self) -> Result<(), Refused> {
        Law::check("law_step", abi::law_step())
    }

    /// Quanta stepped since the score was loaded.
    pub fn steps(&self) -> u64 {
        abi::law_steps()
    }

    /// The last committed quantum, or `None` while nothing is committed.
    pub fn horizon(&self) -> Option<u64> {
        match abi::law_horizon() {
            0 => None,
            h => Some(h),
        }
    }

    /// The committed frames of the quanta `first..=last`.
    pub fn frames(&mut self, first: u64, last: u64) -> Result<Frames, Refused> {
        Law::check("law_frames", abi::law_frames(first, last))?;
        let bytes = read(abi::law_frames_ptr(), abi::law_frames_len());
        decode_frames(&bytes).map_err(|r| Refused {
            verb: "the frame decoder",
            code: r.code(),
            reason: r.to_string(),
        })
    }

    /// The live note-on: a key went down, heard at law sample
    /// `onset_sample`. The law decides what it cites.
    pub fn live_note(
        &mut self,
        onset_sample: i64,
        pitch: u32,
        velocity: u32,
    ) -> Result<(), Refused> {
        Law::check(
            "law_live_note",
            abi::law_live_note(onset_sample, pitch, velocity),
        )
    }

    /// The live note-off: the key of `pitch` came up at law sample
    /// `off_sample`, which ends the live note of that pitch the law holds.
    pub fn live_note_off(&mut self, pitch: u32, off_sample: i64) -> Result<(), Refused> {
        Law::check(
            "law_live_note_off",
            abi::law_live_note_off(pitch, off_sample),
        )
    }

    /// The snapshot's hash and the rows.
    pub fn record(&mut self) -> Result<Record, Refused> {
        Law::check("law_snapshot", abi::law_snapshot())?;
        let hash = read(abi::law_hash_ptr(), 32)
            .try_into()
            .map_err(|_| Refused {
                verb: "law_hash_ptr",
                code: 0,
                reason: String::from("the hash is not 32 bytes"),
            })?;
        let rows =
            String::from_utf8_lossy(&read(abi::law_rows_ptr(), abi::law_rows_len())).into_owned();
        Ok(Record { hash, rows })
    }
}

impl Drop for Law {
    fn drop(&mut self) {
        HELD.store(false, Ordering::Release);
    }
}

/// Copies `len` bytes out of the law's buffer at `ptr`.
fn read(ptr: *const u8, len: u32) -> Vec<u8> {
    let Ok(len) = usize::try_from(len) else {
        return Vec::new();
    };
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: the pointer and length come from the law's getters, which point
    // into its live buffers until the next call that changes them; the bytes
    // are copied out before any such call.
    unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
}
