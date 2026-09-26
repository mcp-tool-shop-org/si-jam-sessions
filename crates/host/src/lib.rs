//! The native host of si-jam-sessions: the picture, in PHASE-0's three owners.
//!
//! It owns the audio device and its callback, plays the law's committed
//! frames, and timestamps live input. It decides nothing: the law holds the
//! score, the take and the verdicts, and the host drives it through the same
//! `extern "C"` exports the wasm law has ([`bridge`]), so the boundary it uses
//! is the boundary the golden hashes.
//!
//! - [`synth`]: the render, two oscillator voices and a click. The audio
//!   callback calls it; it allocates nothing, locks nothing and makes no system
//!   call, and it puts every event on its sample exactly.
//! - [`callback`]: the audio callback's body, which calls the synth. Its
//!   silent pre-roll holds law time at sample 0 until a callback's frames are
//!   all in the ring, so the device's first fill makes no event late.
//! - [`event`]: what crosses into the callback, through `rtrb` rings created
//!   before the stream.
//! - [`schedule`]: the non-real-time side. It steps the law one quantum per 48
//!   samples of the audio clock, in a jam with the playhead on the sample
//!   heard, and moves committed frames into the ring ahead of the render, each
//!   exactly once.
//! - [`anchor`] and [`live`]: the clocks, and live input from a press to the
//!   law's live verbs, a note-on when a key goes down and a note-off when it
//!   comes up, each within the law's delivery allowance. A key press is stamped
//!   on the stream clock, a MIDI message on WinMM's; both become a law sample
//!   through the same audio clock, which is re-anchored on every callback.
//! - [`offline`]: the same pipeline without a device, for `render`.
//! - [`device`], and `winmm` and `console` on Windows: the thin shells around
//!   cpal, WinMM MIDI input and the Windows console. They are not exercised in
//!   CI, which has no audio or MIDI device.
//!
//! The host's tests are unit tests, in this library. An integration test makes
//! cargo build the host binary beside the tests, and with it the law's cdylib a
//! second time (under `panic = "abort"` instead of the tests' unwinding); the
//! two `law.dll` outputs collide, which fails the link on Windows.
//!
//! # Standards compliance
//!
//! The studio's six workflow standards, scored 0-3 for the host.
//!
//! - **PIN_PER_STEP 2.** cpal 0.18.2, rtrb 0.4.0 and windows 0.62.2 are pinned
//!   exactly and the lockfile is `--locked`. The rate, the quantum and the
//!   horizon come from the law crate, never from copies.
//! - **ANDON_AUTHORITY 2.** A refused law call stops the command with the law's
//!   code and reason. A device error stops the stream and is reported. Late
//!   and dropped events are counted and printed, and a render that clips fails
//!   its test.
//! - **NAMED_COMPENSATORS.** Nothing here is irreversible: the host writes the
//!   WAV file it is asked for and publishes nothing, so there is nothing to
//!   undo.
//! - **DECOMPOSE_BY_SECRETS 2.** The law is reached only through its C ABI.
//!   The real-time code shares two rings and a handful of atomics with the rest,
//!   and nothing else. No secret is read.
//! - **UNCERTAINTY_GATED_HUMANS 1.** skip: what is certain is tested (where
//!   every event lands, exactly; the scheduling; the clock maths). What only
//!   ears can judge (the timbres, the delay felt through a device) goes to the
//!   owner's listening test, with the times to listen at.
//! - **EXTERNAL_VERIFIER 2.** No model grades anything. The law grades live
//!   notes with its hand-written rule, and the code is reviewed by a different
//!   model family.

#[cfg(test)]
mod alloc_free;
pub mod anchor;
pub mod bridge;
pub mod callback;
#[cfg(windows)]
pub mod console;
pub mod device;
pub mod event;
pub mod live;
pub mod notices;
pub mod offline;
pub mod schedule;
pub mod score;
pub mod synth;
#[cfg(windows)]
pub mod winmm;

/// A command's result once its report is printed. A report that could not be
/// printed is the result; otherwise the error that stopped the command, a
/// device error or a refused law call, if there was one: a command that
/// stopped on an error exits 1 even when it printed its report.
pub fn outcome(report: Result<(), String>, stopped: Option<String>) -> Result<(), String> {
    report?;
    match stopped {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Events the ring into the callback holds. The scheduler keeps it filled
/// through the committed horizon, 100 ms ahead of the playhead, and *The
/// Entertainer* commits a few dozen events in any 100 ms.
pub const RING_EVENTS: usize = 4_096;

#[cfg(test)]
mod tests {
    use super::*;

    /// A jam or a play that stopped on a device error prints its report and
    /// still fails; a report that cannot be printed fails first.
    #[test]
    fn a_command_that_stopped_on_an_error_fails_after_its_report() {
        let device = || Some(String::from("the output device is no longer available"));
        assert_eq!(outcome(Ok(()), None), Ok(()));
        assert_eq!(
            outcome(Ok(()), device()),
            Err(String::from("the output device is no longer available"))
        );
        assert_eq!(
            outcome(Err(String::from("law_snapshot refused")), device()),
            Err(String::from("law_snapshot refused"))
        );
    }
}
