//! The score in law form: law ticks, the tempo map, and each note's sample
//! position.

use alloc::vec::Vec;

use score_model::IngestedScore;

use crate::MAX_SAMPLE;
use crate::refusal::{Event, Refusal};
use crate::take::ScoreNoteId;
use crate::time::{TempoMap, rescale_tick};

/// A tempo change in law form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawTempo {
    /// Law tick of the change.
    pub tick: u64,
    /// Microseconds per quarter note from `tick` on.
    pub us_per_quarter: u32,
    /// The change's sample position, from the tempo map.
    pub start_sample: u64,
}

/// A time-signature change in law form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawMeter {
    /// Law tick of the change.
    pub tick: u64,
    pub numerator: u8,
    /// The denominator as a power of two, as SMF stores it.
    pub denominator_pow2: u8,
}

/// A score note in law form. Its [`ScoreNoteId`] is its index in
/// [`LawScore::notes`], which keeps score-model's canonical order
/// `(start_tick, pitch, track, channel, end_tick)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LawNote {
    /// Law tick of the note-on.
    pub start_tick: u64,
    pub pitch: u8,
    pub track: u16,
    pub channel: u8,
    /// Law tick of the note-off, after `start_tick`.
    pub end_tick: u64,
    /// 1..=127.
    pub velocity: u8,
    /// `start_tick` through the tempo map. This is the `onset_sample` a socket
    /// consumes.
    pub onset_sample: u64,
    /// `end_tick`'s sample position minus `onset_sample`. Both positions are
    /// exact floors, so this can be 0 for a note shorter than one sample.
    pub duration_samples: u64,
}

/// A score the law holds: every tick in law ticks at PPQ 3360, and every note
/// placed on the sample clock through the tempo map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LawScore {
    tempo: Vec<LawTempo>,
    meter: Vec<LawMeter>,
    notes: Vec<LawNote>,
    map: TempoMap,
}

/// Keeps the earliest failing tick; a tie at one tick goes to the first event
/// in [`Event`]'s order.
fn earliest(first: &mut Option<(u64, Event, Refusal)>, tick: u64, event: Event, refusal: Refusal) {
    let earlier = match first {
        None => true,
        Some((t, e, _)) => (tick, event) < (*t, *e),
    };
    if earlier {
        *first = Some((tick, event, refusal));
    }
}

fn in_range(tick: u64, sample: u64) -> Result<u64, Refusal> {
    if sample > MAX_SAMPLE {
        Err(Refusal::SampleOutOfRange { tick, sample })
    } else {
        Ok(sample)
    }
}

fn reserved<T>(len: usize) -> Result<Vec<T>, Refusal> {
    let mut v = Vec::new();
    v.try_reserve_exact(len).map_err(|_| Refusal::OutOfMemory)?;
    Ok(v)
}

