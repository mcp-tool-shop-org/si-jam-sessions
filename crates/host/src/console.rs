//! The computer keyboard: the live input when no MIDI port is present.
//!
//! Two octaves on the letter rows, laid out as trackers lay them out, read
//! from the Windows console. Each key is stamped on the stream clock the
//! moment `ReadConsoleInputW` hands it over, then goes through the same clocks
//! and the same live verb as a MIDI note. Auto-repeat is ignored; Esc stops
//! the jam.
//!
//! The keys come from the console's input buffer opened by name, `CONIN$`,
//! not from standard input: a console with no window, which no keystroke from
//! the desktop can reach, still has one, and a redirected standard input does
//! not hide it.

use std::fs::{File, OpenOptions};
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::StreamTrait;
use rtrb::Producer;
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Console::{INPUT_RECORD, KEY_EVENT, ReadConsoleInputW};
use windows::Win32::System::Threading::WaitForSingleObject;

use crate::device::nanos;
use crate::event::Monitor;
use crate::live::{Press, Stamp};

/// The layout, for the person at the keyboard.
pub const LAYOUT: &str = "Z S X D C V G B H N J M , play C4 to C5; Q 2 W 3 E R 5 T 6 Y 7 U I \
                          play C5 to C6; Esc stops";

const ESCAPE: u16 = 0x1B;
const COMMA: u16 = 0xBC;

/// The console's input buffer, open for reading and writing.
struct ConsoleInput {
    file: File,
}

impl ConsoleInput {
    fn open() -> Result<ConsoleInput, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONIN$")
            .map(|file| ConsoleInput { file })
            .map_err(|e| format!("the console's input did not open (is there a console?): {e}"))
    }

    /// The handle, valid while `self` lives.
    fn handle(&self) -> HANDLE {
        HANDLE(self.file.as_raw_handle())
    }
}

/// The pitch a key plays, by its Windows virtual-key code.
pub fn pitch_of(key: u16) -> Option<u8> {
    const LOWER: [u8; 12] = *b"ZSXDCVGBHNJM";
    const UPPER: [u8; 13] = *b"Q2W3ER5T6Y7UI";
    if key == COMMA {
        return Some(72);
    }
    let byte = u8::try_from(key).ok()?;
    if let Some(i) = LOWER.iter().position(|&k| k == byte) {
        return u8::try_from(i).ok().map(|i| 60 + i);
    }
    UPPER
        .iter()
        .position(|&k| k == byte)
        .and_then(|i| u8::try_from(i).ok())
        .map(|i| 72 + i)
}

/// Reads the console's keys until Esc or `stop`, starting and releasing the
/// monitor voice and passing each press to the law thread.
pub fn read(
    stop: &AtomicBool,
    stream: &cpal::Stream,
    mut monitor: Producer<Monitor>,
    mut input: Producer<Press>,
) -> Result<(), String> {
    let input_buffer = ConsoleInput::open()?;
    let handle = input_buffer.handle();
    let mut held = [false; 256];
    let mut records = [INPUT_RECORD::default(); 32];
    while !stop.load(Ordering::Relaxed) {
        // SAFETY: `handle` is the console's input handle.
        if unsafe { WaitForSingleObject(handle, 20) } != WAIT_OBJECT_0 {
            continue;
        }
        let mut count = 0u32;
        // SAFETY: `records` is writable and `count` receives how many were.
        unsafe { ReadConsoleInputW(handle, &mut records, &mut count) }
            .map_err(|e| format!("the console's input did not read: {e}"))?;
        let arrived = nanos(stream.now());
        let read = usize::try_from(count).unwrap_or(0);
        for record in records.iter().take(read) {
            if u32::from(record.EventType) != KEY_EVENT {
                continue;
            }
            // SAFETY: a KEY_EVENT record holds a KEY_EVENT_RECORD.
            let key = unsafe { record.Event.KeyEvent };
            let down = key.bKeyDown.as_bool();
            if key.wVirtualKeyCode == ESCAPE && down {
                stop.store(true, Ordering::SeqCst);
                return Ok(());
            }
            let Some(pitch) = pitch_of(key.wVirtualKeyCode) else {
                continue;
            };
            let Some(was) = held.get_mut(usize::from(key.wVirtualKeyCode & 0xFF)) else {
                continue;
            };
            if *was == down {
                continue;
            }
            *was = down;
            let _ = monitor.push(if down {
                Monitor::On {
                    pitch,
                    velocity: 100,
                }
            } else {
                Monitor::Off { pitch }
            });
            let _ = input.push(Press {
                down,
                pitch,
                velocity: 100,
                stamp: Stamp::Stream { nanos: arrived },
            });
        }
    }
    Ok(())
}

