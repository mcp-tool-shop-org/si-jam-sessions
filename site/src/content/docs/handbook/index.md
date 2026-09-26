---
title: The si-jam-sessions handbook
description: What si-jam-sessions is, how it decides what happened in a piece of music, and where to start.
sidebar:
  order: 0
---

**si-jam-sessions** is a music engine where an AI's playing can be graded exactly, replayed exactly, and
trained on with a clean conscience.

It rests on one division of labour:

1. **A model proposes:** a phrase to play, an arrangement, a lesson.
2. **A checker that is not the model** admits the proposal or refuses it, and every refusal carries a
   reason.
3. **The law commits and hashes** what was admitted. The law is a deterministic program written in integer
   arithmetic and compiled to one pinned WebAssembly binary.

The model never touches the score directly. What the law committed is the record, and it replays to the same
hash on any machine, without calling a model.

## What the law decides

- **When a note landed:** in integer samples at 48 kHz, never as a floating-point time.
- **Which note of the score it answers,** and whether it was early, late or in time. The gate is ±40 ms,
  inclusive.
- **Whether a score may enter at all.** A score is admitted only with evidence that it is free to use in both
  the United States and the European Union.

Grading a take is therefore a fact, not an opinion. "Late by 45 ms" is something the engine can prove, and two
machines grading the same take agree to the last bit.

## The two exemplars

The project's showpiece is *Battle Hymn of the Republic*, arranged twice for solo piano. Two models, glm-5.3
and kimi-k3, each arranged it from the same source: this project's transcription of the anonymous 1862 Ditson
edition. Both arrangements passed the law's licence check, and the law committed every note of each. The
[landing page](/si-jam-sessions/#listen) plays both recordings and shows, section by section, where the two
arrangements differ. [The exemplars](/si-jam-sessions/handbook/exemplars/) tells the whole story.

## Where to start

- [Getting started](/si-jam-sessions/handbook/getting-started/): build it, fetch the piano, and play.
- [The law](/si-jam-sessions/handbook/the-law/): integer time, the commit horizon, grading, and the golden
  hash.
- [Provenance](/si-jam-sessions/handbook/provenance/): how a score earns its place, and why most are refused.
- [Reference](/si-jam-sessions/handbook/reference/): every command and option.
- [Security](/si-jam-sessions/handbook/security/): what the code touches, and what it never does.

## Status

The first milestone is complete: the law, ingest and provenance, the golden hash, and the host. Version
0.1.0 is the first release. Its assets are the two Battle Hymn renders as uncompressed WAV files. The design
is recorded in `docs/PHASE-0.md`, and `docs/HANDOFF.md` tracks what comes next.
