//! The same pipeline without a device.
//!
//! `render` runs the scheduler and the synth in turn, block by block, exactly
//! as the law thread and the audio callback run beside each other with a
//! device: the scheduler steps the law to the frame the synth renders next and
//! fills the ring, and the synth renders the block from the ring. Nothing here
//! is timed, so the result is the same on every run.

use std::io::{self, Write};

use rtrb::RingBuffer;

use crate::RING_EVENTS;
use crate::bridge::{Law, Refused};
use crate::event::Event;
use crate::schedule::{Scheduler, steps_for};
use crate::synth::{Counts, RATE, Synth};

/// Every event the scheduler moves through the ring for frames `0..end`, in
/// the order the callback would take them.
pub fn events(law: &mut Law, end: u64) -> Result<Vec<Event>, Refused> {
    let (mut producer, mut consumer) = RingBuffer::new(RING_EVENTS);
    let mut scheduler = Scheduler::new(0);
    let mut out = Vec::new();
    let mut frame = 0u64;
    while frame < end {
        scheduler.pump(law, steps_for(frame, 0, None), &mut producer)?;
        let block_end = frame.saturating_add(480).min(end);
        while let Ok(event) = consumer.peek() {
            if event.onset() >= block_end {
                break;
            }
            if let Ok(event) = consumer.pop() {
                out.push(event);
            }
        }
        frame = block_end;
    }
    Ok(out)
}

/// The mono mix of frames `0..end`, rendered `block` frames at a time, and
/// what the synth counted.
pub fn render(law: &mut Law, end: u64, block: usize) -> Result<(Vec<f32>, Counts), Refused> {
    let (out, _, counts) = render_with(law, end, block, Synth::new(0))?;
    Ok((out, counts))
}

/// The mix of frames `0..end` rendered by `synth`, `block` frames at a
/// time: interleaved, with the synth's channels (one, or two with the piano),
/// which it also returns, and what the synth counted.
pub fn render_with(
    law: &mut Law,
    end: u64,
    block: usize,
    mut synth: Box<Synth>,
) -> Result<(Vec<f32>, usize, Counts), Refused> {
    let channels = if synth.stereo() { 2 } else { 1 };
    let (mut producer, mut consumer) = RingBuffer::new(RING_EVENTS);
    let mut scheduler = Scheduler::new(0);
    let frames = usize::try_from(end).unwrap_or(0);
    let mut out = vec![0.0f32; frames.saturating_mul(channels)];
    for chunk in out.chunks_mut(block.max(1) * channels) {
        scheduler.pump(law, steps_for(synth.frame(), 0, None), &mut producer)?;
        synth.render(chunk, channels, &mut consumer, None);
    }
    Ok((out, channels, synth.counts()))
}

/// Writes `samples` as a mono 32-bit float WAV at 48 kHz: the rendered
/// buffer itself, so sample `n` of the file is law sample `n`.
pub fn write_wav(out: &mut impl Write, samples: &[f32]) -> io::Result<()> {
    write_wav_with(out, samples, 1, &[])
}

