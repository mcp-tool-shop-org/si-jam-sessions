//! The grand piano's samples: which file plays which key at which velocity,
//! and loading them into memory before any stream exists.
//!
//! The samples are the Salamander Grand Piano V3 by Alexander Holm, a Yamaha
//! C5 recorded at 48 kHz in 24 bits, under CC BY 3.0 ([`CREDIT`]). They are
//! never committed: `host fetch-piano` downloads FreePats' FLAC edition, checks
//! it against a pinned SHA-256 and unpacks it ([`crate::fetch`]). The layout
//! below is the one the edition's own SFZ file
//! (`SalamanderGrandPiano-V3+20200602.sfz`) plays:
//!
//! - **Notes.** 30 sampled keys, every third key from A0 (MIDI 21) to C8
//!   (108): A0, C1, D#1, F#1, and so on. Each plays its own key and the keys a
//!   semitone either side of it (A0 and C8 have one neighbour on the
//!   keyboard), pitched by resampling ([`crate::sampler`]). The SFZ plays them
//!   as recorded, so the FreePats "retuned" SFZ's per-sample cents are not
//!   applied.
//! - **Velocity layers.** 16 per key, `samples/<key>v<layer>.flac`; a note's
//!   velocity picks the layer by the SFZ's `hivel` bounds ([`LAYER_TOP`]).
//! - **Hammer-noise releases.** One per key, chromatic, `samples/rel<n>.flac`
//!   for key `20 + n`, played when the note ends.
//! - Not used: the string-resonance releases (`harm*`) and the pedal noises
//!   (`pedal*`) belong to the sustain pedal, and the law carries no pedal: a
//!   score writes sustain as note lengths.
//!
//! # Memory
//!
//! A sample is decoded with claxon, a pure-Rust FLAC decoder (Apache-2.0),
//! and kept as 16-bit stereo scaled so its own loudest point is ±32,767, with
//! the scale kept beside it ([`Sample::unit`]). Relative to each sample's own
//! peak, the rounding sits 101 dB down, below the recordings' own noise; 16
//! bits halve what 32-bit floats would hold. The 480 note samples are 6,595
//! seconds of stereo audio in all, so the whole set is 1,208 MiB this way and
//! would be 2,415 MiB as floats; the 88 hammer releases add 7 MiB.
//!
//! Every velocity layer is kept, but only the samples a piece needs are
//! loaded ([`Needs`]): every note the piano plays is committed or read before
//! the stream starts, so the set is known then. A piece that used every key at
//! every layer would hold all 1,215 MiB.

use std::io::Cursor;
use std::path::Path;
use std::thread;

/// The credit the licence asks for, printed whenever the piano plays and
/// written into every WAV it renders.
pub const CREDIT: &str = "Piano samples: Salamander Grand Piano V3 by Alexander Holm, CC BY 3.0, \
                          https://creativecommons.org/licenses/by/3.0/";

/// The lowest and highest keys the piano has: A0 and C8.
pub const LOWEST: u8 = 21;
pub const HIGHEST: u8 = 108;
/// Keys on the piano.
pub const KEYS: usize = 88;
/// Sampled keys: every third key from A0.
pub const ZONES: usize = 30;
/// Velocity layers per sampled key.
pub const LAYERS: usize = 16;

/// The highest velocity each layer plays, from the SFZ's `hivel` (the 16th
/// has none and plays to 127). The layers are uneven: the SFZ balances their
/// loudness this way.
pub const LAYER_TOP: [u8; LAYERS] = [
    26, 34, 36, 43, 46, 50, 56, 64, 72, 80, 88, 96, 104, 112, 120, 127,
];

/// The pitch-class names the sampled keys use in their file names.
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// The zone that plays `key`, and the key's distance from the zone's sampled
/// key in semitones (-1, 0 or +1), or `None` for a key the piano does not have.
pub fn zone(key: u8) -> Option<(usize, i8)> {
    if !(LOWEST..=HIGHEST).contains(&key) {
        return None;
    }
    let from_lowest = usize::from(key - LOWEST);
    let zone = (from_lowest + 1) / 3;
    let offset = i8::try_from(from_lowest).ok()? - i8::try_from(zone * 3).ok()?;
    Some((zone, offset))
}

