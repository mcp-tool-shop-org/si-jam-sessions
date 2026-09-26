---
title: The law
description: Integer time, the commit horizon, grading to the sample, and the golden hash that proves it.
sidebar:
  order: 2
---

The law is the part of si-jam-sessions that decides what happened. It is a Rust crate, `crates/law`, built as
one `wasm32-unknown-unknown` module with a raw C ABI. The native tests and the host use the same code as an
ordinary library.

It holds a score, its tempo map and a played take, all as integers. It ingests a score only through the
[licence predicate](/si-jam-sessions/handbook/provenance/), and it steps time forward one quantum at a time
whether or not anything is proposed. It admits a take note or refuses it with a reason, grades every take note
against the score note it cites, and hashes a canonical snapshot of everything it holds.

## No floating point

Everything the law hashes is an integer:

- `tests/no_float.rs` fails if a floating-point type appears in the crate, and Clippy's `float_arithmetic`
  lint is denied.
- Arithmetic is `checked_*`. An overflow becomes a refusal rather than a wrapped number.
- The release profile keeps overflow checks on, and a panic aborts. In WebAssembly an abort is a trap, never
  an unwind into the host.

This is why two machines agree to the last bit. No transcendental function, rounding mode or compiler choice
about fused multiply-add can reach a hash.

## The pins

These constants are part of the law version. Changing any of them changes the snapshot header, so it moves
every hash.

| Pin | Value | Meaning |
|---|---|---|
| `LAW_VERSION` | 5 | The law's version, written into every snapshot |
| `PPQ` | 3,360 ticks per quarter note | Musical time. Every common subdivision, triplets included, is a whole number of ticks |
| `QUANTUM_SAMPLES` | 48 samples | One step of the law at 48 kHz: 1 ms |
| `HORIZON_QUANTA` | 100 quanta | The commit horizon: 100 ms. A plan must arrive this far ahead of the music |
| `GATE_SAMPLES` | 1,920 samples | The grading gate: ±40 ms, both edges inclusive |

## The clock never waits

The law steps a fixed quantum whether anyone plays or not. A model that plans phrases does so ahead of the
commit horizon. A plan that arrives after the horizon has passed is refused. It is never allowed to stall the
music, because a stalled clock would make the record depend on how fast the model answered.

## Grading

Each take note cites the score note it answers. `delta` is the take note's onset minus the score note's onset,
in samples.

| Verdict | When |
|---|---|
| **Match** | The exact pitch, with `delta` from −1,920 to 1,920 samples |
| **Early** | The exact pitch, with `delta` < −1,920 |
| **Late** | The exact pitch, with `delta` > 1,920 |
| **Wrong pitch** | Any other pitch, whatever its timing. The row still states the timing, so a wrong note that is also late reads as both |
| **Addition** | A take note that cites no score note |
| **Never played** | A score note that no take note cites |

Every verdict states its timing in digits, so "late by 45 ms" is a fact the engine can prove. The audio is only
presentation.

## Committed frames

The law exports what it has committed as frames: every score note-on and every beat, in order. The export is
read-only.

- A score note closes 180 ms after its onset.
- A row, once shown, never changes.

A host plays what the law committed and never feeds anything back into it. `host notes` writes the committed
notes of a piece as JSON: onset and length in samples, pitch, velocity and track, with the beats. The landing
page draws its comparison from these files.

## The snapshot and the golden hash

The snapshot is a canonical, little-endian encoding of everything the law holds. Its header carries the law
version and the licence predicate's version. Its SHA-256 is the golden hash.

**The Entertainer's golden** is a constructed take of Scott Joplin's rag, graded and hashed. CI regenerates it
and checks that it is the same in five places:

- natively on x86_64;
- natively on ARM64;
- as WebAssembly under V8;
- under SpiderMonkey;
- under JavaScriptCore.

Each engine is pinned by the SHA-256 of every file it runs. Today's value is
`66b59807261ff93086eed6b0c17cb4a7673e40937a83e0bd32db7ae25ef02946`.

**Each Battle Hymn exemplar has a golden of its own.** It holds the committed frames of the whole render
window, the fewest steps that commit that window, the frame bytes a host receives, and the snapshot's hash at
ingest. The law is run through both its C ABI and its Rust API, and the two must agree to the byte.

`write-golden --check` regenerates every golden and compares it with the committed files. CI also changes one
frame on purpose, and requires the check to fail and name the line.

## Versions are frozen when they reach main

A law version may be refined on its branch. One number never names two different goldens, and every pushed
refinement is recorded. Once a version reaches `main` it is frozen, and any further change is a new version.
Byte-patch tests prove that each version step changes only the version words in the snapshot.
