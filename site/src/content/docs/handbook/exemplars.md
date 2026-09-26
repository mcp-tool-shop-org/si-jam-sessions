---
title: The exemplars
description: How two models arranged Battle Hymn of the Republic, how the law admitted both, and how to reproduce each recording bit for bit.
sidebar:
  order: 4
---

The exemplars are two arrangements of *Battle Hymn of the Republic* for solo piano. They play side by side on
the [landing page](/si-jam-sessions/#listen). Each step that produced them is recorded, and each is checked
by something other than the step itself.

## The source

The project transcribed the 1862 Ditson edition from its scans. The transcription is
`research/battle-hymn/battle-hymn-ditson-1862.abc` (12,766 bytes). It is the only source the arrangers were
allowed to follow. Two readings in the scans are still open: the octave of the A at bar 19, beat 4, and the
alto's last note in bar 13.

## The arrangers

Each arrangement is the answer to one call on Ollama Cloud, on 2026-09-26, at temperature 0, with the same
system prompt, the same brief and the same transcription. glm-5.3's first call, with thinking set to high,
spent its whole output allowance thinking and returned no score, so its arrangement comes from a second call,
at medium. The brief names the transcription as the only source and forbids
anything taken from a later arrangement.

| | glm-5.3 | kimi-k3 |
|---|---|---|
| Thinking | medium | high |
| What the project changed | Removed the Markdown code fence around the answer | Added `\language "english"`, because the model wrote English note names without declaring them |
| Length | 5:02 | 4:37 |
| Notes the law committed | 1,720 | 1,924 |

No note was changed in either file. A third model, minimax-m3, returned no score in two attempts.

LilyPond 2.24.4 turns each `.ly` into the `.mid` the law reads. The project used the LilyPond project's
mingw-x86_64 build, and rendering again gives the same bytes.

## Admission

Both arrangements are admitted as engravings by this project under licence predicate version 3, and both are
dedicated CC0 1.0. Each `.ly` states `CC0 1.0` in its `copyright` field, and each `.mid` states no licence at
all. Each receipt records the evidence for the tune, the words and the edition, the call that produced the
arrangement, and the change the project made to it.

| Receipt | Digest |
|---|---|
| glm-5.3 | `ee5a82dfc077829c0303daef54c1039d6d7b3149b0f768cdf429ee8d260569d8` |
| kimi-k3 | `d044ffa49ce26ae3268e8a743ba737dd3d05e93fa7a9106afd203d100e9e4793` |

## The recordings

Each recording is rendered through the law, not straight from the MIDI file. The law ingests the score and
commits its notes, and the host plays what the law committed on the Salamander Grand Piano V3. No click sounds
in the recording, because an exemplar is a score with no take.

```bash
cargo run -p host --release --locked -- fetch-piano
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
cargo run -p host --release --locked -- render battle-hymn-kimi-k3.wav --piece battle-hymn-kimi-k3 --voice piano
```

| WAV (48 kHz, stereo, 32-bit float) | Bytes | SHA-256 |
|---|---|---|
| battle-hymn-glm-5.3.wav | 115,998,136 | `85b2d56723cacc6c87895a2784d2cd034dea1142e62de6d8a1a9d2867dad2eed` |
| battle-hymn-kimi-k3.wav | 106,486,216 | `bf49d310de5e94975b277cd1e7c57134388a0232d53bb059f12e7727baeba017` |

- **Clean renders:** no note was late or dropped, and nothing clipped.
- **Reproducible:** a second render gives identical bytes. CI's piano job rendered each exemplar whole on
  Linux, and both SHA-256s equal the release: the same on Windows and Linux.
- **Credited:** each WAV carries the piano's credit in its `LIST/INFO` chunk.

The page plays compressed copies made with ffmpeg 7.1: MP3 at 192 kb/s and Opus at 128 kb/s. They were encoded
with its bit-exact flags, so they too come out the same every time. The uncompressed WAVs are assets of the
[0.1.0 release](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## The comparison

Both arrangements follow one form: an introduction, five verses each with its chorus, and a coda. The landing
page sets them side by side on one time axis, section by section.

**Finding the sections.** `tools/site/sections.py` works out where each section begins:

1. It compiles a copy of each `.ly` with a MIDI program change placed where each section starts.
2. It reads those changes back.
3. It converts their positions to the law's sample time through the tempo map. The tempo map is first
   checked against every beat the law committed.
4. It snaps every start to a note onset the law committed, and stops if there is none within two samples.

That check caught a real case. kimi-k3's coda changes tempo on an eighth-note pickup, and interpolating between
the law's beats would have misplaced the start by about 800 samples.

**The measurements.** Within each section, the page measures each arrangement's:

- length;
- notes a second;
- register, as the median note;
- average velocity;
- most notes sounding at once.

It then states the largest of those five differences in words. The table also shows each section's range,
which is not one of the five. One example: each of glm-5.3's verses runs 28.4 seconds, and
each of kimi-k3's runs 25.7.

## What is not checked

Nobody has yet done a listening test, or compared the arrangements note by note with the 1862 edition. The
brief asked each model to follow the edition, and the receipts record exactly what each model was given, but
neither arrangement has been proven faithful to it.