/// The key a zone was sampled at.
pub fn zone_key(zone: usize) -> u8 {
    LOWEST + u8::try_from(zone * 3).unwrap_or(0)
}

/// The layer, 0 to 15, a velocity plays. Velocity 0 is not a note; it plays
/// the first layer here.
pub fn layer(velocity: u8) -> usize {
    LAYER_TOP
        .iter()
        .position(|&top| velocity <= top)
        .unwrap_or(LAYERS - 1)
}

/// One file of the set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum File {
    /// A zone's sample at a velocity layer (both counted from 0).
    Note { zone: usize, layer: usize },
    /// A key's hammer-noise release.
    Release { key: u8 },
}

impl File {
    /// The file's path in the set, as the SFZ names it.
    pub fn name(&self) -> String {
        match *self {
            File::Note { zone, layer } => {
                let key = zone_key(zone);
                let name = NAMES.get(usize::from(key % 12)).copied().unwrap_or("?");
                let octave = i32::from(key) / 12 - 1;
                format!("samples/{name}{octave}v{}.flac", layer + 1)
            }
            File::Release { key } => format!("samples/rel{}.flac", key.saturating_sub(20)),
        }
    }
}

/// The samples a piece needs: the zone and layer of every note it plays, and
/// the hammer release of every key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Needs {
    notes: [[bool; LAYERS]; ZONES],
    releases: [bool; KEYS],
}

impl Default for Needs {
    fn default() -> Self {
        Needs {
            notes: [[false; LAYERS]; ZONES],
            releases: [false; KEYS],
        }
    }
}

impl Needs {
    /// Every file of the set.
    pub fn all() -> Needs {
        Needs {
            notes: [[true; LAYERS]; ZONES],
            releases: [true; KEYS],
        }
    }

    /// Adds the samples a note of `key` at `velocity` plays. False for a key
    /// the piano does not have, which needs nothing.
    pub fn note(&mut self, key: u8, velocity: u8) -> bool {
        let Some((zone, _)) = zone(key) else {
            return false;
        };
        if let Some(slot) = self
            .notes
            .get_mut(zone)
            .and_then(|z| z.get_mut(layer(velocity)))
        {
            *slot = true;
        }
        if let Some(slot) = self.releases.get_mut(usize::from(key - LOWEST)) {
            *slot = true;
        }
        true
    }

    /// The files needed, notes first.
    pub fn files(&self) -> Vec<File> {
        let mut files = Vec::new();
        for (zone, layers) in self.notes.iter().enumerate() {
            for (layer, needed) in layers.iter().enumerate() {
                if *needed {
                    files.push(File::Note { zone, layer });
                }
            }
        }
        for (key, needed) in (LOWEST..).zip(self.releases.iter()) {
            if *needed {
                files.push(File::Release { key });
            }
        }
        files
    }
}

/// One decoded sample: interleaved stereo, left first.
pub struct Sample {
    /// Each value is the recording's scaled so the loudest point of either
    /// channel is ±32,767, rounded to the nearest (halves away from zero).
    pub pcm: Box<[i16]>,
    /// One unit of `pcm` as a fraction of the recording's full scale.
    pub unit: f32,
}

impl Sample {
    /// Stereo frames.
    pub fn frames(&self) -> usize {
        self.pcm.len() / 2
    }
}

