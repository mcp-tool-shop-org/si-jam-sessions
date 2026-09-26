//! The clocks: a live note's instant as a law sample.
//!
//! Three clocks meet here, and none of them is the law's:
//! - **The stream clock.** cpal's `StreamInstant`; on WASAPI it is
//!   `QueryPerformanceCounter`, in nanoseconds. The host stamps a key press
//!   with it the moment the press arrives (`Stream::now`).
//! - **The audio clock.** The frames the callback renders. Each callback
//!   carries cpal's prediction of the stream instant its first frame is heard
//!   (`OutputCallbackInfo::timestamp().playback`), which is a [`Reading`]: law
//!   sample `sample` is heard at stream instant `nanos`.
//! - **The MIDI clock.** WinMM stamps a message in whole milliseconds since
//!   `midiInStart`, which the host reads as microseconds (KB recipe 1536).
//!
//! The audio and stream clocks are independent crystals, and so are the MIDI
//! and stream clocks, so each pair drifts apart; a single anchor taken at the
//! start goes stale by the drift times the time since (KB recipe 1537). Both
//! mappings here are therefore re-anchored continuously:
//! - [`AudioClock`] fits a line through the most recent readings, one per
//!   callback, and reads the law sample heard at an instant off that line;
//! - [`MidiClock`] bounds the MIDI-to-stream offset from above with every
//!   message (a message cannot arrive before it was stamped) and keeps the
//!   smallest bound of the last ten seconds.
//!
//! A key press takes one step, stream instant to law sample. A MIDI note takes
//! two, MIDI time to stream instant and then the same step, through the same
//! [`AudioClock`]: the two paths share their anchoring code.

/// One reading of the audio clock: law sample `sample` is heard at
/// stream-clock instant `nanos`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub sample: u64,
    pub nanos: u64,
}

/// Readings the fit uses: the last 128 callbacks, about 1.3 s of 10 ms
/// buffers.
const READINGS: usize = 128;

/// Samples per nanosecond at 48 kHz.
const NOMINAL: f64 = 48_000.0 / 1e9;

/// The law sample heard at a stream-clock instant, from the latest readings.
pub struct AudioClock {
    readings: [Reading; READINGS],
    len: usize,
    newest: usize,
}

impl AudioClock {
    pub fn new() -> AudioClock {
        AudioClock {
            readings: [Reading {
                sample: 0,
                nanos: 0,
            }; READINGS],
            len: 0,
            newest: 0,
        }
    }

    /// A callback's reading. The oldest of the last `READINGS` is dropped.
    pub fn push(&mut self, reading: Reading) {
        if self.len > 0 {
            self.newest = (self.newest + 1) % READINGS;
        }
        if let Some(slot) = self.readings.get_mut(self.newest) {
            *slot = reading;
        }
        self.len = (self.len + 1).min(READINGS);
    }

    /// The law sample heard at stream instant `nanos`, to the nearest sample,
    /// or `None` before the first reading. It is negative before sample 0.
    ///
    /// With one reading the rate is 48 kHz exactly. With more, the rate and the
    /// offset are the least-squares line through the readings, measured from
    /// the newest, so the device's own rate against the stream clock is
    /// followed; a fitted rate more than 1% from 48 kHz is taken as bad
    /// readings, and 48 kHz is used with the readings' mean offset.
    pub fn sample_at(&self, nanos: u64) -> Option<i64> {
        let last = *self.readings.get(self.newest)?;
        if self.len == 0 {
            return None;
        }
        let points = self.readings.iter().take(self.len);
        let x = |r: &Reading| (i128::from(r.nanos) - i128::from(last.nanos)) as f64;
        let y = |r: &Reading| (i128::from(r.sample) - i128::from(last.sample)) as f64;
        let n = self.len as f64;
        let (mx, my) = points
            .clone()
            .fold((0.0, 0.0), |(sx, sy), r| (sx + x(r) / n, sy + y(r) / n));
        let (sxx, sxy) = points.fold((0.0, 0.0), |(sxx, sxy), r| {
            let dx = x(r) - mx;
            (sxx + dx * dx, sxy + dx * (y(r) - my))
        });
        let fitted = if sxx > 0.0 { sxy / sxx } else { NOMINAL };
        let slope = if (fitted / NOMINAL - 1.0).abs() <= 0.01 {
            fitted
        } else {
            NOMINAL
        };
        let at = (i128::from(nanos) - i128::from(last.nanos)) as f64;
        let offset = my + slope * (at - mx);
        let sample = last.sample as f64 + offset;
        Some(sample.round() as i64)
    }
}