/// Writes interleaved `samples` of `channels` channels as a 32-bit float WAV
/// at 48 kHz, so frame `n` of the file is law sample `n`. With `info`, a
/// `LIST` chunk of type `INFO` follows the audio, one sub-chunk per id and
/// text, each text NUL-terminated and padded to an even length: the audio's
/// header stays the 58 bytes it is without one.
pub fn write_wav_with(
    out: &mut impl Write,
    samples: &[f32],
    channels: u16,
    info: &[([u8; 4], &str)],
) -> io::Result<()> {
    let too_long = || io::Error::other("the render is longer than a WAV can hold");
    let data = u32::try_from(samples.len() * 4).map_err(|_| too_long())?;
    let mut list = Vec::new();
    if !info.is_empty() {
        list.extend_from_slice(b"INFO");
        for (id, text) in info {
            if !text.is_ascii() || text.contains('\0') {
                return Err(io::Error::other(
                    "a WAV INFO text must be ASCII without NUL",
                ));
            }
            let size = u32::try_from(text.len() + 1).map_err(|_| too_long())?;
            list.extend_from_slice(id);
            list.extend_from_slice(&size.to_le_bytes());
            list.extend_from_slice(text.as_bytes());
            list.push(0);
            if list.len() % 2 == 1 {
                list.push(0);
            }
        }
    }
    let list_chunk = if list.is_empty() { 0 } else { 8 + list.len() };
    let riff = u32::try_from(list_chunk)
        .ok()
        .and_then(|l| data.checked_add(4 + 26 + 12 + 8)?.checked_add(l))
        .ok_or_else(too_long)?;
    let block = 4 * channels;
    out.write_all(b"RIFF")?;
    out.write_all(&riff.to_le_bytes())?;
    out.write_all(b"WAVE")?;
    // fmt: 18 bytes, IEEE float, the channels, 48 kHz, 4 bytes a sample, 32
    // bits.
    out.write_all(b"fmt ")?;
    out.write_all(&18u32.to_le_bytes())?;
    out.write_all(&3u16.to_le_bytes())?;
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&RATE.to_le_bytes())?;
    out.write_all(&(RATE * u32::from(block)).to_le_bytes())?;
    out.write_all(&block.to_le_bytes())?;
    out.write_all(&32u16.to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?;
    // fact: the frame count, which a float WAV carries.
    out.write_all(b"fact")?;
    out.write_all(&4u32.to_le_bytes())?;
    out.write_all(&(data / u32::from(block.max(1))).to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data.to_le_bytes())?;
    for s in samples {
        out.write_all(&s.to_le_bytes())?;
    }
    if !list.is_empty() {
        out.write_all(b"LIST")?;
        out.write_all(
            &u32::try_from(list.len())
                .map_err(|_| too_long())?
                .to_le_bytes(),
        )?;
        out.write_all(&list)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wav_header_is_a_float_wav_at_48_khz() {
        let mut bytes = Vec::new();
        write_wav(&mut bytes, &[0.5, -0.25, 0.0]).unwrap();
        assert_eq!(bytes.len(), 58 + 12);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 70 - 8);
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 3, "IEEE float");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "mono");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            48_000
        );
        assert_eq!(&bytes[38..42], b"fact");
        assert_eq!(u32::from_le_bytes(bytes[46..50].try_into().unwrap()), 3);
        assert_eq!(&bytes[50..54], b"data");
        assert_eq!(u32::from_le_bytes(bytes[54..58].try_into().unwrap()), 12);
        assert_eq!(f32::from_le_bytes(bytes[58..62].try_into().unwrap()), 0.5);
        assert_eq!(f32::from_le_bytes(bytes[62..66].try_into().unwrap()), -0.25);
    }

    /// A stereo WAV with a credit: the same 58-byte header with two channels,
    /// the frames interleaved, and the LIST/INFO chunk after the audio, which
    /// the RIFF size counts.
    #[test]
    fn a_stereo_wav_carries_its_credit_in_a_list_info_chunk() {
        let mut bytes = Vec::new();
        let credit = "Piano samples: an example, CC BY 3.0";
        write_wav_with(
            &mut bytes,
            &[0.5, -0.5, 0.25, -0.25],
            2,
            &[(*b"ICMT", credit), (*b"ISFT", "si-jam-sessions host")],
        )
        .unwrap();
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        assert_eq!(u32_at(4) as usize, bytes.len() - 8, "RIFF size");
        assert_eq!(u16_at(22), 2, "stereo");
        assert_eq!(u32_at(28), 48_000 * 8, "byte rate");
        assert_eq!(u16_at(32), 8, "block align");
        assert_eq!(u32_at(46), 2, "two frames");
        assert_eq!(&bytes[50..54], b"data");
        assert_eq!(u32_at(54), 16);
        assert_eq!(f32::from_le_bytes(bytes[62..66].try_into().unwrap()), -0.5);
        let list = 58 + 16;
        assert_eq!(&bytes[list..list + 4], b"LIST");
        assert_eq!(u32_at(list + 4) as usize, bytes.len() - list - 8);
        assert_eq!(&bytes[list + 8..list + 12], b"INFO");
        assert_eq!(&bytes[list + 12..list + 16], b"ICMT");
        let size = u32_at(list + 16) as usize;
        assert_eq!(size, credit.len() + 1);
        assert_eq!(
            &bytes[list + 20..list + 20 + credit.len()],
            credit.as_bytes()
        );
        assert_eq!(bytes[list + 20 + credit.len()], 0);
        let next = list + 20 + size + size % 2;
        assert_eq!(&bytes[next..next + 4], b"ISFT");
        assert_eq!(bytes.len() % 2, 0);
        let e = write_wav_with(&mut Vec::new(), &[], 2, &[(*b"ICMT", "caf\u{e9}")]);
        assert!(e.is_err(), "INFO text is ASCII");
    }
}