/// Decodes one FLAC file of the set: 48 kHz stereo, at most 24 bits. `name`
/// names it in an error.
pub fn decode(name: &str, bytes: &[u8]) -> Result<Sample, String> {
    let bad = |what: String| format!("{name}: {what}");
    let mut reader =
        claxon::FlacReader::new(Cursor::new(bytes)).map_err(|e| bad(format!("not FLAC: {e}")))?;
    let info = reader.streaminfo();
    if info.sample_rate != crate::synth::RATE || info.channels != 2 {
        return Err(bad(format!(
            "{} Hz with {} channels, not 48,000 Hz stereo",
            info.sample_rate, info.channels
        )));
    }
    let bits = info.bits_per_sample;
    if !(8..=24).contains(&bits) {
        return Err(bad(format!("{bits} bits a sample, not 8 to 24")));
    }
    let expected = info
        .samples
        .and_then(|s| usize::try_from(s).ok())
        .ok_or_else(|| bad(String::from("its length is not stated")))?;
    let mut raw: Vec<i32> = Vec::with_capacity(expected.saturating_mul(2));
    let mut frames = reader.blocks();
    let mut buffer = Vec::new();
    loop {
        match frames.read_next_or_eof(buffer) {
            Ok(Some(block)) => {
                for (l, r) in block.channel(0).iter().zip(block.channel(1)) {
                    raw.push(*l);
                    raw.push(*r);
                }
                buffer = block.into_buffer();
            }
            Ok(None) => break,
            Err(e) => return Err(bad(format!("does not decode: {e}"))),
        }
    }
    if raw.len() != expected.saturating_mul(2) {
        return Err(bad(format!(
            "decoded {} frames where its header states {expected}",
            raw.len() / 2
        )));
    }
    let peak = raw.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    let pcm: Box<[i16]> = raw.iter().map(|&s| scale(s, peak)).collect();
    let full = f32::from(32_767u16) * (1u32 << (bits - 1)) as f32;
    // Both are exact in f32 (a peak below 2^24; 32,767 times a power of two),
    // so the quotient is the one correctly rounded value on every machine.
    let unit = if peak == 0 { 0.0 } else { peak as f32 / full };
    Ok(Sample { pcm, unit })
}

/// `value * 32,767 / peak`, rounded to the nearest, halves away from zero, in
/// integers.
fn scale(value: i32, peak: u32) -> i16 {
    if peak == 0 {
        return 0;
    }
    let n = i64::from(value.unsigned_abs()) * 32_767;
    let p = i64::from(peak);
    let magnitude = (2 * n + p) / (2 * p);
    let signed = if value < 0 { -magnitude } else { magnitude };
    i16::try_from(signed).unwrap_or(if value < 0 { i16::MIN } else { i16::MAX })
}

/// The samples of a piece, decoded: what the sampler plays from.
pub struct Bank {
    notes: Vec<Option<Sample>>,
    releases: Vec<Option<Sample>>,
}

