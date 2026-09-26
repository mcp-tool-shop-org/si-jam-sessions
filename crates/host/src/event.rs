//! What crosses into the audio callback: committed events from the law, and
//! the live monitor from the input.

use law::{Frames, Voice as LawVoice};

/// Which of the host's two note voices plays a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    /// The score: a sine.
    Score,
    /// The take, or a live note heard back: a soft square, so it can be told
    /// from the score by ear.
    Take,
}

/// One committed event, as the scheduler pushes it and the callback plays it.
/// Positions are on the law's sample clock, where sample 0 is score tick 0
/// and the stream's first frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A note-on, with how long it sounds.
    Note {
        onset: u64,
        voice: Voice,
        pitch: u8,
        velocity: u8,
        duration: u64,
    },
    /// A beat of the score's meter; a downbeat is the first beat of a bar.
    Beat { onset: u64, downbeat: bool },
}

impl Event {
    /// The sample the event starts on.
    pub fn onset(&self) -> u64 {
        match *self {
            Event::Note { onset, .. } | Event::Beat { onset, .. } => onset,
        }
    }
}

/// A live note heard back at once, before the law sees it: the monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Monitor {
    On { pitch: u8, velocity: u8 },
    Off { pitch: u8 },
}

/// The events of a frame window in the order the ring carries them: by onset,
/// and at one onset the beat before the notes, the notes in the law's frame
/// order. `out` is cleared first.
///
/// Live notes are left out. The host heard each one back through the monitor
/// the moment it was played, and the law admits it later, into quanta the
/// scheduler has already read.
pub fn from_frames(frames: &Frames, out: &mut Vec<Event>) {
    out.clear();
    let mut beats = frames.beats.iter().peekable();
    for n in &frames.notes {
        while let Some(b) = beats.next_if(|b| b.onset_sample <= n.onset_sample) {
            out.push(Event::Beat {
                onset: b.onset_sample,
                downbeat: b.downbeat,
            });
        }
        let voice = match n.voice {
            LawVoice::Score => Voice::Score,
            LawVoice::Take => Voice::Take,
            LawVoice::Live => continue,
        };
        out.push(Event::Note {
            onset: n.onset_sample,
            voice,
            pitch: n.pitch,
            velocity: n.velocity,
            duration: n.duration_samples,
        });
    }
    for b in beats {
        out.push(Event::Beat {
            onset: b.onset_sample,
            downbeat: b.downbeat,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use law::{Beat, FrameNote, ScoreNoteId};

    fn note(onset: u64, voice: LawVoice, pitch: u8) -> FrameNote {
        FrameNote {
            onset_sample: onset,
            voice,
            note: Some(ScoreNoteId(0)),
            pitch,
            velocity: 64,
            duration_samples: 10,
        }
    }

    fn beat(onset: u64, downbeat: bool) -> Beat {
        Beat {
            onset_sample: onset,
            bar: 0,
            beat: u8::from(!downbeat),
            downbeat,
        }
    }

    #[test]
    fn events_come_by_onset_beats_first_and_live_notes_left_out() {
        let frames = Frames {
            first_quantum: 0,
            last_quantum: 10,
            notes: vec![
                note(0, LawVoice::Score, 60),
                note(0, LawVoice::Take, 61),
                note(100, LawVoice::Score, 62),
                note(100, LawVoice::Live, 63),
                note(300, LawVoice::Take, 64),
            ],
            beats: vec![
                beat(0, true),
                beat(200, false),
                beat(300, false),
                beat(400, true),
            ],
        };
        let mut out = vec![Event::Beat {
            onset: 9,
            downbeat: false,
        }];
        from_frames(&frames, &mut out);
        let summary: Vec<(u64, char)> = out
            .iter()
            .map(|e| match e {
                Event::Beat { onset, .. } => (*onset, 'b'),
                Event::Note {
                    onset,
                    voice: Voice::Score,
                    ..
                } => (*onset, 's'),
                Event::Note { onset, .. } => (*onset, 't'),
            })
            .collect();
        assert_eq!(
            summary,
            [
                (0, 'b'),
                (0, 's'),
                (0, 't'),
                (100, 's'),
                (200, 'b'),
                (300, 'b'),
                (300, 't'),
                (400, 'b'),
            ]
        );
    }
}
