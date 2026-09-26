# Arrange "Battle Hymn of the Republic" for solo grand piano

Write a complete arrangement for solo grand piano, the whole song through, as one LilyPond 2.24 file. It will be
this project's exemplar: rendered on a sampled Yamaha C5 grand and published, so it must sound rich, warm and
expressive, and it must compile without errors or warnings.

## The source you must follow

The file below is a verified transcription of the only source you may use: the anonymous 1862 Oliver Ditson
edition (public domain). Tune X:1 is the introduction (bars 1–3), verse 1 (bars 4–12) and the chorus (bars
13–20); tune X:2 is the music for verses 2–5. Keep its key (B-flat major), its melody (pitches and rhythms,
including the dotted "Glo-ry, glo-ry, hal-le-lu-jah" figures and the upbeats) and its harmony. The chord symbols
in voice CH are a reading of the printed notes. Do not use any later version of the song: no "familiar"
harmonisations, countermelodies or modulations from 20th-century arrangements.

## The shape

1. **Introduction**: the edition's three bars, voiced for full piano.
2. **Five passes of verse and chorus**, one for each of Julia Ward Howe's five stanzas, each with its own
   accompaniment, building from quiet to full:
   - verse 1, *p*, simple and chordal, close to the edition;
   - verse 2, *mp*, flowing broken chords in the left hand;
   - verse 3, *mf*, the march: the edition's repeated-note drum figure, crisp;
   - verse 4, *mp*, a hushed chorale in four parts, the melody in the tenor register for the verse;
   - verse 5, *f* to *ff*, the melody in octaves over full chords and a moving bass.
   Each chorus rises above its verse.
3. **Coda**: a final "Glory, hallelujah" cadence, broadened, ending on a full B-flat chord, *ff* then let ring.

You may vary the texture and voicing freely, but never the melody's notes or the harmony's roots.

## LilyPond rules (the file is compiled and played by machine)

- `\version "2.24.0"`, one `\score` with a `\new PianoStaff << \new Staff = "upper" { … } \new Staff = "lower"
  { … } >>`, and both a `\layout { }` and a `\midi { }` block.
- `\tempo 4 = 76` at the start; the coda may broaden with explicit `\tempo` changes (the MIDI has no rit.).
- Dynamics and hairpins on the upper staff only, with a `\new Dynamics` context if you like; they drive the
  MIDI velocities.
- **No sustain pedal marks.** Write sustained sound out as note lengths and ties, so the notes themselves carry
  the resonance.
- Every bar complete, one time signature (`\time 4/4`), `\partial` for the upbeat, no repeats (write each pass
  out), no grace notes, no tuplets, no `\ottava`, and no text that could fail to parse.
- Range A0 to C8. At most ten notes sounding at once, and hands a pianist could play.
- `\header` with `title = "Battle Hymn of the Republic"`, `subtitle = "for solo piano"`,
  `poet = "Words: Julia Ward Howe (1862)"`, `composer = "Tune: anonymous (in print by 1859)"`,
  `arranger = "Arrangement: si-jam-sessions"`, `copyright = "CC0 1.0"`, and `tagline = ##f`.

Output only the LilyPond file.