impl Default for AudioClock {
    fn default() -> Self {
        AudioClock::new()
    }
}

/// How far a device's readings stray from a straight line: the root mean
/// square and the largest distance, in samples, of each reading from the
/// least-squares line through all of them. This is the jitter the audio clock
/// averages away; `None` for fewer than three readings.
pub fn residuals(readings: &[Reading]) -> Option<(f64, f64)> {
    let first = *readings.first()?;
    if readings.len() < 3 {
        return None;
    }
    let n = readings.len() as f64;
    let x = |r: &Reading| (i128::from(r.nanos) - i128::from(first.nanos)) as f64;
    let y = |r: &Reading| (i128::from(r.sample) - i128::from(first.sample)) as f64;
    let (mx, my) = readings
        .iter()
        .fold((0.0, 0.0), |(sx, sy), r| (sx + x(r) / n, sy + y(r) / n));
    let (sxx, sxy) = readings.iter().fold((0.0, 0.0), |(sxx, sxy), r| {
        let dx = x(r) - mx;
        (sxx + dx * dx, sxy + dx * (y(r) - my))
    });
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    let (mut squares, mut worst) = (0.0f64, 0.0f64);
    for r in readings {
        let off = y(r) - (my + slope * (x(r) - mx));
        squares += off * off;
        worst = worst.max(off.abs());
    }
    Some(((squares / n).sqrt(), worst))
}

/// Arrivals the MIDI offset is estimated from.
const ARRIVALS: usize = 64;
/// How far back an arrival still counts: ten seconds.
const MIDI_WINDOW_NS: u64 = 10_000_000_000;

/// A MIDI timestamp's instant on the stream clock.
pub struct MidiClock {
    /// (MIDI microseconds, arrival on the stream clock in nanoseconds)
    arrivals: [(u64, u64); ARRIVALS],
    len: usize,
    newest: usize,
}

impl MidiClock {
    pub fn new() -> MidiClock {
        MidiClock {
            arrivals: [(0, 0); ARRIVALS],
            len: 0,
            newest: 0,
        }
    }

    /// A message stamped `midi_micros` arrived at stream instant
    /// `arrived_nanos`.
    pub fn arrival(&mut self, midi_micros: u64, arrived_nanos: u64) {
        if self.len > 0 {
            self.newest = (self.newest + 1) % ARRIVALS;
        }
        if let Some(slot) = self.arrivals.get_mut(self.newest) {
            *slot = (midi_micros, arrived_nanos);
        }
        self.len = (self.len + 1).min(ARRIVALS);
    }

    /// The stream instant of MIDI time `midi_micros`, or `None` before the
    /// first arrival: the MIDI time plus the smallest arrival-minus-stamp of
    /// the last ten seconds of arrivals. A message cannot arrive before it
    /// happened, so each difference is the offset plus that message's delay,
    /// and the smallest is the best bound.
    pub fn nanos_at(&self, midi_micros: u64) -> Option<u64> {
        let (_, newest) = *self.arrivals.get(self.newest)?;
        if self.len == 0 {
            return None;
        }
        let offset = self
            .arrivals
            .iter()
            .take(self.len)
            .filter(|(_, arrived)| newest.saturating_sub(*arrived) <= MIDI_WINDOW_NS)
            .map(|&(stamp, arrived)| i128::from(arrived) - i128::from(stamp) * 1_000)
            .min()?;
        u64::try_from(i128::from(midi_micros) * 1_000 + offset).ok()
    }
}