/// A key no layout uses, for [`measure`]: F24.
const PROBE: u16 = 0x87;

/// Times the part of a key's journey the host can time: from a key record
/// written into the console's input buffer (`WriteConsoleInputW`) to the
/// moment a reader, waiting as [`read`] waits, stamps it on the stream clock.
/// It writes into the console the command runs in, which needs no window.
/// Returns each delay in nanoseconds, `count` of them, taken 3 to 13 ms apart.
/// The keyboard's own scan and its USB polling come before the buffer and are
/// not in these numbers.
pub fn measure(stream: &cpal::Stream, count: usize) -> Result<Vec<u64>, String> {
    use std::sync::mpsc;
    use std::time::Duration;
    use windows::Win32::System::Console::{INPUT_RECORD_0, KEY_EVENT_RECORD, WriteConsoleInputW};

    let input_buffer = ConsoleInput::open()?;
    let handle = input_buffer.handle();
    let (stamped, stamps) = mpsc::channel::<u64>();
    std::thread::scope(|scope| {
        let reader = scope.spawn(move || -> Result<(), String> {
            // A handle is not Send; this thread opens its own.
            let own = ConsoleInput::open()?;
            let handle = own.handle();
            let mut records = [INPUT_RECORD::default(); 16];
            let mut seen = 0;
            while seen < count {
                // SAFETY: `handle` is the console's input handle.
                if unsafe { WaitForSingleObject(handle, 2_000) } != WAIT_OBJECT_0 {
                    return Err(String::from("no key arrived in 2 s"));
                }
                let mut read = 0u32;
                // SAFETY: `records` is writable and `read` receives the count.
                unsafe { ReadConsoleInputW(handle, &mut records, &mut read) }
                    .map_err(|e| format!("the console's input did not read: {e}"))?;
                let at = nanos(stream.now());
                for record in records.iter().take(usize::try_from(read).unwrap_or(0)) {
                    if u32::from(record.EventType) != KEY_EVENT {
                        continue;
                    }
                    // SAFETY: a KEY_EVENT record holds a KEY_EVENT_RECORD.
                    let key = unsafe { record.Event.KeyEvent };
                    if key.wVirtualKeyCode == PROBE && key.bKeyDown.as_bool() {
                        seen += 1;
                        let _ = stamped.send(at);
                    }
                }
            }
            Ok(())
        });
        let mut delays = Vec::with_capacity(count);
        let probe = INPUT_RECORD {
            EventType: u16::try_from(KEY_EVENT).unwrap_or(1),
            Event: INPUT_RECORD_0 {
                KeyEvent: KEY_EVENT_RECORD {
                    bKeyDown: true.into(),
                    wRepeatCount: 1,
                    wVirtualKeyCode: PROBE,
                    ..Default::default()
                },
            },
        };
        for i in 0..count {
            std::thread::sleep(Duration::from_millis(3 + (i as u64 * 7) % 11));
            let mut written = 0u32;
            let sent = nanos(stream.now());
            // SAFETY: one record is read from `probe`; `written` is writable.
            unsafe { WriteConsoleInputW(handle, &[probe], &mut written) }.map_err(|e| {
                format!("a key could not be written to the console (is this a terminal?): {e}")
            })?;
            let at = stamps
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| String::from("the written key was not read back"))?;
            delays.push(at.saturating_sub(sent));
        }
        reader
            .join()
            .map_err(|_| String::from("the reader stopped"))??;
        Ok(delays)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_letter_rows_are_two_octaves() {
        let lower: Vec<Option<u8>> = "ZSXDCVGBHNJM"
            .bytes()
            .map(|k| pitch_of(u16::from(k)))
            .collect();
        assert_eq!(lower, (60..72).map(Some).collect::<Vec<_>>());
        assert_eq!(pitch_of(COMMA), Some(72));
        let upper: Vec<Option<u8>> = "Q2W3ER5T6Y7UI"
            .bytes()
            .map(|k| pitch_of(u16::from(k)))
            .collect();
        assert_eq!(upper, (72..85).map(Some).collect::<Vec<_>>());
        assert_eq!(pitch_of(u16::from(b'A')), None);
        assert_eq!(pitch_of(ESCAPE), None);
    }
}