impl LawScore {
    /// Takes an ingested score into law form.
    ///
    /// 1. `validate()` runs first, and any `ModelError` is refused as
    ///    [`Refusal::Model`]: the law does not trust ingest.
    /// 2. Every tick of every event (tempo and meter changes, note starts and
    ///    ends) rescales to `tick × 3360 / source_ppq`. If any is not whole,
    ///    the refusal names the earliest such source tick of any event (see
    ///    [`rescale_tick`]).
    /// 3. The tempo map places each note: `onset_sample` from its start tick,
    ///    `duration_samples` from its end tick. Every sample position must be
    ///    at most [`MAX_SAMPLE`].
    ///
    /// Rescaling multiplies every tick by the same positive rational, so the
    /// canonical order of the notes, and therefore every [`ScoreNoteId`], is
    /// the ingested order.
    pub fn from_ingested(score: &IngestedScore) -> Result<Self, Refusal> {
        score.validate().map_err(Refusal::Model)?;
        let count = score.notes.len();
        if u32::try_from(count).is_err() {
            return Err(Refusal::TooManyNotes { count });
        }
        let ppq = score.source_ppq;
        let mut first: Option<(u64, Event, Refusal)> = None;

        let mut tempo_changes = reserved(score.tempo.len())?;
        for (i, t) in score.tempo.iter().enumerate() {
            match rescale_tick(t.tick, ppq, Event::Tempo(i)) {
                Ok(tick) => tempo_changes.push((tick, t.us_per_quarter)),
                Err(r) => earliest(&mut first, t.tick, Event::Tempo(i), r),
            }
        }
        let mut meter = reserved(score.meter.len())?;
        for (i, m) in score.meter.iter().enumerate() {
            match rescale_tick(m.tick, ppq, Event::Meter(i)) {
                Ok(tick) => meter.push(LawMeter {
                    tick,
                    numerator: m.numerator,
                    denominator_pow2: m.denominator_pow2,
                }),
                Err(r) => earliest(&mut first, m.tick, Event::Meter(i), r),
            }
        }
        let mut spans = reserved(count)?;
        for (i, n) in score.notes.iter().enumerate() {
            let start = rescale_tick(n.start_tick, ppq, Event::NoteStart(i));
            let end = rescale_tick(n.end_tick, ppq, Event::NoteEnd(i));
            match (start, end) {
                (Ok(start), Ok(end)) => spans.push((start, end)),
                (start, end) => {
                    if let Err(r) = start {
                        earliest(&mut first, n.start_tick, Event::NoteStart(i), r);
                    }
                    if let Err(r) = end {
                        earliest(&mut first, n.end_tick, Event::NoteEnd(i), r);
                    }
                }
            }
        }
        if let Some((_, _, refusal)) = first {
            return Err(refusal);
        }

        let map = TempoMap::new(&tempo_changes)?;
        let mut tempo = reserved(tempo_changes.len())?;
        for (tick, us_per_quarter, start_sample) in map.changes() {
            tempo.push(LawTempo {
                tick,
                us_per_quarter,
                start_sample: in_range(tick, start_sample)?,
            });
        }
        let mut notes = reserved(count)?;
        for (n, &(start_tick, end_tick)) in score.notes.iter().zip(spans.iter()) {
            let onset_sample = in_range(start_tick, map.sample_at(start_tick)?)?;
            let end_sample = in_range(end_tick, map.sample_at(end_tick)?)?;
            let duration_samples = end_sample
                .checked_sub(onset_sample)
                .ok_or(Refusal::Overflow)?;
            notes.push(LawNote {
                start_tick,
                pitch: n.pitch,
                track: n.track,
                channel: n.channel,
                end_tick,
                velocity: n.velocity,
                onset_sample,
                duration_samples,
            });
        }
        Ok(LawScore {
            tempo,
            meter,
            notes,
            map,
        })
    }

    /// The notes, in canonical order: a note's index is its [`ScoreNoteId`].
    pub fn notes(&self) -> &[LawNote] {
        &self.notes
    }

    /// The note with this id, if the score has one.
    pub fn note(&self, id: ScoreNoteId) -> Option<&LawNote> {
        usize::try_from(id.0).ok().and_then(|i| self.notes.get(i))
    }

    /// The tempo changes, in tick order.
    pub fn tempo(&self) -> &[LawTempo] {
        &self.tempo
    }

    /// The meter changes, in tick order.
    pub fn meter(&self) -> &[LawMeter] {
        &self.meter
    }

