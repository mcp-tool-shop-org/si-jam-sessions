//! MIDI input through WinMM: the primary live input on Windows.
//!
//! It makes the calls midir 0.11.0's WinMM backend makes (`midiInGetNumDevs`,
//! `midiInGetDevCapsW`, `midiInOpen` with a callback, `midiInStart`, and
//! `midiInStop`, `midiInReset` and `midiInClose` to finish), through the
//! `windows` crate that cpal already links on Windows. midir itself is not a
//! dependency: through its Android-only dependencies (jni 0.21.1 and
//! jni-min-helper 0.3.4) midir 0.11.0 brings libloading 0.7.4 into the
//! lockfile, which is ISC, outside the licence allowlist; the last midir
//! without them, 0.10.4, links an older alsa-sys than cpal 0.18.2 and cannot
//! share a lockfile with it.
//!
//! WinMM stamps each message in milliseconds since `midiInStart`, and the host
//! reads it in microseconds, as midir does (KB recipe 1536). The callback does
//! no more than it must: it reads the stream clock for the message's arrival
//! (cpal's `Stream::now`, which on WASAPI is `QueryPerformanceCounter`), then
//! [`deliver`] starts or releases the monitor voice and hands the press to the
//! law thread, through two `rtrb` rings made before the port opened. It calls
//! no multimedia function, allocates nothing and takes no lock; a full ring
//! drops the message rather than wait. WinMM's invalid-message notice,
//! `MIM_ERROR`, is counted with one atomic add, and `jam` reports the count.
//!
//! A keyboard unplugged mid-jam sends WinMM nothing, so the callback never
//! hears of it. `jam` watches the port count instead ([`port_count`]) and
//! says when it drops.
//!
//! Two premises here are WinMM's, and midir rests on the same two: WinMM calls
//! the callback for one port serially, and never after `midiInClose` has
//! returned. The owner's first plug-in of a keyboard is their first test on a
//! real device.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cpal::traits::StreamTrait;
use rtrb::Producer;
use windows::Win32::Media::Audio::{
    CALLBACK_FUNCTION, HMIDIIN, MIDIINCAPSW, midiInClose, midiInGetDevCapsW, midiInGetNumDevs,
    midiInOpen, midiInReset, midiInStart, midiInStop,
};
use windows::Win32::Media::{MM_MIM_DATA, MM_MIM_ERROR, MMSYSERR_NOERROR};

use crate::device::nanos;
use crate::event::Monitor;
use crate::live::{Dropped, Press, Stamp, midi_press};

/// How many MIDI input ports WinMM lists now.
pub fn port_count() -> u32 {
    // SAFETY: no arguments; it only counts devices.
    unsafe { midiInGetNumDevs() }
}

/// The MIDI input ports, by WinMM index, with their names.
pub fn ports() -> Vec<String> {
    // SAFETY: no arguments; it only counts devices.
    let count = unsafe { midiInGetNumDevs() };
    (0..count)
        .map(|index| {
            let mut caps = MIDIINCAPSW::default();
            let size = u32::try_from(size_of::<MIDIINCAPSW>()).unwrap_or(0);
            // SAFETY: `caps` is a writable MIDIINCAPSW of exactly `size` bytes.
            let status = unsafe { midiInGetDevCapsW(index as usize, &mut caps, size) };
            if status != MMSYSERR_NOERROR {
                return format!("(port {index}: its name did not read, error {status})");
            }
            // Copied out of the packed struct before it is read.
            let name: [u16; 32] = caps.szPname;
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            String::from_utf16_lossy(name.get(..end).unwrap_or(&[]))
        })
        .collect()
}

/// Where a message goes: the monitor voice, the law thread, and the stream
/// clock that stamps its arrival; and the count of `MIM_ERROR` notices.
pub struct Sink {
    pub monitor: Producer<Monitor>,
    pub input: Producer<Press>,
    pub stream: Arc<cpal::Stream>,
    pub errors: Arc<AtomicU64>,
    pub dropped: Arc<Dropped>,
}

/// An open MIDI input. [`MidiIn::close`] stops and closes the port; dropping
/// it does the same and prints a close that fails.
pub struct MidiIn {
    handle: HMIDIIN,
    sink: *mut Sink,
    open: bool,
}