impl Bank {
    /// Loads every file `needs` names, reading each through `read`, on as
    /// many threads as the machine offers. The result does not depend on the
    /// threads: each sample goes to its own slot.
    pub fn load(
        needs: &Needs,
        read: impl Fn(File) -> Result<Vec<u8>, String> + Sync,
    ) -> Result<Bank, String> {
        let files = needs.files();
        let threads = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, 16);
        let chunk = files.len().div_ceil(threads).max(1);
        let decoded: Vec<Result<Vec<(File, Sample)>, String>> = thread::scope(|scope| {
            let workers: Vec<_> = files
                .chunks(chunk)
                .map(|part| {
                    let read = &read;
                    scope.spawn(move || {
                        part.iter()
                            .map(|&file| {
                                let bytes = read(file)?;
                                decode(&file.name(), &bytes).map(|s| (file, s))
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|w| {
                    w.join()
                        .unwrap_or_else(|_| Err(String::from("a sample loader stopped")))
                })
                .collect()
        });
        let mut bank = Bank {
            notes: (0..ZONES * LAYERS).map(|_| None).collect(),
            releases: (0..KEYS).map(|_| None).collect(),
        };
        for part in decoded {
            for (file, sample) in part? {
                let slot = match file {
                    File::Note { zone, layer } => bank.notes.get_mut(zone * LAYERS + layer),
                    File::Release { key } => bank.releases.get_mut(usize::from(key - LOWEST)),
                };
                if let Some(slot) = slot {
                    *slot = Some(sample);
                }
            }
        }
        Ok(bank)
    }

    /// Loads what `needs` names from `dir`, a directory `host fetch-piano`
    /// unpacked and verified ([`crate::fetch::verified`]).
    pub fn open(dir: &Path, needs: &Needs) -> Result<Bank, String> {
        crate::fetch::verified(dir)?;
        Bank::load(needs, |file| {
            let path = dir.join(file.name());
            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
        })
    }

    /// The sample of a zone at a layer, if it was loaded.
    pub fn note(&self, zone: usize, layer: usize) -> Option<&Sample> {
        self.notes.get(zone * LAYERS + layer)?.as_ref()
    }

    /// A key's hammer release, if it was loaded.
    pub fn release(&self, key: u8) -> Option<&Sample> {
        self.releases
            .get(usize::from(key.checked_sub(LOWEST)?))?
            .as_ref()
    }

    /// How many samples are loaded, and the bytes of audio they hold.
    pub fn size(&self) -> (usize, usize) {
        let loaded = self.notes.iter().chain(&self.releases).flatten();
        let (count, values) = loaded.fold((0, 0), |(n, v), s| (n + 1, v + s.pcm.len()));
        (count, values * size_of::<i16>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    /// Every key from A0 to C8 plays the nearest sampled key, at most a
    /// semitone away; keys off the keyboard play nothing. The zones are the
    /// SFZ's `lokey`/`hikey` ranges.
    #[test]
    fn every_key_plays_its_nearest_sampled_key() {
        assert_eq!(zone(20), None);
        assert_eq!(zone(109), None);
        assert_eq!(zone(21), Some((0, 0)));
        assert_eq!(zone(22), Some((0, 1)));
        assert_eq!(zone(23), Some((1, -1)));
        assert_eq!(zone(24), Some((1, 0)));
        assert_eq!(zone(25), Some((1, 1)));
        assert_eq!(zone(60), Some((13, 0)), "C4");
        assert_eq!(zone(88), Some((22, 1)), "E6 on D#6's sample");
        assert_eq!(zone(89), Some((23, -1)), "F6 on F#6's sample");
        assert_eq!(zone(107), Some((29, -1)));
        assert_eq!(zone(108), Some((29, 0)));
        for key in LOWEST..=HIGHEST {
            let (z, offset) = zone(key).unwrap();
            assert!(z < ZONES && (-1..=1).contains(&offset), "{key}");
            assert_eq!(i32::from(zone_key(z)) + i32::from(offset), i32::from(key));
        }
        assert_eq!(zone_key(ZONES - 1), HIGHEST);
    }

    /// The layers' velocity bounds are the SFZ's.
    #[test]
    fn a_velocity_plays_the_sfz_layer() {
        let bounds = [
            (1, 0),
            (26, 0),
            (27, 1),
            (34, 1),
            (35, 2),
            (36, 2),
            (37, 3),
            (64, 7),
            (65, 8),
            (120, 14),
            (121, 15),
            (127, 15),
        ];
        for (velocity, want) in bounds {
            assert_eq!(layer(velocity), want, "velocity {velocity}");
        }
    }

    /// The file names are the edition's.
    #[test]
    fn the_file_names_are_the_editions() {
        let name = |zone, layer| File::Note { zone, layer }.name();
        assert_eq!(name(0, 0), "samples/A0v1.flac");
        assert_eq!(name(1, 15), "samples/C1v16.flac");
        assert_eq!(name(2, 9), "samples/D#1v10.flac");
        assert_eq!(name(3, 0), "samples/F#1v1.flac");
        assert_eq!(name(13, 7), "samples/C4v8.flac");
        assert_eq!(name(29, 15), "samples/C8v16.flac");
        assert_eq!(File::Release { key: 21 }.name(), "samples/rel1.flac");
        assert_eq!(File::Release { key: 108 }.name(), "samples/rel88.flac");
        let all = Needs::all().files();
        assert_eq!(all.len(), ZONES * LAYERS + KEYS);
        let names: std::collections::BTreeSet<String> = all.iter().map(File::name).collect();
        assert_eq!(names.len(), all.len(), "no two files share a name");
    }

    /// A note needs its zone at its layer and its key's hammer release;
    /// a key off the keyboard needs nothing.
    #[test]
    fn a_piece_needs_what_its_notes_play() {
        let mut needs = Needs::default();
        assert!(needs.note(61, 100));
        assert!(needs.note(62, 20));
        assert!(!needs.note(109, 100));
        assert_eq!(
            needs.files(),
            [
                File::Note {
                    zone: 13,
                    layer: 12
                },
                File::Note { zone: 14, layer: 0 },
                File::Release { key: 61 },
                File::Release { key: 62 },
            ]
        );
    }

    /// A FLAC file decodes to the values it holds: the fixture's writer and
    /// claxon agree to the bit, and the 16-bit scaling rounds each value to
    /// the nearest step of the sample's own peak.
    #[test]
    fn a_sample_decodes_and_scales_to_its_own_peak() {
        let values = [0, 1, -1, 4_194_304, -8_388_607, 123_457, -2];
        let bytes = fixture::flac(values.len(), |frame, channel| {
            if channel == 0 {
                values[frame]
            } else {
                -values[frame] / 2
            }
        });
        let sample = decode("test", &bytes).unwrap();
        assert_eq!(sample.frames(), values.len());
        let peak = 8_388_607.0f64;
        for (frame, &value) in values.iter().enumerate() {
            for (channel, v) in [(0, value), (1, -value / 2)] {
                let got = f64::from(sample.pcm[frame * 2 + channel]);
                let want = f64::from(v) * 32_767.0 / peak;
                assert!((got - want).abs() <= 0.5, "{frame}/{channel}: {got} {want}");
            }
        }
        assert_eq!(sample.pcm[8], -32_767, "the peak is full scale");
        // One unit, times 32,767, is the peak as a fraction of 24-bit full
        // scale.
        let back = f64::from(sample.unit) * 32_767.0 * 8_388_608.0;
        assert!((back - peak).abs() < 1.0, "{back}");
    }

    /// A file that is not FLAC, or not 48 kHz stereo, is refused by name.
    #[test]
    fn a_file_the_piano_cannot_play_is_refused_by_name() {
        let e = decode("samples/C4v1.flac", b"RIFF....WAVE").err().unwrap();
        assert!(e.starts_with("samples/C4v1.flac: not FLAC"), "{e}");
        let mut mono = fixture::flac(10, |_, _| 5);
        // STREAMINFO's channel bits: 3 bits of (channels - 1) at byte 20.
        mono[20] &= !0b0000_1110;
        let e = decode("x.flac", &mono).err().unwrap();
        assert!(e.contains("1 channels"), "{e}");
    }

    /// The whole set's layout loads: every one of the 480 note files and 88
    /// hammer releases is read by its name and lands in its slot.
    #[test]
    fn every_file_of_the_set_loads_into_its_slot() {
        let bank = fixture::bank(&Needs::all(), |_| 48);
        assert_eq!(
            bank.size(),
            (ZONES * LAYERS + KEYS, (ZONES * LAYERS + KEYS) * 48 * 2 * 2)
        );
        for zone in 0..ZONES {
            for layer in 0..LAYERS {
                let s = bank.note(zone, layer).unwrap();
                assert_eq!(fixture::identify(s), File::Note { zone, layer });
            }
        }
        for key in LOWEST..=HIGHEST {
            assert_eq!(
                fixture::identify(bank.release(key).unwrap()),
                File::Release { key }
            );
        }
    }

    /// The real set, from the directory `SI_JAM_PIANO` names, which
    /// `fetch-piano` filled: all 16 layers of all 30 sampled keys and the 88
    /// hammer releases decode, each to the length its header states, at 48 kHz
    /// stereo in 24 bits, and the audio they hold is measured. CI's
    /// dispatch-only piano job runs it; by hand, `SI_JAM_PIANO=<dir> cargo
    /// test -p host --release -- --ignored`.
    #[test]
    #[ignore = "needs the real samples: SI_JAM_PIANO=<dir>"]
    fn the_real_set_loads_every_layer() {
        let dir = std::env::var_os("SI_JAM_PIANO").expect("SI_JAM_PIANO names the samples");
        let started = std::time::Instant::now();
        let bank = Bank::open(Path::new(&dir), &Needs::all()).unwrap();
        let (count, bytes) = bank.size();
        eprintln!(
            "the real set: {count} samples, {bytes} bytes ({:.1} MiB) of audio, loaded in {:.1} s",
            bytes as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
        assert_eq!(count, ZONES * LAYERS + KEYS);
        for zone in 0..ZONES {
            for layer in 0..LAYERS {
                let sample = bank.note(zone, layer).unwrap();
                assert!(sample.frames() > 48_000 && sample.unit > 0.0);
            }
        }
    }

    /// A file that cannot be read fails the load, and says which.
    #[test]
    fn a_missing_file_fails_the_load() {
        let mut needs = Needs::default();
        needs.note(60, 64);
        let e = Bank::load(&needs, |file| match file {
            File::Release { .. } => Err(format!("{}: not found", file.name())),
            _ => Ok(fixture::sample_bytes(file, 48)),
        })
        .err()
        .unwrap();
        assert_eq!(e, "samples/rel40.flac: not found");
    }
}
