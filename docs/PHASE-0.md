# Phase 0 — si-jam-sessions

2026-09-25. **Status: signed by the coordinator under the Director's delegation.**

This lock went through the following steps:
- Grok drafted it.
- Claude and Gemini each attacked it, independently and in the same shape.
- Grok revised it twice: once as a stateless call and once in the author seat.
- The coordinator adjudicated the three points where the revisions disagreed.

Every step is in `docs/consult/`. The research behind the Claude consult is in `docs/study-swarm/`:
- `phase-0-consult.dispatch.md`: 43 findings, citation gate `escalate`, 0 fabricated, prism receipt `prism-01m3cakqb0htswb2r60vek2rwj`.
- `verification.md`: what was checked out of band.

Findings are cited below as [F*n*].

## What this is

`si-jam-sessions` is the super-intelligence counterpart to `ai-jam-sessions`. It does not import that repository; the sibling is evidence, not a tree to refactor.

The shape follows `si-rpg-engine`:
- A deterministic law advances a fixed quantum whether or not anyone proposes anything.
- A model may propose a musical action, and a checker that is not the model admits it or refuses it with a reason.
- The law hashes what it commits.
- Replay is the seed plus the admitted-action log; the model is not called during replay.

Heard audio is the picture. A played take is not. Its onsets, pitches, velocities, note citations and verdicts are integers in the law, because they are what gets graded, what a training row can teach, and what makes it a jam.

## Three owners

| Layer | Owner | What it may do |
|---|---|---|
| Law | One Rust crate, edition 2024, one `wasm32-unknown-unknown` cdylib | Hold the score, the tempo map and the take as integers. Step one quantum whether or not a proposal arrives. Admit or refuse. Hash a canonical snapshot. Emit training rows from admitted actions. |
| Mind | A model, then the checker named for that action | Propose note placements, a chord label, a reharmonisation, or typed claims. Never write the score. Never gate itself. |
| Picture | The host | Own the audio device and the callback. Play committed frames. Timestamp human input. Wear samples or a singer later, as sockets. Decide nothing. |

## The locked shape

1. **The law's binary and boundary.**
   - The law is Rust, edition 2024, pinned to rustc 1.98.1 by `rust-toolchain.toml`, compiled to one `wasm32-unknown-unknown` cdylib with a raw C ABI: `extern "C"` exports marked `#[unsafe(no_mangle)]`.
   - Status codes cross the boundary; panics do not.
   - Every export argument and return value is 64 bits or narrower; a 128-bit value crosses as explicit halves. [KB integer-time, verified: a `u128` export compiles without a lint and takes three JS parameters.]
   - The host passes bytes in. The law does not read the disk, own an audio device, or read a clock.
