//! The notes the law commits for a piece, as compact JSON for a page to draw.
//!
//! [`read`] drives the law's C ABI as the host does to play the piece, with no
//! take: the score is ingested, the snapshot's hash read, and the law stepped
//! until its committed horizon reaches the quantum holding the last sample a
//! render plays ([`Piece::stop`]); then the frames of that whole window are
//! read in one call. No MIDI file is read here: every onset, length, pitch and
//! velocity is the law's frame, and a note's track is the track of the score
//! note the frame names, as the law's score holds it.
//!
//! [`Notes::json`] writes them in a fixed order with no whitespace, so the
//! same law and score give the same bytes:
//!
//! ```text
//! {"format":"si-jam-sessions notes 1","piece":<id>,"title":<title>,
//!  "law_version":5,"sample_rate":48000,"golden":<the snapshot's SHA-256>,
//!  "end":<the sample after the last note's end>,
//!  "note_fields":["onset","length","pitch","velocity","track"],
//!  "beat_fields":["onset","bar","beat"],
//!  "notes":[[...],...],"beats":[[...],...]}
//! ```
//!
//! Onsets and lengths are samples at 48 kHz, from sample 0. `track` is the
//! SMF track the note came from (LilyPond writes one per staff), `bar` counts
//! from 0 at tick 0, and `beat` from 0 within its bar. The notes come in the
//! law's frame order (by onset, then pitch), and the beats are every beat of
//! the meter up to and including `end`.

use std::fmt::Write as _;

use law::{QUANTUM_SAMPLES, Voice};

use crate::bridge::{Law, Refused};
use crate::score::Piece;

/// One note as the export writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    pub onset: u64,
    pub length: u64,
    pub pitch: u8,
    pub velocity: u8,
    pub track: u16,
}

/// One beat as the export writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beat {
    pub onset: u64,
    pub bar: u64,
    pub beat: u8,
}

/// What [`read`] took from the law.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notes {
    pub id: &'static str,
    pub title: &'static str,
    /// The SHA-256 of the law's snapshot of the ingested score: for an
    /// exemplar, its golden hash.
    pub golden: [u8; 32],
    /// The sample after the last note's end.
    pub end: u64,
    pub notes: Vec<Note>,
    pub beats: Vec<Beat>,
}

/// The score's committed notes and beats for `piece`, read through the law's
/// C ABI with no take.
pub fn read(law: &mut Law, piece: &Piece) -> Result<Notes, Refused> {
    law.ingest(&piece.container)?;
    let golden = law.record()?.hash;
    let last = piece.stop().saturating_sub(1) / u64::from(QUANTUM_SAMPLES);
    while law.horizon().is_none_or(|h| h < last) {
        law.step()?;
    }
    let frames = law.frames(0, last)?;
    let missing = |what: &str| Refused {
        verb: "the notes export",
        code: 0,
        reason: format!("a committed frame {what}"),
    };
    let mut notes = Vec::with_capacity(frames.notes.len());
    for f in &frames.notes {
        if f.voice != Voice::Score {
            return Err(missing("is not the score's"));
        }
        let id = f.note.ok_or_else(|| missing("names no score note"))?;
        let track = piece
            .score
            .note(id)
            .ok_or_else(|| missing("names a note the score does not hold"))?
            .track;
        notes.push(Note {
            onset: f.onset_sample,
            length: f.duration_samples,
            pitch: f.pitch,
            velocity: f.velocity,
            track,
        });
    }
    let end = notes
        .iter()
        .map(|n| n.onset.saturating_add(n.length))
        .max()
        .unwrap_or(0);
    let beats = frames
        .beats
        .iter()
        .filter(|b| b.onset_sample <= end)
        .map(|b| Beat {
            onset: b.onset_sample,
            bar: b.bar,
            beat: b.beat,
        })
        .collect();
    Ok(Notes {
        id: piece.name.id(),
        title: piece.name.title(),
        golden,
        end,
        notes,
        beats,
    })
}

