VERDICT: revise
SENTENCE: Keep the integers, the byte discipline and the licence predicate; the lock lacks a clock and a take, so the law becomes a checked notation editor and the one signal the sibling proved learnable, timing against a gate, falls into the picture.

Q1
ANSWER: An integer-tick score is a notation law, not a jam law. Two things a player feels belong in the hash, both as integers:
- **A transport.** The law advances one quantum per step whether or not anything is proposed. It admits a proposal only past a commit horizon and refuses a late one with its lateness in quanta. ReaLJam's agent cannot change chords once they are committed. StreamMUSE ties feasibility to round-trip latency, and over 60% of its cloud configurations failed it.
- **A take.** Onsets are integer samples at one rate, with integer pitch and velocity; each note cites a score note id or is marked as an addition. Swing is then an integer difference.

Leave these in the host and rows cannot teach timing judgement, the sibling's only locked result (17/17 held-out, 24/24 near-gate probe, two seeds). Score-only rows teach chord labels, which models already do near ceiling from MIDI text. The KB recipe the lock cites still steps its law every quantum.
CHANGE IN THE LOCK: item 2 → "A score is integer ticks at one PPQ; a take is integer sample onsets at 48 kHz with integer pitch and velocity, each citing a score note id or marked as an addition. An integer tempo map (µs per quarter) sends ticks to samples by the floor of the exact rational. The law steps one 128-sample quantum at a time whether or not anything is proposed, and admits proposals only past the commit horizon."
CONFIDENCE: high on the take; medium on 128 samples
BASIS: arxiv.org/html/2502.21267 §2.1; arxiv.org/html/2606.11886 §IV-D, §V-C; arxiv.org/abs/2510.22455; sibling experiments/coverage-v1-sft/RESULTS-r48.md; rust-knowledge sim-architecture recipe

Q2
ANSWER: The shelf is not ten:
- **OpenScore** is CC0: about 1,500 Lieder and 200 quartets.
- **PDMX** has a conflict-free subset of 222,856 files, but 12.29% of its files disagree with their own page's licence. The in-file licence must match or the file is refused.

Engraving our own is the fallback. The predicate misses the edition layer: Germany protects scholarly editions for 25 years, and OpenScore names no source editions.

**Smallest preregistered set** (my binomial arithmetic): a 13/16 bar has only about 40% power at a true 75% win rate, while 80% power at 70% needs 37 paired items (bar ≥ 24). The sibling showed it: adapters that missed at 12/16 won 29/36 on a wider cohort. So I'd require ≥ 37 held-out items from ≥ 10 works by ≥ 5 composers, split by deduplicated work cluster; Lakh has ≥ 38,134 near-duplicates.

**CC-BY:** yes, in its own tier, publishable beside MIT/Apache. Creative Commons says training attribution can be a link to each source; the model card carries those links and the credit ledger. Share-alike stays refused, since shared models built on it should carry the same licence.
CHANGE IN THE LOCK: item 4, add "The source edition, publisher and year are on the record, and an edition under 25 years old is refused. The in-file licence must equal the host page's. CC-BY-4.0 enters a separate tier whose rows carry a credit-ledger id."
CONFIDENCE: high on the arithmetic; medium on the legal readings, which are not legal advice
BASIS: fourscoreandmore.org/openscore; zenodo.org/doi/10.5281/zenodo.15571083; gesetze-im-internet.de/urhg/__70.html; arxiv.org/abs/2509.16662; creativecommons.org/using-cc-licensed-works-for-ai-training-2; sibling docs/finetune-arc-v2-b1-eval-report.md

Q3
ANSWER: Every class gets a hand-authored predicate or a refusal; no model gates.
• **ingest** — predicate: the licence predicate; integer parse; a canonical re-encode that round-trips; the file's own title events match the catalogue, else quarantine.
• **placing notes** — predicate: instrument range; no overlap within a voice; onset past the commit horizon; each note cites a score note or is a declared addition.
• **chord label** — predicate over the law's pitch-class analysis. Equivalent spellings (C6 = Am7/C) are all admitted, and none is trained as the only gold.
• **reharmonization** — predicate: melody bytes unchanged; consonance rule; the signed non-triviality fraction (the sibling locked 1/3); a one-bar copy check against the whole training corpus, not only the source.
• **teaching sentence** — not admitted yet. Admit the typed claims inside it (bar, label, function) and render the sentence from them by template. Free text stays presentation, as the engine's spoken line does.
CHANGE IN THE LOCK: item 3, add "A teaching sentence is rendered from admitted typed claims; free text is never a proposal."
CONFIDENCE: high
BASIS: sibling src/maker/er-gate.ts; arxiv.org/abs/2509.13658; si-rpg-engine docs/PHASE-0.md

Q4
ANSWER: No. An oscillator reading a fixed score is a music box, the engine's frozen seat again. The missing sound is the person's own note landing against the law's beat: a key press admitted on the clock, heard back fast, and graded against the note it answers. Between rhythmic pairs, delays under 11.5 ms sped players up and longer delays slowed them steadily, with no threshold cliff. A click gives context; the take gives something to do.
CHANGE IN THE LOCK: slice → "plays the score and a click from committed beat events, admits one take, and hashes the take's integer onsets and per-note verdicts; the waveform is not hashed."
CONFIDENCE: high on the premise; medium on the delay figure (rhythmic pairs, not keyboards)
BASIS: ccrma.stanford.edu/~cc/shtml/ensDelay.shtml; si-rpg-engine docs/PHASE-0.md

Q5
ANSWER: Discipline, if the clock field lands in fixture one; a refusal if it waits. The field is **onset_sample** (u64, absolute, at the law's rate) plus a stable **note_id** on every committed note-on: derived through the tempo map for score notes, admitted directly for takes. A singer consumes (note_id, onset_sample, duration, pitch) and is gated at 1,920 samples, which is 40 ms at 48 kHz. A tick alone fails, because the tempo map decides when a syllable sounds. I could not confirm the brief's diagnosis: the sibling's v2.3.0 README describes a score clock with a 40 ms gate.
CHANGE IN THE LOCK: item 6, add "Every committed note-on carries note_id and onset_sample; sockets consume only these."
CONFIDENCE: medium
BASIS: sibling README.md line 563

Q6
ANSWER: The assumption is that the value and the training signal live in score edits. What I would write instead is one slice:
- **Input:** one re-fetched public-domain score with its receipt, and one constructed take of it (three notes late by 30, 45 and 60 ms, one early by 45 ms, one wrong pitch), admitted as integer sample onsets.
- **Grading:** the law steps in 128-sample quanta and grades every note against its score note (a two-sided 1,920-sample gate, exact pitch). It writes one row per note, whose target states the comparison in digits.
- **Golden:** one golden hash over score, take and verdicts, native and under wasm.
- **Sound:** two oscillator voices plus a click, so a person hears the late notes. Then a live keyboard take goes through the same admit path.

No model, no MCP.
CHANGE IN THE LOCK: the proposed-slice sentence, as above.
CONFIDENCE: high
BASIS: sibling experiments/coverage-v1-sft/RESULTS-r48.md

SCAR: A played take is not picture. The waveform is picture; the take's integer onsets, pitches and velocities are law, because they are what gets graded, what trains, and what makes it a jam.

DID NOT CHECK: the vocals-paused claim (artifact not found); Chafe 2010's full text (the publisher refused, so I used CCRMA's summary); any crate licence (none recommended); 128-sample buffers in WASAPI shared mode on this rig. Corrected on the way: the sibling registers 54 tools at b015f1d, not 49.
