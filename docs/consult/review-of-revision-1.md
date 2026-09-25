# Advisor review of candidate draft v1 (x-ai/grok-4.7 via OpenRouter, 2026-09-25)

Reviewer: the Claude advisor seat, which is a different family from the author. This is a review, not a signature; the Director adjudicates.

## Accepted as written
- **One clock in the law.** The law steps in ticks, not 128-sample buffers. The take's onsets are sample data, and the host owns buffers. That is coherent, and my 128-sample proposal was marked medium and unsourced.
- **The live key.** A live key uses the admit path but stays out of the golden, because a golden cannot depend on a live clock. This improves on my Q6.
- **Micro-timing.** Micro-timing is the take-minus-score difference, not a second authored offset. That is the right reason to reject Gemini's offsets field.
- **The compensators table.** It matches studio measurements: a Zenodo owner can delete a record within 30 days, after which it becomes a tombstone; cargo yank does not delete anything; `gh release delete` keeps the tag; unpublishing Pages keeps the repository content.

## Corrections to v1
1. **The standards scores are inflated.** Under the rubric, 2 means "enforced in the implementation" and 3 means "enforced + documented + tests/receipts". No code exists yet, so every standard is at most 1 (prose). The author also scored its own lock, which is the self-rating the house re-rates cross-family. Proposed honest scores, each with the slice that raises it:
   - **PIN_PER_STEP 1.** Becomes 2 when slice 1's workflow pins the toolchain, lock, digest and constants in CI.
   - **ANDON_AUTHORITY 1.** Becomes 2 when the execution gate runs in CI before any publish and a test proves it goes red.
   - **NAMED_COMPENSATORS 1.** Becomes 2 when each publish workflow carries its undo step.
   - **DECOMPOSE_BY_SECRETS 1.** Becomes 2 when the wasm law's imports are provably empty, so no token or uncleared byte can reach it.
   - **UNCERTAINTY_GATED_HUMANS 1.** Becomes 2 when the edition-year refusal has a named human clearance path.
   - **EXTERNAL_VERIFIER 1.** Becomes 2 when the first proposal class is gated by its predicate in code and a cross-family review reads the checker.
2. **The andon wording.** "Any of the three owners can halt a publish" puts discretion with the Mind seat. The halt should be the CI execution gate, which is deterministic. Owners do not vote.
3. **Two factual disputes in v1's section 3, checked against the source bytes:**
   - ReaLJam §2.3.1 does say "a majority of responses return within 100 milliseconds". v1's search missed it. The lock was right to keep the constant out anyway.
   - OpenScore's page counts both collections: c.1,500 songs and c.200 quartets (c.700 movements) in the full set, and about 1,300 songs and about 100 quartets published on MuseScore. v1's claim that the page does not count the quartets is wrong. Keeping counts out of the lock is still right.
4. **Cost.** $1.31 for one call. xAI search ran 40 times and returned partly unrelated pages (TikTok, a furniture store). Next time, run Grok without the web plugin or cap the results.

## Sibling lessons v1 did not have (file-cited at b015f1d; from the inventory that landed after the call)
5. **Mandatory arms before any RL spend.** A 50-line nearest-tone heuristic scored 32/32 on the P4 pool (experiments/rollout-arc/p4/RESULTS.md:114). The base model with one worked example reached 91% for $0. Every pool therefore reports a trivial-heuristic arm and a few-shot base arm before any training is priced.
6. **One declared gate per verdict.** The sibling's `score_performance` defaults to 150 ms, and only `score_audio_take` uses the 40 ms gate (src/score-performance.ts:369; src/mcp-server.ts:3011). The new lock declares every gate once, in the constants file, and no verb may carry a default.
7. **Train/serve parity for tool results.** Training traces carried JSON tool results while the live tool returns markdown (src/mcp-server.ts:2964–3047). A row's tool result must be byte-identical to what the served verb returns.
8. **Vary the perturbation position.** Every acoustic-v1 take perturbs only the first note (src/dataset/acoustic-v1/f5-acoustic.ts:293). Constructed takes must vary which note is wrong, and a test must assert the spread.
9. **No placeholders.** `composition_year` was 1 on 237 records (src/dataset/acoustic-v1/builder.ts:135), and `leakage_check` was "pending" on all 115 v0 records. CI refuses any placeholder value in a row.
10. **Replay covers every record.** Label verification covered 6 of 108 records (docs/findings/v0-label-verification-covers-six-records.md). The replay-to-hash admission covers 100% of rows, not a sample.
11. **Published versions re-audit when provenance changes.** The sibling's published datasets still declare cc-by-sa-3.0, while 58 of 115 jam-actions-v0-public records and 36 of 108 jam-actions-acoustic-v0 records come from songs whose licence the audit found "unknown". The audit never touched `datasets/`. The lock's execution gate must re-run on every published version when a song's provenance changes, and the compensator (withdraw and new version) must fire.