impl Notes {
    /// The export's text: compact JSON, in a fixed order (see the module
    /// documentation).
    pub fn json(&self) -> String {
        let mut t = String::with_capacity(64 + self.notes.len() * 28 + self.beats.len() * 20);
        // Writing to a String cannot fail. The id and the title are plain
        // ASCII with no quote or backslash, so they need no escaping.
        let _ = write!(
            t,
            "{{\"format\":\"si-jam-sessions notes 1\",\"piece\":\"{}\",\"title\":\"{}\",\
             \"law_version\":{},\"sample_rate\":{},\"golden\":\"{}\",\"end\":{},\
             \"note_fields\":[\"onset\",\"length\",\"pitch\",\"velocity\",\"track\"],\
             \"beat_fields\":[\"onset\",\"bar\",\"beat\"],\"notes\":[",
            self.id,
            self.title,
            law::LAW_VERSION,
            law::SAMPLE_RATE,
            golden::run::hex(&self.golden),
            self.end
        );
        for (i, n) in self.notes.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(
                t,
                "{comma}[{},{},{},{},{}]",
                n.onset, n.length, n.pitch, n.velocity, n.track
            );
        }
        t.push_str("],\"beats\":[");
        for (i, b) in self.beats.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            let _ = write!(t, "{comma}[{},{},{}]", b.onset, b.bar, b.beat);
        }
        t.push_str("]}\n");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::{Name, root};

    /// Each exemplar's export is its golden: every committed note in the
    /// frame order, with the frame's onset, length, pitch and velocity and the
    /// law's track for it; the beats of the golden's window through the end;
    /// and the golden hash.
    #[test]
    fn an_exemplars_notes_are_its_goldens_frames() {
        let root = root();
        for name in [Name::BattleHymnGlm53, Name::BattleHymnKimiK3] {
            let piece = Piece::load(&root, name).unwrap();
            let mut law = Law::acquire();
            let notes = read(&mut law, &piece).unwrap();
            let golden = golden::exemplar::compute_at(&root, name.exemplar().unwrap()).unwrap();
            assert_eq!(notes.golden, golden.golden, "{}", name.id());
            assert_eq!(notes.end, golden.score_end);
            assert_eq!(notes.notes.len(), golden.frames.notes.len());
            for (n, f) in notes.notes.iter().zip(&golden.frames.notes) {
                let s = piece.score.note(f.note.unwrap()).unwrap();
                assert_eq!(
                    (n.onset, n.length, n.pitch, n.velocity, n.track),
                    (
                        f.onset_sample,
                        f.duration_samples,
                        f.pitch,
                        f.velocity,
                        s.track
                    )
                );
            }
            let beats: Vec<Beat> = golden
                .frames
                .beats
                .iter()
                .filter(|b| b.onset_sample <= golden.score_end)
                .map(|b| Beat {
                    onset: b.onset_sample,
                    bar: b.bar,
                    beat: b.beat,
                })
                .collect();
            assert_eq!(notes.beats, beats);
            assert_eq!(notes.beats.first().map(|b| b.onset), Some(0));
            // A piano staff: LilyPond's MIDI writes the upper staff as track 1
            // and the lower as track 2.
            let mut tracks: Vec<u16> = notes.notes.iter().map(|n| n.track).collect();
            tracks.sort_unstable();
            tracks.dedup();
            assert_eq!(tracks, [1, 2], "{}", name.id());
        }
    }

    /// The same law and score write the same bytes, and the text is the
    /// layout the module documentation gives.
    #[test]
    fn the_export_is_deterministic_compact_json() {
        let piece = Piece::load(&root(), Name::BattleHymnKimiK3).unwrap();
        let mut law = Law::acquire();
        let a = read(&mut law, &piece).unwrap().json();
        let b = read(&mut law, &piece).unwrap().json();
        assert_eq!(a, b);
        assert!(a.starts_with(
            "{\"format\":\"si-jam-sessions notes 1\",\"piece\":\"battle-hymn-kimi-k3\",\
             \"title\":\"Battle Hymn of the Republic, arranged by kimi-k3\",\"law_version\":5,\
             \"sample_rate\":48000,\"golden\":\""
        ));
        assert!(a.ends_with("]]}\n"));
        let notes = read(&mut law, &piece).unwrap();
        // Spaces only inside the format and the title; one "],[" between
        // neighbours in each array.
        assert_eq!(a.matches(' ').count(), 2 + notes.title.matches(' ').count());
        assert_eq!(
            a.matches("],[").count() + 2,
            notes.notes.len() + notes.beats.len()
        );
        assert_eq!(notes.notes.len(), 1_924);
    }

    /// The Entertainer exports its score alone: its constructed take is not
    /// the score, and no take is admitted.
    #[test]
    fn the_entertainer_exports_its_score_without_the_take() {
        let piece = Piece::load(&root(), Name::Entertainer).unwrap();
        let mut law = Law::acquire();
        let notes = read(&mut law, &piece).unwrap();
        assert_eq!(notes.notes.len(), 2_621);
        assert_eq!(notes.id, "entertainer");
    }
}
