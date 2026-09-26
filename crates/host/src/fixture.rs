//! A synthetic sample set for the tests, so no test needs the 742 MB
//! download.
//!
//! [`flac`] writes a real FLAC file, 48 kHz stereo in 24 bits, whose
//! subframes are all VERBATIM: the samples themselves, big-endian, with the
//! header CRC-8 and the frame CRC-16 libFLAC computes. claxon decodes it the
//! way it decodes the real set, CRCs checked.
//!
//! Every fixture sample names itself in its first frame: the left channel is
//! the sample's peak, 8,000,000, and the right channel is its code times
//! 1,000 ([`code`]). Scaling to the peak keeps their ratio, so a sample, or a
//! voice playing it on its own key, can be told from its first frame
//! ([`identify`], [`code_of`]). After that frame:
//! - a note sample is a triangle wave whose period is `40 + 2 × zone` samples
//!   and whose height is `200,000 × (layer + 1)`, on the left, with half of it
//!   on the right;
//! - a hammer release alternates between +300,000 and -300,000 on both
//!   channels.
//!
//! Everything is integer arithmetic, so the fixture is the same bytes on every
//! machine.

use crate::piano::{Bank, File, LAYERS, Needs, Sample};

/// The first frame's left value: every sample's peak.
pub const MARK: i32 = 8_000_000;

/// Frames per FLAC block.
const BLOCK: usize = 4_096;

/// A file's code: `zone × 16 + layer + 1` for a note, `1,000 + key` for a
/// hammer release.
pub fn code(file: File) -> i32 {
    match file {
        File::Note { zone, layer } => i32::try_from(zone * LAYERS + layer + 1).unwrap(),
        File::Release { key } => 1_000 + i32::from(key),
    }
}

/// The file a code names.
fn file_of(code: i32) -> File {
    if code > 1_000 {
        File::Release {
            key: u8::try_from(code - 1_000).unwrap(),
        }
    } else {
        let n = usize::try_from(code - 1).unwrap();
        File::Note {
            zone: n / LAYERS,
            layer: n % LAYERS,
        }
    }
}

/// The code a first frame carries, from its two channels as heard: the right
/// over the left, times the mark over 1,000.
pub fn code_of(left: f32, right: f32) -> i32 {
    (f64::from(right) / f64::from(left) * f64::from(MARK) / 1_000.0).round() as i32
}

/// The file a decoded fixture sample was made as.
pub fn identify(sample: &Sample) -> File {
    file_of(code_of(f32::from(sample.pcm[0]), f32::from(sample.pcm[1])))
}

/// The value of a fixture sample at `frame`, on `channel`.
pub fn value(file: File, frame: usize, channel: usize) -> i32 {
    if frame == 0 {
        return if channel == 0 {
            MARK
        } else {
            code(file) * 1_000
        };
    }
    match file {
        File::Note { zone, layer } => {
            let period = i64::try_from(40 + 2 * zone).unwrap();
            let height = 200_000 * i64::try_from(layer + 1).unwrap();
            let m = i64::try_from(frame).unwrap() % period;
            let left = if 2 * m < period {
                height - 4 * height * m / period
            } else {
                -3 * height + 4 * height * m / period
            };
            let v = if channel == 0 { left } else { left / 2 };
            i32::try_from(v).unwrap()
        }
        File::Release { .. } => {
            if frame.is_multiple_of(2) {
                300_000
            } else {
                -300_000
            }
        }
    }
}

/// The FLAC bytes of a fixture sample `frames` long.
pub fn sample_bytes(file: File, frames: usize) -> Vec<u8> {
    flac(frames, |frame, channel| value(file, frame, channel))
}

/// The samples `needs` names, `frames(file)` frames long each, loaded through
/// the bank's own loader and claxon.
pub fn bank(needs: &Needs, frames: impl Fn(File) -> usize + Sync) -> Bank {
    Bank::load(needs, |file| Ok(sample_bytes(file, frames(file)))).unwrap()
}

