---
title: Provenance
description: How a score earns its place in the law, and why most scores are refused.
sidebar:
  order: 3
---

Music enters si-jam-sessions only with evidence. Every score comes with a **receipt**, a JSON record of where
the composition, the edition and the files came from. The **licence predicate**, in `crates/provenance`, reads
the receipt and the files and admits the score or refuses it with a reason. The law ingests nothing else.

The predicate is version 3, in law version 5. Its rules year is 2026.

## What a score must show

1. **A composition in the public domain in both the United States and the European Union.**
   - **US:** first published in 1930 or earlier. Works published in 1930 entered the US public domain on
     1 January 2026.
   - **EU, for a named author:** every author died in 1955 or earlier (70 years after death, to the end of the
     year).
   - **EU, for an anonymous work:** first published in 1955 or earlier. An unknown author is recorded as
     anonymous, never as a guess.
2. **An arrangement the project may use.** There are three kinds:
   - one in the public domain;
   - one under an admitted licence, with the typesetter named and the host page's licence quoted from
     evidence;
   - an engraving by this project, which must state exactly `CC0 1.0`.
3. **The source edition:** its publisher, its year and the evidence for them. The year must not be before first
   publication, and the edition must be out of any scholarly-edition term.
4. **An in-file licence that agrees with the receipt.** Every file is read again, and every licence statement in
   it must equal the receipt's record of it.
   - In MIDI, that means copyright and text events, and the less obvious places: sequencer-specific and unknown
     meta events, SysEx and escapes.
   - In LilyPond, it means the header's `copyright` field.

The checks run in a fixed order, and the first failure is the refusal. Every file listed in the receipt must be
supplied once, with its size and SHA-256 equal to the receipt's, and nothing else may be supplied.

## What is refused

- **Unknown or missing evidence.** A claim without evidence is not a claim.
- **Share-alike, non-commercial and AI-restricted licences.** Wording that restricts use by AI is named first,
  so a mixed notice is refused for that reason.
- **A statement that negates itself.** A notice such as "No known copyright restrictions" contains a negating
  word, and it is refused until it is curated.
- **Anything published after 1930,** whatever its composer. *God Bless America* was the first wish for the
  exemplar. It was published in 1938, and Irving Berlin died in 1989, so it stays protected in the US until 2034
  and in the EU until 2060. The law refuses it.

CC BY 4.0 typesettings of public-domain compositions sit in a tier of their own. Every row built from them
carries the credit, and the tiers never mix.

## Receipts are hashed

A receipt has a canonical binary encoding, and its SHA-256 is the **receipt digest**. The law's snapshot
carries the digest, so changing one word of a receipt's notes changes the golden hash. That is deliberate. In
review round 9, a receipt said the project had changed none of a model's answer, but the project had removed
the Markdown code fence around it. The note was corrected to say exactly what was removed, and both the digest
and the golden moved with it.

## The Battle Hymn's evidence

*Battle Hymn of the Republic* is recorded as anonymous:

- The tune was in print by 1859, and the 1859 printings name no composer.
- William Steffe is the most-cited claimant, but the Library of Congress's authority record says no claim can
  be sustained.
- Julia Ward Howe's words were published in 1862.

The project transcribed the 1862 Ditson edition from its scans. The two arrangements follow only that edition,
because anything published after 1930 may still be protected. Each arrangement's receipt carries 21 pieces of
evidence, the call that produced it (model, settings, and the SHA-256 of the prompt), and the CC0 1.0
dedication.