    /// The tempo map, for any law tick's sample position.
    pub fn tempo_map(&self) -> &TempoMap {
        &self.map
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use score_model::{IngestedNote, MeterChange, ModelError, TempoChange};

    pub(crate) fn note(start_tick: u64, pitch: u8, end_tick: u64) -> IngestedNote {
        IngestedNote {
            start_tick,
            pitch,
            track: 1,
            channel: 0,
            end_tick,
            velocity: 80,
        }
    }

    /// Two quarters at 120 then a change to 90 BPM, PPQ 384 in the source.
    pub(crate) fn ingested() -> IngestedScore {
        IngestedScore {
            source_ppq: 384,
            tempo: vec![
                TempoChange {
                    tick: 0,
                    us_per_quarter: 500_000,
                },
                TempoChange {
                    tick: 768,
                    us_per_quarter: 666_667,
                },
            ],
            meter: vec![MeterChange {
                tick: 0,
                numerator: 2,
                denominator_pow2: 2,
            }],
            notes: vec![
                note(0, 60, 192),
                note(0, 64, 384),
                note(384, 62, 768),
                note(768, 65, 1_152),
            ],
        }
    }

    #[test]
    fn notes_keep_their_order_and_land_on_the_sample_clock() {
        let s = LawScore::from_ingested(&ingested()).unwrap();
        // 384 source ticks = 3360 law ticks = one quarter = 24,000 samples at
        // 120 BPM.
        let got: vec::Vec<_> = s
            .notes()
            .iter()
            .map(|n| (n.start_tick, n.pitch, n.onset_sample, n.duration_samples))
            .collect();
        // The 90 BPM quarter: 3360 × 666,667 × 48,000 / 3.36e9 = 32,000.016,
        // floored to 32,000.
        assert_eq!(
            got,
            [
                (0, 60, 0, 12_000),
                (0, 64, 0, 24_000),
                (3_360, 62, 24_000, 24_000),
                (6_720, 65, 48_000, 32_000),
            ]
        );
        assert_eq!(s.note(ScoreNoteId(2)).unwrap().pitch, 62);
        assert_eq!(s.note(ScoreNoteId(4)), None);
        assert_eq!(
            s.tempo(),
            [
                LawTempo {
                    tick: 0,
                    us_per_quarter: 500_000,
                    start_sample: 0
                },
                LawTempo {
                    tick: 6_720,
                    us_per_quarter: 666_667,
                    start_sample: 48_000
                },
            ]
        );
        assert_eq!(
            s.meter(),
            [LawMeter {
                tick: 0,
                numerator: 2,
                denominator_pow2: 2
            }]
        );
    }

    #[test]
    fn every_model_error_is_a_refusal() {
        let mut s = ingested();
        s.source_ppq = 0;
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::Model(ModelError::ZeroPpq))
        );
        let mut s = ingested();
        s.notes[1].velocity = 0;
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::Model(ModelError::VelocityOutOfRange { index: 1 }))
        );
        let mut s = ingested();
        s.notes.swap(0, 3);
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::Model(ModelError::NotesNotSorted { index: 1 }))
        );
    }

    #[test]
    fn a_non_divisible_tick_is_refused_by_name() {
        // At source PPQ 384 a tick is whole in law ticks only when it is a
        // multiple of 4 (3360 / 384 = 35 / 4).
        let mut s = ingested();
        s.notes[3].end_tick = 1_153;
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::InexactTick {
                event: Event::NoteEnd(3),
                tick: 1_153,
                source_ppq: 384
            })
        );
    }

    #[test]
    fn the_refusal_names_the_earliest_inexact_tick_of_any_event() {
        let mut s = ingested();
        s.notes[3].end_tick = 1_153; // a note end at 1153
        s.tempo[1].tick = 769; // a tempo change at 769
        s.meter.push(MeterChange {
            tick: 770,
            numerator: 3,
            denominator_pow2: 2,
        });
        s.notes[2].start_tick = 385; // a note start at 385, the earliest
        assert_eq!(s.validate(), Ok(()));
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::InexactTick {
                event: Event::NoteStart(2),
                tick: 385,
                source_ppq: 384
            })
        );
        // With that one fixed, the tempo change at 769 is the earliest left.
        s.notes[2].start_tick = 384;
        assert_eq!(
            LawScore::from_ingested(&s),
            Err(Refusal::InexactTick {
                event: Event::Tempo(1),
                tick: 769,
                source_ppq: 384
            })
        );
    }

    #[test]
    fn a_position_past_the_law_range_is_refused() {
        // At source PPQ 3360 and the slowest tempo, one tick is ~239.7
        // samples, so a note starting past MAX_SAMPLE / 239 is out of range
        // while its tick still fits a u64.
        let far = 38_600_000_000_000_000u64;
        let s = IngestedScore {
            source_ppq: 3_360,
            tempo: vec![TempoChange {
                tick: 0,
                us_per_quarter: score_model::MAX_US_PER_QUARTER,
            }],
            meter: vec![MeterChange {
                tick: 0,
                numerator: 4,
                denominator_pow2: 2,
            }],
            notes: vec![note(0, 60, 1), note(far, 61, far + 1)],
        };
        let refusal = LawScore::from_ingested(&s).unwrap_err();
        assert!(
            matches!(refusal, Refusal::SampleOutOfRange { tick, sample } if tick == far && sample > MAX_SAMPLE),
            "{refusal:?}"
        );
    }
}