impl Default for MidiClock {
    fn default() -> Self {
        MidiClock::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so the jitter is the same every run.
    struct Lcg(u64);

    impl Lcg {
        /// A value in `-1.0..1.0`.
        fn unit(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        }
    }

    const SECOND: u64 = 1_000_000_000;

    /// A device whose clock runs `ppm` parts per million fast against the
    /// stream clock: one callback every 480 of its frames, each reading
    /// jittered by up to `jitter_ns`. Returns the readings, and the true law
    /// sample heard at any stream instant.
    fn device(
        start_ns: u64,
        ppm: f64,
        jitter_ns: f64,
        seconds: u64,
    ) -> (Vec<Reading>, impl Fn(u64) -> f64) {
        let rate = 48_000.0 * (1.0 + ppm * 1e-6);
        let mut lcg = Lcg(7);
        let mut readings = Vec::new();
        let mut sample = 0u64;
        while sample < seconds * 48_000 {
            let true_ns = start_ns as f64 + sample as f64 / rate * 1e9;
            readings.push(Reading {
                sample,
                nanos: (true_ns + jitter_ns * lcg.unit()).round() as u64,
            });
            sample += 480;
        }
        (readings, move |ns: u64| {
            (ns as f64 - start_ns as f64) * rate / 1e9
        })
    }

    /// Readings jittered by up to half a millisecond (24 samples) stray from
    /// their line by about that; exact readings do not stray at all.
    #[test]
    fn residuals_measure_the_jitter_of_the_readings() {
        let (exact, _) = device(SECOND, 150.0, 0.0, 3);
        let (rms, worst) = residuals(&exact).unwrap();
        assert!(worst < 0.01, "{rms} {worst}");
        let (jittered, _) = device(SECOND, 150.0, 500_000.0, 3);
        let (rms, worst) = residuals(&jittered).unwrap();
        // Uniform in ±24 samples: an RMS of 24 / sqrt(3), about 13.9. The
        // line is fitted to the jittered readings, not the true line, so the
        // farthest reading can stray a sample or two past 24.
        assert!((12.0..16.0).contains(&rms), "{rms}");
        assert!((20.0..=27.0).contains(&worst), "{worst}");
        assert_eq!(residuals(&exact[..2]), None);
    }

    #[test]
    fn nothing_is_known_before_the_first_reading() {
        assert_eq!(AudioClock::new().sample_at(5 * SECOND), None);
        assert_eq!(MidiClock::new().nanos_at(1_000), None);
    }

    /// A clock that keeps exact time maps every instant to the sample heard
    /// then, to the nearest sample.
    #[test]
    fn a_steady_clock_maps_exactly() {
        let (readings, truth) = device(3 * SECOND, 0.0, 0.0, 2);
        let mut clock = AudioClock::new();
        for r in &readings {
            clock.push(*r);
        }
        for ns in [
            3 * SECOND,
            3 * SECOND + 20_833,
            4 * SECOND + 1,
            5 * SECOND + 7_000_000,
        ] {
            assert_eq!(clock.sample_at(ns), Some(truth(ns).round() as i64), "{ns}");
        }
        // Before the first reading's sample: negative, before the take starts.
        assert_eq!(clock.sample_at(3 * SECOND - 1_000_000), Some(-48));
    }

    /// The device clock runs 150 ppm fast and every reading jitters by up to
    /// half a millisecond. Re-anchored on every reading, the clock stays within
    /// 12 samples (0.25 ms) of the truth for ten minutes, just behind the
    /// newest reading and 50 ms past it. Held to its first reading instead, it
    /// would be 90 ms out by the end.
    #[test]
    fn the_audio_clock_follows_a_drifting_device() {
        let (readings, truth) = device(SECOND, 150.0, 500_000.0, 600);
        let mut clock = AudioClock::new();
        let mut first = AudioClock::new();
        first.push(readings[0]);
        let mut worst = 0.0f64;
        for (i, r) in readings.iter().enumerate() {
            clock.push(*r);
            if i % 997 == 0 && i > 300 {
                for ahead in [0, 3_000_000, 50_000_000] {
                    let ns = r.nanos + ahead;
                    let error = clock.sample_at(ns).unwrap() as f64 - truth(ns);
                    worst = worst.max(error.abs());
                }
            }
        }
        assert!(worst <= 12.0, "worst error {worst} samples");
        let end = readings.last().unwrap().nanos;
        let stale = first.sample_at(end).unwrap() as f64 - truth(end);
        assert!(
            stale.abs() > 48.0 * 80.0,
            "a first anchor alone is {stale} samples out"
        );
    }

    /// The fitted rate, not only the re-anchoring. A device 300 ppm fast with
    /// exact readings: the line through the last readings is the device's own,
    /// so the clock is right to the sample 50 ms past the newest reading. At
    /// 48 kHz exactly, re-anchored on the same readings' mean, it would be
    /// about 10 samples out there (300 ppm of the 0.7 s from their middle).
    #[test]
    fn the_fitted_rate_follows_a_fast_device_to_the_sample() {
        let (readings, truth) = device(SECOND, 300.0, 0.0, 60);
        let mut clock = AudioClock::new();
        let mut worst = 0.0f64;
        for (i, r) in readings.iter().enumerate() {
            clock.push(*r);
            if i > READINGS && i % 50 == 0 {
                let ns = r.nanos + 50_000_000;
                let error = clock.sample_at(ns).unwrap() as f64 - truth(ns);
                worst = worst.max(error.abs());
            }
        }
        assert!(worst <= 1.0, "worst error {worst} samples");
    }

    /// WinMM stamps a MIDI message in whole milliseconds since midiInStart;
    /// the message reaches the host up to 2 ms later. The earliest arrivals
    /// pin the offset between the two clocks to within a millisecond.
    #[test]
    fn a_midi_time_maps_through_its_earliest_arrival() {
        let offset = 17 * SECOND + 123_456;
        let mut lcg = Lcg(3);
        let mut clock = MidiClock::new();
        let mut t_us = 0.0f64;
        let mut worst = 0.0f64;
        for i in 0..400 {
            t_us += 180_000.0 + 90_000.0 * lcg.unit();
            let stamp = (t_us / 1_000.0).floor() as u64 * 1_000;
            let delay = 50_000.0 + 1_950_000.0 * (lcg.unit() + 1.0) / 2.0;
            let arrived = (offset as f64 + t_us * 1_000.0 + delay) as u64;
            clock.arrival(stamp, arrived);
            if i >= 20 {
                let error =
                    clock.nanos_at(stamp).unwrap() as f64 - (offset as f64 + t_us * 1_000.0);
                worst = worst.max(error.abs());
            }
        }
        assert!(worst <= 1_000_000.0, "worst error {worst} ns");
    }

    /// The MIDI clock runs 50 ppm slow against the stream clock. Its offset is
    /// estimated from recent arrivals only, so it follows the drift: within
    /// 1.5 ms over ten minutes, where the first offset alone ends 30 ms out.
    #[test]
    fn the_midi_clock_follows_its_drift() {
        let mut lcg = Lcg(11);
        let mut clock = MidiClock::new();
        let mut first: Option<i128> = None;
        let mut t_us = 0.0f64;
        let (mut worst, mut stale) = (0.0f64, 0.0f64);
        while t_us < 600e6 {
            t_us += 400_000.0 + 200_000.0 * lcg.unit();
            let stream_ns = 5e9 + t_us * 1_000.0;
            let midi_us = t_us * (1.0 - 50e-6);
            let stamp = (midi_us / 1_000.0).floor() as u64 * 1_000;
            let arrived = (stream_ns + 100_000.0 + 400_000.0 * (lcg.unit() + 1.0) / 2.0) as u64;
            clock.arrival(stamp, arrived);
            let offset = *first.get_or_insert(i128::from(arrived) - i128::from(stamp) * 1_000);
            if t_us > 20e6 {
                let error = clock.nanos_at(stamp).unwrap() as f64 - stream_ns;
                worst = worst.max(error.abs());
                stale = (i128::from(stamp) * 1_000 + offset) as f64 - stream_ns;
            }
        }
        assert!(worst <= 1_500_000.0, "worst error {worst} ns");
        assert!(
            stale.abs() > 25e6,
            "the first offset alone is {stale} ns out"
        );
    }

    /// A MIDI note and a key press take the same last step: the audio clock.
    #[test]
    fn a_midi_note_and_a_key_meet_on_the_audio_clock() {
        let (readings, truth) = device(2 * SECOND, 0.0, 0.0, 3);
        let mut audio = AudioClock::new();
        for r in &readings {
            audio.push(*r);
        }
        let mut midi = MidiClock::new();
        // MIDI time 0 is stream instant 2.5 s; the message arrives 0.3 ms late.
        midi.arrival(1_000, 2_500_000_000 + 1_000_000 + 300_000);
        midi.arrival(9_000, 2_500_000_000 + 9_000_000);
        let key = 2_500_000_000 + 9_000_000;
        let from_midi = audio.sample_at(midi.nanos_at(9_000).unwrap()).unwrap();
        assert_eq!(from_midi, audio.sample_at(key).unwrap());
        assert_eq!(from_midi, truth(key).round() as i64);
    }
}