/// The offline proof that the render puts each committed event on its
/// `onset_sample`, exactly, in the mix a person hears.
///
/// The mix is a sum of voices, so where one event starts is read by
/// difference: the same frames are rendered twice, once with every event and
/// once with that event left out. Nothing differs before the event's onset,
/// because every other event is the same; the mix differs on the onset sample
/// itself, because every voice's first sample is not zero (`synth.rs`). The
/// first index where the two renders differ is where the event landed.
///
/// PHASE-0's constructed take moves five notes of *The Entertainer*: three
/// late by 1,440, 2,160 and 2,880 samples, one early by 2,160, and one at the
/// wrong pitch. Each is found in the rendered mix at its onset, against its
/// score note and the beat before it.
#[cfg(test)]
mod proof {
    use super::*;
    use crate::event::Voice;
    use crate::score::{Piece, root};
    use crate::synth::Synth;
    use golden::take::Perturbation;
    use rtrb::RingBuffer;

    /// Renders frames `start..end` of `events` in blocks of `block` frames, as
    /// the callback does, from a synth that starts at `start`.
    fn mix(events: &[Event], start: u64, end: u64, block: usize) -> Vec<f32> {
        let due: Vec<Event> = events
            .iter()
            .copied()
            .filter(|e| e.onset() >= start && e.onset() < end)
            .collect();
        let (mut producer, mut consumer) = RingBuffer::new(due.len().max(1));
        for e in due {
            producer.push(e).unwrap();
        }
        let mut synth = Synth::new(start);
        let mut out = vec![0.0f32; usize::try_from(end - start).unwrap()];
        for chunk in out.chunks_mut(block) {
            synth.render(chunk, 1, &mut consumer, None);
        }
        assert_eq!(synth.counts().late, 0);
        assert_eq!(synth.counts().dropped, 0);
        out
    }

    /// Where `event` lands in the mix of `events` over `start..end`: the first
    /// frame at which leaving it out changes the mix.
    fn landing(events: &[Event], event: Event, start: u64, end: u64) -> u64 {
        let with = mix(events, start, end, 441);
        let without: Vec<Event> = events.iter().copied().filter(|e| *e != event).collect();
        assert_eq!(
            without.len() + 1,
            events.len(),
            "{event:?} is in the events once"
        );
        let without = mix(&without, start, end, 441);
        let first = with
            .iter()
            .zip(&without)
            .position(|(a, b)| a != b)
            .expect("leaving the event out changes the mix");
        start + first as u64
    }

    fn note(events: &[Event], voice: Voice, onset: u64, pitch: u8) -> Event {
        *events
            .iter()
            .find(|e| {
                matches!(e, Event::Note { voice: v, onset: o, pitch: p, .. }
                    if *v == voice && *o == onset && *p == pitch)
            })
            .unwrap_or_else(|| panic!("no {voice:?} note at {onset} with pitch {pitch}"))
    }

