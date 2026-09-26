//! The audio device: listing, choosing, opening, and the callback.
//!
//! A thin shell around cpal 0.18.2 (WASAPI shared mode on Windows): nothing
//! here decides anything, and none of it runs in CI, which has no audio
//! device. The callback does three things, none of which allocates, locks or
//! makes a system call:
//! 1. it pushes a [`Reading`] into the readings ring: the law sample of the
//!    buffer's first frame and cpal's prediction of when that frame is heard
//!    (`OutputCallbackInfo::timestamp().playback`, KB recipe 1532);
//! 2. it renders the buffer ([`Synth::render`]), popping committed events and
//!    the live monitor from their rings;
//! 3. it publishes how far it has rendered, and what the synth counted, in
//!    atomics the law thread reads.
//!
//! The error callback only records the error's kind and raises the stop flag;
//! the command stops the stream and reports it. cpal 0.18.2 reports buffer
//! underruns (`Xrun`) on the input path only, so an output stream is never
//! told of one; a device that disappears reports `DeviceNotAvailable`, and
//! `StreamInvalidated` (WASAPI's `AUDCLNT_E_RESOURCES_INVALIDATED`) comes when
//! the stream is suspended or another client takes the device, not when the
//! default device changes. Any of them stops the command cleanly.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, OutputCallbackInfo, SampleFormat, StreamConfig};
use rtrb::{Consumer, Producer};

use crate::anchor::Reading;
use crate::event::{Event, Monitor};
use crate::synth::{RATE, Synth};

/// The buffer the host asks WASAPI for: 480 frames, 10 ms. Shared mode may
/// round it (KB recipe 1531).
pub const REQUESTED_FRAMES: u32 = 480;

/// An output device and what the host shows of it.
pub struct Output {
    pub device: cpal::Device,
    pub name: String,
    pub default: bool,
    /// cpal says the device is Bluetooth. It does not always know: on the
    /// machine this was built on, cpal 0.18.2 reports a Bluetooth speaker's
    /// interface as S/PDIF. So a command also judges by the latency the
    /// device reports once it plays.
    pub bluetooth: bool,
}

/// Every output device of the default host, the default one marked.
pub fn outputs() -> Result<Vec<Output>, String> {
    let host = cpal::default_host();
    // Devices are compared by id: two handles on one WASAPI device need not
    // compare equal.
    let default = host.default_output_device().and_then(|d| d.id().ok());
    let devices = host
        .output_devices()
        .map_err(|e| format!("the audio host lists no output devices: {e}"))?;
    Ok(devices
        .map(|device| {
            let description = device.description().ok();
            Output {
                name: description
                    .as_ref()
                    .map_or_else(|| device.to_string(), |d| d.name().to_owned()),
                bluetooth: description.as_ref().is_some_and(|d| {
                    d.interface_type() == cpal::device_description::InterfaceType::Bluetooth
                }),
                default: default.is_some() && device.id().ok() == default,
                device,
            }
        })
        .collect())
}

