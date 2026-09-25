# si-jam-sessions

**A music engine where an AI's playing can be graded exactly, replayed exactly, and trained on with a clean conscience.**

Play a phrase against the beat, and the engine records every note of the take: when it landed, in integer samples, and which note of the score it answers. It then grades each note against the score. Two machines grading the same take agree to the last bit, because the grading law is deterministic Rust compiled to one pinned WebAssembly binary. Because everything is logged, any take, lesson or jam replays from its log without calling a model.

`si-jam-sessions` is the counterpart to [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions) and a sibling of [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). A model proposes; a checker that is not the model admits or refuses with a reason; the law commits and hashes. The model never touches the score directly.

## What makes it different

- **The take is part of the law.** Timing, pitch and velocity are integers in the hashed state, so "late by 45 ms" is a fact the engine can prove. The waveform stays presentation.
- **The clock never waits.** The law steps a fixed quantum whether anyone plays or not. A model plans phrases ahead of a commit horizon, and a late plan is refused, never allowed to stall the music.
- **Every song earns its place.** Music enters only with evidence:
  - a public-domain composition in both the US and the EU;
  - a public-domain or self-engraved arrangement;
  - the source edition and its year;
  - an in-file licence that matches its host page.

  Unknown, share-alike, non-commercial and AI-restricted sources are refused. CC-BY-4.0 material sits in its own attributed tier.
- **Training data is a printout of what the law committed.** Every row replays to the same hash. Splits are by work, fixed before any row exists, and every claimed result states its statistical power.

## Where it stands

The design is locked in [`docs/PHASE-0.md`](docs/PHASE-0.md). The first build is a golden-hash harness that grades one receipted public-domain score against one constructed take. It must print the same hash natively on x86_64 and ARM64, and as WebAssembly under V8, SpiderMonkey and JavaScriptCore.

## License

Code: MIT (see [`LICENSE`](LICENSE)). Datasets carry their own licences on their own cards.