2. **Time and state.**
   - **The score** is integer ticks at **PPQ 3360** (2⁵·3·5·7), so 3-, 5- and 7-tuplets are exact at every level down to a 128th. [Arithmetic; the KB's verified recipe covers 480, 960 and 3840, and a checked recipe for 3360 is queued for its next wave.]
   - **The tempo map** is SMF-style microseconds per quarter.
   - **Tick to sample:** `floor(tick × tempo × 48000 / (PPQ × 10⁶))` in `u128`, carrying the division remainder across tempo segments so cumulative time stays exact. [KB integer-time, verified.]
   - **A take** is integer sample onsets at 48 kHz with integer pitch and velocity; each take note cites a score note id or is marked as an addition. Swing and rubato are the integer difference between a take onset and its tempo-mapped score onset.
   - **The step.** The law steps one quantum of *Q* samples whether or not anything is proposed. *Q* is a fixed integer declared in the law version and chosen in slice 1. It is not the audio device's buffer.
   - **Admission.** A proposal is admissible only for quanta past a commit horizon of *H* quanta, and a late proposal is refused with its lateness in quanta. [F29, F30]
   - **Arithmetic.** All hashed state is integer, with no `f64` in the time path. Arithmetic is `checked_*` with a refusal on `None`, and the release profile sets `overflow-checks = true`. [KB integer-time, verified.]
   - **The snapshot** is little-endian, versioned, sorted by a total key, and free of `HashMap` order.
3. **Verbs and checkers.**
   - Five verbs: ingest, place notes, label a chord, reharmonise a span, refuse with a reason.
   - Every checker is a hand-authored predicate. No model gates; a second model may sit on a calibration panel.
   - A teaching sentence is not a verb. Typed claims (bar, label, function) use the verbs above, and any sentence shown to a person is picture, rendered from admitted claims.
   - Timeouts are a field on each verb.
4. **The licence predicate.**
   - **Composition.** Public domain in the United States and the European Union, with the composer's death year and the first-publication year on the record.
   - **Arrangement.** Either public domain with a fetched terms quote and a sha256, or engraved by this product and therefore under the product licence.
   - **Source edition.** Its publisher and year are on the record. A third-party scholarly edition still inside its edition term is refused, and so is an engraving this product makes from one. [F2, F3]
   - **In-file licence.** The licence read inside the file must equal the host page's, or the file is refused. [F5]
   - **Refusals at ingest:** unknown, all-rights-reserved, no-redistribution, share-alike, non-commercial, no-derivatives, and any source term restricting AI use. [F10, F11]
   - **CC-BY-4.0** enters a separate tier. Its rows carry a credit-ledger id, and the model card links each source. [F10]
5. **Training rows.**
   - The law emits training rows from admitted actions. Each row carries the law version, the score hash and, when present, the take hash.
   - A row is admitted only if the law replays it to the same hash, and every row is replayed, not a sample. [F26]
   - Train and held-out are disjoint work clusters, split after similarity deduplication and committed before any row exists. [F18]
   - The win bar is committed at the same time.
   - A law bump that moves a label bumps the dataset in the same commit.
6. **The host.**
   - It receives committed frames. In slice 1 it plays them through oscillator voices together with a click taken from committed beat events.
   - Samples and a singer are sockets. Every committed note-on carries `note_id` and `onset_sample`, and sockets consume only those fields plus duration and pitch.
   - Sockets are outside the hash, and the singer is not in v1.
   - The waveform is not hashed.

## The clock and the jam

A jam is a transport, not an editor. The law keeps stepping while a person does nothing, exactly as `si-rpg-engine`'s pump does.

A model's latency is measured in hundreds of milliseconds to seconds:
- A real-time jamming system commits a window of upcoming chords that the agent can no longer change, plans four beats ahead, and answers in about 100 ms on one device. [F29]
- Real-time LLM accompaniment is feasible only when the request interval and generation length satisfy an inequality against round-trip latency, and over 60% of cloud configurations fail it. [F30]

So the model sits behind the commit horizon, proposing phrases for quanta not yet committed. A deterministic tempo follower belongs in the law when live following is built. (That rests on score-following work retrieved by a research seat — Raphael 2010; Cont 2010 — which is in `docs/study-swarm/lanes/` but not in the gated dispatch; it is a direction, not a pin.)

Heard delay matters too. In rhythmic pairs, one-way delays under about 11.5 ms sped players up and longer ones slowed them progressively, with no threshold cliff. [F31] A person's own note therefore reaches their ears through the host's shortest path. The live input is admitted on the law's clock and never waits for the model.

**Live MIDI input.** It is timestamped by the host and anchored to the law's sample clock:
- WinMM input has 1 ms resolution, which is 48 samples at 48 kHz.
- The MIDI clock and the audio clock are independent, so the host anchors them and re-anchors for drift. [KB host-audio-and-midi, verified from source.]

## The Rust constraints the law inherits

The studio's [Rust knowledge base](https://github.com/mcp-tool-shop-org/readouts/tree/main/rust-knowledge) binds the law. It has 230 verified recipes for `si-rpg-engine`, plus a four-lane wave written for this lock. Its next wave marks the entries below as consumed pins; from then on, an edit to any of them is raised as a change to this lock before it lands.

| Pin | Source |
|---|---|
| `extern "C"` + `#[unsafe(no_mangle)]`; status codes; a panic is an unreachable trap | wasm-raw-abi lane |
| Integer quanta from a host-owned accumulator; render interpolation stays out of state | sim-architecture lane |
| Snapshot bytes: explicit little-endian, declared field order, sorted, versioned | sim-architecture lane |
| No transcendental functions in anything hashed; toolchain and lockfile bumps gated on goldens | float-determinism lane |
| Digest re-pinned only from x86_64 Linux; `--locked`; path remapping; `clippy -D warnings`; `wasm-opt` only if pinned | ci-reproducible-builds lane |
| `midly` 0.5.3, `default-features = false`, `features = ["alloc", "strict"]`; SMPTE-timed files refused | midi-notation-ingest lane (verified; `strict` path not yet compiler-tested) |
| MusicXML `<divisions>` mapped to PPQ by integer maths with a divisibility test; ABC durations parsed as exact rationals, never through a crate's `f32` | midi-notation-ingest lane (verified) |
| Native host: `cpal` 0.18.2 (Apache-2.0, WASAPI shared mode only; it reports xruns on the input path only), `rtrb` SPSC queue, no allocation on the callback | host-audio-and-midi lane (source-verified; compile-only) |
| cargo-deny allowlist MIT / Apache-2.0 / Unlicense / BSD-1-Clause, plus one scoped exception for build-only `unicode-ident` (Unicode-3.0) | crate-licences lane (verified; measured with `cargo metadata`) |

## What comes across, and what stays in the sibling

Measured on `ai-jam-sessions` at `b015f1d`, and corrected there on 2026-09-25.

**Comes across:**
- the provenance block (source URL, verbatim terms quote, sha256, title verdict, credited parties);
- quarantine when a file's own title contradicts the catalogue;
- the reharmonisation predicate (melody locked, a consonance rule, a signed non-triviality fraction);
- the experiment contract (constructed gold; labels checked against what the tools measure; split by the unit that leaks; per-class results with trivial baselines and the base model; thresholds in the record; guard bands wider than estimator error; a new schema version per corpus);
- the finding that a target stating the comparison in digits is what teaches the comparison;
- an oscillator as the first voice.

**Stays in the sibling:**
- its uncleared songs and their derived data;
- the Salamander piano and the tract singer;
- its 54-tool surface;
- the TypeScript audio stack.

On 2026-09-25 the sibling itself found published records built from uncleared arrangements and withdrew them (`ai-jam-sessions` `docs/findings/published-dataset-licence-audit.md`). That is the reason for the last training rule below.

## Training, so the gates are in the first commit

**The action language is the dataset schema.** An MCP server is a later picture of it.

**Rules that exist because the sibling measured their absence:**
- **Arms before spend.** Every pool reports a trivial-heuristic arm and a few-shot base arm before any training is priced. A 50-line nearest-tone heuristic scored 32/32 on the sibling's RL pool, and the base model with one worked example reached 91% for nothing.
- **Mandatory controls.** Untuned base and instruct; a random-reward arm for RL; a pure-RL arm beside SFT-then-RL; a second model family; pass@k to large k. [F20–F24]
- **Per step, the environment emits:**
  - the seed;
  - the law, generator and template hashes;
  - the admitted or refused action with its reason;
  - token ids;
  - the law-computed score components;
  - the per-group reward variance, with a halt when groups are uniform. [F21]
- **Serving parity.**
  - The chat-template renderer is pinned and shipped with any adapter.
  - Serving parity is a release gate, because the sibling's adapter scored 12/17 through one template and 17/17 through the one it was trained on.
  - A tool result in a row is byte-identical to what the served verb returns.
- **No shortcuts in the data.**
  - Constructed takes vary which note is wrong; the sibling perturbed only the first note.
  - CI refuses any placeholder value.
  - Every pool carries impossible canaries, and the model has a refuse-with-reason verb. [F28]
- **The win bar.**
  - The bar is the exact two-sided sign test at α = 0.05 on non-tied pairs, frozen as a table before any row exists.
  - The preregistration states the cohort size and its power at the claimed effect. Below 80% power the run is a recorded pilot, not a claim.
  - The sibling's 13/16 bar had about 40% power at a true 75% win rate. 80% power at a 70% win rate needs 37 non-tied pairs.
- **Provenance changes reach published data.** A provenance change re-runs the licence gate on every published version and fires the withdraw-and-new-version compensator. A gate that only guards the next build is how the sibling shipped 58 records it had no licence to ship.

**Audio rows,** when they come, come from a separately pinned renderer:
- integer 24-bit PCM, content-addressed over samples as FLAC does;
- labels at integer resolution (onsets in samples, pitch in millicents);
- correctly rounded maths only, with subnormals flushed by explicit comparison, never by a mode flag.

The play-path waveform stays picture. [F36–F43]

## Vocals stay a socket

The socket is discipline, not a refusal of half the product. A singer attaches later by consuming `note_id`, `onset_sample`, duration and pitch, gated at 1,920 samples (40 ms at 48 kHz) against `onset_sample`.

A tick alone is not enough, because the tempo map decides when a syllable sounds. The consult could not confirm the sibling's diagnosis for pausing its singer: its v2.3.0 README describes a score clock with a 40 ms gate. That diagnosis is not a premise here.

## What the first slice is

**The score.** One public-domain score, re-fetched with its receipt and its edition year on the record: Scott Joplin, *The Entertainer* (1902), from the Mutopia Project's public-domain typesetting.

**The take.** One constructed take of it: three notes late by 30, 45 and 60 ms, one early by 45 ms, one wrong pitch, at positions drawn by the seed. It is admitted as integer sample onsets.

**Grading.** The law steps in *Q*-sample quanta and grades every take note against its score note, inside a two-sided 1,920-sample gate at exact pitch. It writes one row per note, whose target states the comparison in digits.

**The golden hash.**
- It covers the score, the tempo map, the take and the verdicts.
- It is checked natively on x86_64 and ARM64, and as wasm under V8, SpiderMonkey and JavaScriptCore.
- Only `write-golden` writes it.

**The sound.** The host plays two oscillator voices and a click from committed beat events, so a person hears the late notes against the beat. A live keyboard take uses the same admit path and is heard back on it. It is not inside the golden, because a golden that depends on a live clock is not a golden.

**Not in slice 1:** a model, an MCP server, a second song, a sample library, or a singer.

## Standards compliance

Scored 0–3 against the studio's six workflow standards. Slice 1 raised two of them to 2; each line still at 1 names the slice that raises it to 2.

| Standard | Score | Evidence now | Raised to 2 by |
|---|---|---|---|
| PIN_PER_STEP | 2 | The snapshot header carries the law version, PPQ, rate, *Q*, *H*, the gate, and the licence predicate's version and cut-off years, so a changed pin moves the golden hash; CI proves it by setting *H* to 101 and watching `write-golden --check` fail. The golden file records every input's SHA-256, the seed and the PRNG. CI pins the toolchain, the lockfile (`--locked`), every action by commit SHA, and node, SpiderMonkey and JavaScriptCore by version and by the SHA-256 of every file they run, checked before they run (`.github/engines/`) | Slice 1 (the golden hash) |
| ANDON_AUTHORITY | 1 | The publish halts are specified | The slice that first publishes: the execution gate runs in CI before any publish, with a test proving it goes red |
| NAMED_COMPENSATORS | 1 | The table below names every irreversible call | Each publish workflow carries its undo step when it is written |
| DECOMPOSE_BY_SECRETS | 2 | The law takes bytes and returns status and hash; `crates/law/tests/wasm_artifact.rs` asserts the wasm module imports nothing, and CI asserts it again with node's parser, so each engine hands the law bytes and reads bytes back; tokens stay in publish steps | Slice 1 (the law core) |
| UNCERTAINTY_GATED_HUMANS | 1 | Refusals do not publish; edition years are confirmed by a person before a retry | The ingest slice: a named human clearance path for scholarly editions inside their term |
| EXTERNAL_VERIFIER | 1 | Checkers are hand-authored predicates, never the proposer; the consult was cross-family | The first proposal class gated in code, reviewed by a different model family |

## Compensators

| Call | Undo | State after undo | Owner |
|---|---|---|---|
| GitHub repository created | Archive the repository; deletion is not an undo | Read-only; history kept | coordinator |
| Hugging Face dataset repository created | `hf repos delete OWNER/NAME --repo-type dataset` while empty; otherwise make it private | Removed or private | dataset publisher |
| Hugging Face dataset push | Revert commit; a correction push with `delete_stale` for removed records | Previous revision current; old revisions reachable by hash unless squashed | dataset publisher |
| Zenodo DOI mint | Owner deletion within 30 days; after that, restrict files, add a public note, publish a new version | Tombstone, or restricted files with the DOI resolving | Zenodo record owner |
| Crate publish | `cargo yank --version V` (does not delete) | New resolves avoid it; lockfiles keep working | crate publisher |
| GitHub release | `gh release delete TAG -y`, tag removed separately | Release gone; downloaded assets not recalled | repository admin |
| Pages deploy | Unpublish from the repository's Pages settings | Site offline; content kept | repository admin |

## Decisions the Director may override

**PPQ.** You might expect 960, the common DAW resolution. The lock chooses 3360, because quintuplets and septuplets are exact at every level down to a 128th and the cost is only larger integers. Override by writing 960 into the law version before slice 1.

**The CC-BY tier.** You might expect the corpus to stay public-domain or self-engraved only. The lock admits CC-BY-4.0 in its own tier with a credit ledger, because Creative Commons treats a source link as training attribution and the tier widens the shelf. Share-alike, non-commercial and no-derivatives stay refused. Override by striking the tier before the first ingest.

**The first score.** You might expect one of our own engravings. The lock starts from a receipted Mutopia public-domain typesetting of *The Entertainer*, because slice 1 tests the law and not the engraving pipeline. Override by naming another score with its receipt.
