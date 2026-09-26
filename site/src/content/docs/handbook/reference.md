---
title: Reference
description: Every host command and option, the pieces, the files in the repository, and the notes format.
sidebar:
  order: 5
---

Run any command as `cargo run -p host --release -- <command>`. `host help` prints the same list.

## Commands

| Command | What it does |
|---|---|
| `devices` | List the audio outputs and the MIDI inputs |
| `play [--piece P] [--output X] [--mute] [--voice piano\|osc] [--samples DIR]` | Play the piece. A Battle Hymn arrangement is the score alone. *The Entertainer* is its constructed take against the score and the click. `--mute` renders and counts everything, and sends the device silence |
| `render <out.wav> [--piece P] [--voice piano\|osc] [--samples DIR]` | Render the same mix to a WAV file, with no device |
| `jam [--piece P] [--output X] [--midi X \| --keyboard] [--mute] [--voice piano\|osc] [--samples DIR]` | Play the score and the click, take a live take, and print its verdicts |
| `notes <out.json> [--piece P]` | Write the notes the law commits for the piece as compact JSON |
| `jitter [--output X]` | Measure the audio clock's readings and the console's key path |
| `fetch-piano [--dir DIR] [--archive FILE] [--keep-archive]` | Download the piano's samples (742 MB, CC BY 3.0) into the per-user cache or DIR, check them against the pinned SHA-256, and unpack them |
| `preview <in.mid> <out.wav> [--samples DIR]` | Render a MIDI file straight through the piano, without the law, to audition a draft. It is not the law's committed frames |
| `notices` | Print the licences of the crates and the samples the host uses |

## Options

- **`--piece P`**:
  - `battle-hymn-glm-5.3` (the default) and `battle-hymn-kimi-k3` are *Battle Hymn of the Republic* as each
    model arranged it;
  - `entertainer` is *The Entertainer*, the first milestone's test piece.

  Any other value is refused, and the valid names are listed. A piece name never becomes a path.
- **`--output X`, `--midi X`:** an index from `devices`, or part of a device's name.
- **`--voice`:** the score's voice.
  - The default is the piano, when `fetch-piano` has put verified samples in the cache or in the directory
    `--samples` names.
  - Otherwise it is the oscillator.
  - The take, your live notes and the click are oscillators either way.
- **The click** sounds only against a take: *The Entertainer*'s constructed take in `play` and `render`, and
  your live take in `jam`.
- **The credit:** the piano prints its credit whenever it plays.

## Exit status

| Status | Meaning |
|---|---|
| 0 | The command succeeded |
| 1 | The command line was wrong: an unknown command, a missing or extra argument, an option the command does not take, or a value it does not allow. A hint that `host help` lists the commands follows the message |
| 2 | The command line was right, and the command failed while it ran: a device, a file or the network |

Every error is printed on standard error, starting `host: `.

## Files

| Path | What it holds |
|---|---|
| `scores/<piece>/` | The score: the `.ly` (for the exemplars), the `.mid` the law reads, and `receipt.json` |
| `golden/` | Each piece's golden: `entertainer.golden` and `.rows`; `battle-hymn-*.golden` and `.frames` |
| `crates/law` | The law, built to WebAssembly |
| `crates/provenance` | The receipt, its canonical encoding and the licence predicate |
| `crates/ingest`, `crates/score-model` | Reading Standard MIDI Files into the law's score |
| `crates/golden` | The constructed take and `write-golden` |
| `crates/host` | The `host` binary: audio, MIDI, the piano and every command above |
| `research/battle-hymn/` | The Battle Hymn's evidence and its 1862 transcription |
| `tools/review/` | The external-review tools |
| `tools/site/sections.py` | Where each exemplar's sections begin, for the landing page |
| `docs/PHASE-0.md`, `docs/HANDOFF.md` | The design of record, and the current handoff |

## The notes format

`host notes` writes one JSON object:

```json
{
  "format": "si-jam-sessions notes 1",
  "piece": "battle-hymn-glm-5.3",
  "title": "Battle Hymn of the Republic, arranged by glm-5.3",
  "law_version": 5,
  "sample_rate": 48000,
  "golden": "1c0789b1…",
  "end": 14451739,
  "note_fields": ["onset", "length", "pitch", "velocity", "track"],
  "beat_fields": ["onset", "bar", "beat"],
  "notes": [[0, 37894, 34, 90, 2], …],
  "beats": [[0, 0, 0], …]
}
```

- Onsets and lengths are in samples at 48 kHz.
- Pitch and velocity are MIDI numbers.
- Track 1 is the upper staff and track 2 the lower.
- The file is read through the law's C ABI and never from the MIDI file.
- `golden` is the SHA-256 of the law's snapshot at ingest.
