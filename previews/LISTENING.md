# Battle Hymn drafts: listening sheet

**These are previews, not the law's committed frames.** Each draft's MIDI, compiled by LilyPond, was rendered straight through the host's piano with `host preview`, without the law: no receipt, no licence predicate, no step, no hash. This branch holds only the compressed previews and this sheet. It is never merged.

Piano samples: Salamander Grand Piano V3 by Alexander Holm, CC BY 3.0, https://creativecommons.org/licenses/by/3.0/

## The drafts, from `exemplar/drafts` at `9aeec06`

| Draft | Compiles | Preview | Duration | Bars | Notes |
|---|---|---|---|---|---|
| glm-5.3 (`drafts/battle-hymn-glm-5.3.ly`, blob `3bc969f0`) | yes, no warnings | `battle-hymn-glm-5.3.preview.mp3` | 5:02.1 | 93, all 4/4 | 1,720 |
| kimi-k3 (`drafts/battle-hymn-kimi-k3.ly`, blob `bc910298`) | no: 1,534 errors | none | none | none | none |

### glm-5.3

B-flat major, 4/4, a quarter note at 76 until the coda, which slows to 66, 60, 52, 46 and 42. It writes no sustain pedal and no note off the keyboard. The preview peaks at -7.6 dBFS and clips nothing.

| Section | Bars | Starts at | As the draft marks it |
|---|---|---|---|
| Introduction | 1-3 | 0:00.0 | the edition's three bars, voiced for full piano |
| Verse 1 | 4-12 | 0:09.5 | p, simple and chordal, close to the edition |
| Chorus 1 | 13-20 | 0:37.9 | mp |
| Verse 2 | 21-29 | 1:03.2 | mp, flowing broken chords in the left hand |
| Chorus 2 | 30-37 | 1:31.6 | mf |
| Verse 3 | 38-46 | 1:56.8 | mf, the march, crisp |
| Chorus 3 | 47-54 | 2:25.3 | f |
| Verse 4 | 55-63 | 2:50.5 | mp, a hushed four-part chorale, melody in the tenor register |
| Chorus 4 | 64-71 | 3:18.9 | mf, the edition's four vocal parts, melody in the soprano |
| Verse 5 | 72-80 | 3:44.2 | f to ff, melody in octaves over full chords and a moving bass |
| Chorus 5 | 81-88 | 4:12.6 | ff |
| Coda | 89-93 | 4:37.9 | a broadened "Glory, hallelujah" cadence, ff, then let ring |

The bar count is the MIDI's: its last note ends at tick 142,848, 93 bars of 1,536 ticks. The section bars are each section's bar checks, and they sum to the same 93.

### kimi-k3

It does not compile under LilyPond 2.24.4: 1,534 errors and no MIDI, so there is no preview. The draft writes English note names (`bf`, `ef`) without `\language "english"`, and LilyPond reads Dutch names (`bes`, `es`) by default. The errors are 609 "not a note name: bf", 238 "not a note name: ef", and syntax errors that follow from them, starting at line 14's `\key bf \major`. It is not repaired here.

As written, it plans a three-bar introduction, five verse and chorus pairs with a pickup before each verse (verses p, mp, mf, mp, f to ff; choruses mp, mf, f, f, ff), and a broadened coda at 70, 60 and 50.

## How they were made

1. LilyPond 2.24.4 (the LilyPond project's mingw-x86_64 build, zip SHA-256 `e238f5a3...`), `lilypond --loglevel=WARNING <draft>.ly`.
2. `host preview <draft>.mid <draft>.wav`, from branch `host/piano` at `5e25698` (PR #11), on the samples `host fetch-piano` verified (archive SHA-256 `b7760e16...`). The glm-5.3 WAV is 14,501,280 frames of 48 kHz stereo 32-bit float, SHA-256 `2fb574a1...`; rendering the same MIDI again gives the same bytes.
3. ffmpeg 7.1 with libmp3lame: MP3 at 192 kb/s, 48 kHz stereo. Its tags carry its title, marked PREVIEW, and the credit. MP3 SHA-256 `02c50d0a...`.