/// A FLAC file of `frames` stereo frames at 48 kHz in 24 bits:
/// `sample(frame, channel)` is each value, which must fit 24 bits.
pub fn flac(frames: usize, sample: impl Fn(usize, usize) -> i32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"fLaC");
    // The one metadata block, STREAMINFO: last, type 0, 34 bytes.
    out.extend_from_slice(&[0x80, 0x00, 0x00, 34]);
    let block = u16::try_from(BLOCK).unwrap();
    out.extend_from_slice(&block.to_be_bytes());
    out.extend_from_slice(&block.to_be_bytes());
    // Minimum and maximum frame sizes: unknown.
    out.extend_from_slice(&[0; 6]);
    // 20 bits of rate, 3 of channels - 1, 5 of bits - 1, 36 of frames.
    let packed: u64 =
        (48_000u64 << 44) | (1u64 << 41) | (23u64 << 36) | u64::try_from(frames).unwrap();
    out.extend_from_slice(&packed.to_be_bytes());
    // The MD5 of the audio: zero, which FLAC reads as not computed.
    out.extend_from_slice(&[0; 16]);

    for (number, start) in (0..frames).step_by(BLOCK).enumerate() {
        let size = (frames - start).min(BLOCK);
        let mut frame = vec![
            0xFF,
            0xF8,
            // Block size from the end of the header (16 bits); 48 kHz.
            0b0111_1010,
            // Two independent channels; 24 bits; reserved 0.
            0b0001_1100,
        ];
        frame.extend_from_slice(&utf8(number));
        frame.extend_from_slice(&u16::try_from(size - 1).unwrap().to_be_bytes());
        frame.push(crc8(&frame));
        for channel in 0..2 {
            // Padding 0, type VERBATIM (000001), no wasted bits.
            frame.push(0b0000_0010);
            for i in start..start + size {
                let v = sample(i, channel);
                assert!((-8_388_608..8_388_608).contains(&v), "{v} is not 24 bits");
                let bytes = v.to_be_bytes();
                frame.extend_from_slice(&bytes[1..]);
            }
        }
        let crc = crc16(&frame);
        frame.extend_from_slice(&crc.to_be_bytes());
        out.extend_from_slice(&frame);
    }
    out
}

/// A frame number as FLAC codes it, the way UTF-8 codes a character.
fn utf8(n: usize) -> Vec<u8> {
    let n = u32::try_from(n).unwrap();
    let low = |shift: u32| 0x80 | u8::try_from((n >> shift) & 0x3F).unwrap();
    match n {
        0..0x80 => vec![u8::try_from(n).unwrap()],
        0x80..0x800 => vec![0xC0 | u8::try_from(n >> 6).unwrap(), low(0)],
        _ => vec![0xE0 | u8::try_from(n >> 12).unwrap(), low(6), low(0)],
    }
}

/// CRC-8 as FLAC's frame header has it: x^8 + x^2 + x + 1, from 0.
fn crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in bytes {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// CRC-16 as FLAC's frame footer has it: x^16 + x^15 + x^2 + 1, from 0.
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in bytes {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x8005
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CRCs are libFLAC's: its check values for "123456789".
    #[test]
    fn the_crcs_are_flacs() {
        assert_eq!(crc8(b"123456789"), 0xF4);
        assert_eq!(crc16(b"123456789"), 0xFEE8);
    }

    /// A fixture file several blocks long decodes to every value it was
    /// written with, across the block boundaries.
    #[test]
    fn a_fixture_file_decodes_to_what_was_written() {
        let file = File::Note { zone: 3, layer: 5 };
        let frames = 2 * BLOCK + 17;
        let bytes = sample_bytes(file, frames);
        let mut reader = claxon::FlacReader::new(std::io::Cursor::new(bytes)).unwrap();
        let got: Vec<i32> = reader.samples().map(Result::unwrap).collect();
        assert_eq!(got.len(), 2 * frames);
        for (i, v) in got.iter().enumerate() {
            assert_eq!(*v, value(file, i / 2, i % 2), "value {i}");
        }
    }
}