/// The output named by `choice`, an index from `devices` or part of a name,
/// or the default output when there is no choice.
pub fn choose_output(choice: Option<&str>) -> Result<Output, String> {
    let all = outputs()?;
    let listed = || {
        all.iter()
            .enumerate()
            .map(|(i, o)| format!("  {i}: {}", o.name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let chosen = match choice {
        None => all.iter().position(|o| o.default),
        Some(text) => match text.parse::<usize>() {
            Ok(index) if index < all.len() => Some(index),
            _ => {
                let wanted = text.to_lowercase();
                let hits: Vec<usize> = all
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.name.to_lowercase().contains(&wanted))
                    .map(|(i, _)| i)
                    .collect();
                match hits.as_slice() {
                    [one] => Some(*one),
                    [] => None,
                    _ => {
                        return Err(format!(
                            "\"{text}\" names more than one output:\n{}",
                            listed()
                        ));
                    }
                }
            }
        },
    };
    let index = chosen.ok_or_else(|| match choice {
        None => String::from("there is no default output device"),
        Some(text) => format!(
            "no output device is \"{text}\"; the outputs are:\n{}",
            listed()
        ),
    })?;
    all.into_iter()
        .nth(index)
        .ok_or_else(|| String::from("the output list changed while it was read"))
}

/// What the callback and the error callback share with the rest of the host.
#[derive(Default)]
pub struct Shared {
    /// The law sample of the next frame to render: frames rendered so far.
    pub frames: AtomicU64,
    /// Raised by the error callback, or by the command to stop.
    pub stop: AtomicBool,
    /// The error callback's error, as [`kind_code`] numbers it; 0 for none.
    pub error: AtomicU32,
    /// The latest `playback - callback`, in nanoseconds: the output latency
    /// cpal predicts.
    pub latency_nanos: AtomicU64,
    pub callbacks: AtomicU64,
    pub largest_buffer: AtomicU64,
    pub notes: AtomicU64,
    pub beats: AtomicU64,
    pub late: AtomicU64,
    pub dropped: AtomicU64,
    pub monitored: AtomicU64,
}

/// A number for an error kind, so the error callback can store it in an
/// atomic; [`kind_text`] reads it back.
pub fn kind_code(kind: ErrorKind) -> u32 {
    match kind {
        ErrorKind::DeviceNotAvailable => 1,
        ErrorKind::StreamInvalidated => 2,
        ErrorKind::DeviceChanged => 3,
        ErrorKind::Xrun => 4,
        ErrorKind::DeviceBusy => 5,
        ErrorKind::BackendError => 6,
        _ => 7,
    }
}

/// What an error code from [`kind_code`] means for the person running the
/// host.
pub fn kind_text(code: u32) -> &'static str {
    match code {
        1 => "the output device is no longer available (it was unplugged or switched off)",
        2 => {
            "the stream was invalidated: Windows suspended it, or another program took the \
             device"
        }
        3 => "the audio route changed and the stream was moved to another device",
        4 => "a buffer underrun or overrun",
        5 => "the device is busy",
        6 => "the audio backend reported an error",
        _ => "the audio device reported an error",
    }
}

/// A running output stream.
pub struct Playing {
    pub stream: Arc<cpal::Stream>,
    pub shared: Arc<Shared>,
    pub channels: u16,
    /// The frames per callback the stream reports, if it reports it.
    pub buffer: Option<u32>,
}

/// Opens `output` at 48 kHz in f32 (KB recipe 1540), asking for a buffer of
/// [`REQUESTED_FRAMES`] (and the device's default if that is refused), and
/// starts it. The synth and the rings move into the callback; every ring was
/// created before this call, which is the only time `rtrb` allocates. With
/// `mute`, everything is rendered and counted as usual, and the device is sent
/// silence: for checking the pipeline on a device without playing through it.
pub fn start(
    output: &Output,
    mut synth: Box<Synth>,
    mut events: Consumer<Event>,
    mut monitor: Option<Consumer<Monitor>>,
    mut readings: Producer<Reading>,
    mute: bool,
) -> Result<Playing, String> {
    let supported = output
        .device
        .default_output_config()
        .map_err(|e| format!("{}: no output configuration: {e}", output.name))?;
    let channels = supported.channels();
    let shared = Arc::new(Shared::default());
    let callback_shared = Arc::clone(&shared);
    let channel_count = usize::from(channels);
    let data = move |data: &mut [f32], info: &OutputCallbackInfo| {
        let first = synth.frame();
        let stamp = info.timestamp();
        let heard = u64::try_from(stamp.playback.as_nanos()).unwrap_or(u64::MAX);
        let _ = readings.push(Reading {
            sample: first,
            nanos: heard,
        });
        if let Some(latency) = stamp.playback.checked_duration_since(stamp.callback) {
            let nanos = u64::try_from(latency.as_nanos()).unwrap_or(u64::MAX);
            callback_shared
                .latency_nanos
                .store(nanos, Ordering::Relaxed);
        }
        synth.render(data, channel_count, &mut events, monitor.as_mut());
        if mute {
            data.fill(0.0);
        }
        let s = &callback_shared;
        let counts = synth.counts();
        s.notes.store(counts.notes, Ordering::Relaxed);
        s.beats.store(counts.beats, Ordering::Relaxed);
        s.late.store(counts.late, Ordering::Relaxed);
        s.dropped.store(counts.dropped, Ordering::Relaxed);
        s.monitored.store(counts.monitored, Ordering::Relaxed);
        s.callbacks.fetch_add(1, Ordering::Relaxed);
        let frames = (data.len() / channel_count.max(1)) as u64;
        s.largest_buffer.fetch_max(frames, Ordering::Relaxed);
        s.frames.store(synth.frame(), Ordering::Release);
    };
    let error_shared = Arc::clone(&shared);
    let on_error = move |error: cpal::Error| {
        error_shared
            .error
            .store(kind_code(error.kind()), Ordering::SeqCst);
        error_shared.stop.store(true, Ordering::SeqCst);
    };
    let config = |buffer_size| StreamConfig {
        channels,
        sample_rate: RATE,
        buffer_size,
    };
    // A refused fixed size leaves the closures with the first attempt, so the
    // fallback cannot reuse them; the size is checked first instead.
    let fixed = match supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => (*min..=*max).contains(&REQUESTED_FRAMES),
        cpal::SupportedBufferSize::Unknown => true,
    };
    let buffer_size = if fixed {
        BufferSize::Fixed(REQUESTED_FRAMES)
    } else {
        BufferSize::Default
    };
    if supported.sample_format() != SampleFormat::F32 {
        eprintln!(
            "note: {} mixes in {:?}; WASAPI converts the host's f32 in shared mode",
            output.name,
            supported.sample_format()
        );
    }
    let stream = output
        .device
        .build_output_stream::<f32, _, _>(config(buffer_size), data, on_error, None)
        .map_err(|e| format!("{}: the stream did not open: {e}", output.name))?;
    let buffer = stream.buffer_size().ok();
    stream
        .play()
        .map_err(|e| format!("{}: the stream did not start: {e}", output.name))?;
    Ok(Playing {
        stream: Arc::new(stream),
        shared,
        channels,
        buffer,
    })
}

/// A stream instant in nanoseconds, the unit of every [`Reading`].
pub fn nanos(instant: cpal::StreamInstant) -> u64 {
    u64::try_from(instant.as_nanos()).unwrap_or(u64::MAX)
}