    #[test]
    fn each_perturbed_note_lands_on_its_sample_against_its_score_note_and_beat() {
        let piece = Piece::entertainer(&root()).unwrap();
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        law.admit_take(&piece.take).unwrap();
        let events = events(&mut law, piece.end + 48_000).unwrap();

        let mut checked = 0;
        for drawn in piece.drawn {
            let score = piece.score.note(drawn.note).unwrap();
            let s = score.onset_sample;
            let t = drawn.perturbation.onset(s).unwrap();
            let played = drawn.perturbation.pitch(score.pitch);
            let score_note = note(&events, Voice::Score, s, score.pitch);
            let take_note = note(&events, Voice::Take, t, played);
            let beat = *events
                .iter()
                .rev()
                .find(|e| matches!(e, Event::Beat { .. }) && e.onset() <= s.min(t))
                .unwrap();

            let start = beat.onset() - 2_400;
            let end = s.max(t) + 4_800;
            let at_score = landing(&events, score_note, start, end);
            let at_take = landing(&events, take_note, start, end);
            let at_beat = landing(&events, beat, start, end);

            assert_eq!(at_score, s, "{:?}: the score note", drawn.perturbation);
            assert_eq!(at_take, t, "{:?}: the take note", drawn.perturbation);
            assert_eq!(at_beat, beat.onset(), "{:?}: the beat", drawn.perturbation);
            let delta = i64::try_from(at_take).unwrap() - i64::try_from(at_score).unwrap();
            assert_eq!(delta, drawn.perturbation.delta_samples());
            let expected = match drawn.perturbation {
                Perturbation::Late30 => 1_440,
                Perturbation::Late45 => 2_160,
                Perturbation::Late60 => 2_880,
                Perturbation::Early45 => -2_160,
                Perturbation::WrongPitch => 0,
            };
            assert_eq!(delta, expected);
            eprintln!(
                "{:>11}: beat at {}, score note {} at {} (pitch {}), take note at {} (pitch {}): \
                 {:+} samples",
                drawn.perturbation.label(),
                at_beat,
                drawn.note.0,
                at_score,
                score.pitch,
                at_take,
                played,
                delta
            );
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    /// With the real piano (the directory `SI_JAM_PIANO` names), the whole
    /// piece renders to the same bits twice, every note started, none late,
    /// none dropped, none clipped; the SHA-256 is printed, to compare across
    /// machines. CI's dispatch-only piano job runs it.
    #[test]
    #[ignore = "needs the real samples: SI_JAM_PIANO=<dir>"]
    fn the_real_piano_renders_the_same_bits_twice() {
        use crate::piano::{Bank, Needs};
        let dir = std::env::var_os("SI_JAM_PIANO").expect("SI_JAM_PIANO names the samples");
        let piece = Piece::entertainer(&root()).unwrap();
        let mut needs = Needs::default();
        for n in piece.score.notes() {
            needs.note(n.pitch, n.velocity);
        }
        let bank = std::sync::Arc::new(Bank::open(std::path::Path::new(&dir), &needs).unwrap());
        let end = piece.end + 48_000;
        let mut runs = Vec::new();
        for _ in 0..2 {
            let mut law = Law::acquire();
            law.ingest(&piece.container).unwrap();
            law.admit_take(&piece.take).unwrap();
            let synth = Synth::new(0).with_piano(std::sync::Arc::clone(&bank));
            let (samples, channels, counts) = render_with(&mut law, end, 480, synth).unwrap();
            assert_eq!(channels, 2);
            assert_eq!(
                (counts.notes, counts.late, counts.dropped),
                (2 * 2_621, 0, 0)
            );
            assert!(samples.iter().all(|s| s.abs() < 1.0), "a sample clipped");
            let mut bytes = Vec::new();
            write_wav_with(&mut bytes, &samples, 2, &[(*b"ICMT", crate::piano::CREDIT)]).unwrap();
            runs.push(golden::run::hex(&golden::run::sha256(&bytes)));
        }
        eprintln!("the real piano's render: SHA-256 {}", runs[0]);
        assert_eq!(runs[0], runs[1]);
    }

    /// The whole piece through the interleaved pipeline, as `render` writes
    /// it: every note and beat is started once, none late and none dropped,
    /// and the mix is the mix of the events rendered in one go.
    #[test]
    fn the_whole_piece_renders_every_event_once_on_time() {
        let piece = Piece::entertainer(&root()).unwrap();
        let end = piece.end + 48_000;
        let mut law = Law::acquire();
        law.ingest(&piece.container).unwrap();
        law.admit_take(&piece.take).unwrap();
        let (rendered, counts) = render(&mut law, end, 480).unwrap();
        law.ingest(&piece.container).unwrap();
        law.admit_take(&piece.take).unwrap();
        let events = events(&mut law, end).unwrap();
        let beats = events
            .iter()
            .filter(|e| matches!(e, Event::Beat { .. }))
            .count() as u64;
        assert_eq!(
            counts,
            Counts {
                notes: 2 * 2_621,
                beats,
                late: 0,
                dropped: 0,
                monitored: 0,
                outside: 0,
            }
        );
        // In the same blocks, the same samples, bit for bit.
        assert!(rendered == mix(&events, 0, end, 480), "the mixes differ");
        // In other blocks, a voice can take another slot, so the float sum
        // runs in another order: the samples agree to rounding.
        let other = mix(&events, 0, end, 1_000);
        let worst = rendered
            .iter()
            .zip(&other)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "{worst}");
        // Nothing reaches the clamp: the voices' levels leave headroom.
        assert!(
            rendered.iter().all(|s| s.is_finite() && s.abs() < 1.0),
            "a sample clipped"
        );
    }
}