impl MidiIn {
    /// Opens WinMM input port `port` and starts it; from then on each note-on
    /// and note-off goes to `sink`.
    pub fn open(port: u32, sink: Sink) -> Result<MidiIn, String> {
        let sink = Box::into_raw(Box::new(sink));
        let mut handle = HMIDIIN(std::ptr::null_mut());
        let callback = on_message as unsafe extern "system" fn(HMIDIIN, u32, usize, usize, usize);
        // SAFETY: `handle` is writable; the callback has MidiInProc's
        // signature; `sink` stays valid until `Drop` closes the port.
        let status = unsafe {
            midiInOpen(
                &mut handle,
                port,
                Some(callback as usize),
                Some(sink as usize),
                CALLBACK_FUNCTION,
            )
        };
        if status != MMSYSERR_NOERROR {
            // SAFETY: WinMM refused the port, so nothing else holds `sink`.
            drop(unsafe { Box::from_raw(sink) });
            return Err(format!(
                "MIDI port {port} did not open (WinMM error {status})"
            ));
        }
        // SAFETY: `handle` was just opened.
        let status = unsafe { midiInStart(handle) };
        if status != MMSYSERR_NOERROR {
            // SAFETY: the port is open and not started; it is closed once,
            // here.
            let closed = unsafe { midiInClose(handle) };
            if closed != MMSYSERR_NOERROR {
                // WinMM kept the port, and may still call the callback: the
                // context is left allocated, as MidiIn::close leaves it.
                return Err(format!(
                    "MIDI port {port} did not start (WinMM error {status}) and did not close \
                     (WinMM error {closed}); its callback's context is left allocated, since \
                     WinMM may still call it"
                ));
            }
            // SAFETY: midiInClose returned with success, so WinMM calls the
            // callback no more, and `sink` is freed once, with nothing using
            // it.
            drop(unsafe { Box::from_raw(sink) });
            return Err(format!(
                "MIDI port {port} did not start (WinMM error {status})"
            ));
        }
        Ok(MidiIn {
            handle,
            sink,
            open: true,
        })
    }

    /// Stops, resets and closes the port, then frees the callback's context.
    ///
    /// WinMM calls the callback no more once `midiInClose` has returned, so
    /// the context is freed only after a close that succeeded. A close that
    /// fails leaves the port with WinMM, whose callback may still run, so the
    /// context is left allocated, never freed, and the failure is returned.
    pub fn close(mut self) -> Result<(), String> {
        self.shut()
    }

    fn shut(&mut self) -> Result<(), String> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        // SAFETY: the port is open; stopping and resetting it end its input.
        unsafe {
            midiInStop(self.handle);
            midiInReset(self.handle);
        }
        // SAFETY: the port is open and is closed once, here.
        let status = unsafe { midiInClose(self.handle) };
        if status != MMSYSERR_NOERROR {
            return Err(format!(
                "the MIDI port did not close (WinMM error {status}); its callback's context \
                 is left allocated, since WinMM may still call it"
            ));
        }
        // SAFETY: midiInClose returned with success, so WinMM calls the
        // callback no more, and `sink` is freed once, with nothing using it.
        drop(unsafe { Box::from_raw(self.sink) });
        Ok(())
    }
}

impl Drop for MidiIn {
    fn drop(&mut self) {
        if let Err(e) = self.shut() {
            eprintln!("host: {e}");
        }
    }
}

/// WinMM's MidiInProc. `instance` is the port's [`Sink`]; `param1` holds a
/// short message (status, then two data bytes) and `param2` its time in
/// milliseconds since `midiInStart`.
unsafe extern "system" fn on_message(
    _handle: HMIDIIN,
    message: u32,
    instance: usize,
    param1: usize,
    param2: usize,
) {
    if instance == 0 || (message != MM_MIM_DATA && message != MM_MIM_ERROR) {
        return;
    }
    if message == MM_MIM_ERROR {
        // SAFETY: as below; only the counter is read.
        let sink = unsafe { &*(instance as *const Sink) };
        sink.errors.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // SAFETY: `instance` is the Sink the port was opened with. WinMM calls
    // this for one port serially, and never after midiInClose returns, so this
    // is the only reference to it while the call runs.
    let sink = unsafe { &mut *(instance as *mut Sink) };
    let arrived = nanos(sink.stream.now());
    deliver(
        &mut sink.monitor,
        &mut sink.input,
        &sink.dropped,
        arrived,
        param1,
        param2,
    );
}

/// The callback's work once it has read the clock: a note-on or note-off,
/// arrived at stream instant `arrived`, goes to the monitor and to the law
/// thread with its WinMM time; any other message is dropped. It only pushes
/// into rings, which never allocate or block, and counts a message a full
/// ring could not take (`alloc_free` counts its allocations: none).
pub(crate) fn deliver(
    monitor: &mut Producer<Monitor>,
    input: &mut Producer<Press>,
    dropped: &Dropped,
    arrived: u64,
    param1: usize,
    param2: usize,
) {
    let Some((down, pitch, velocity)) = midi_press(param1 as u32) else {
        return;
    };
    let heard = monitor.push(if down {
        Monitor::On { pitch, velocity }
    } else {
        Monitor::Off { pitch }
    });
    if heard.is_err() {
        dropped.monitor.fetch_add(1, Ordering::Relaxed);
    }
    let passed = input.push(Press {
        down,
        pitch,
        velocity,
        stamp: Stamp::Midi {
            micros: (param2 as u64).saturating_mul(1_000),
            arrived,
        },
    });
    if passed.is_err() {
        dropped.presses.fetch_add(1, Ordering::Relaxed);
    }
}
