# ai-jam-sessions survey (main @ b015f1d) — read-only inventory, 2026-09-25

The Claude research seat wrote this; the advisor checked the key claims afterwards. The licence counts, the 54 tools, the 12/16 → 29/36 result, the 32/32 heuristic and the published-licence liability were all re-counted from the files. Paths are relative to `E:/AI/ai-jam-sessions/`.

## 1. Product surface

**MCP: 54 tools, 4 prompts.** Counted as 54 `registerTool(` sites (src/mcp-server.ts:485–4476), matching `tool_count: 54` in src/dataset/tool-schemas.json. Prompts: annotate_song, practice_plan, performance_review, maker_loop.

| Group | Count | Tools |
|---|---|---|
| Learn | 10 | list_songs, song_info, registry_stats, list_measures, teaching_note, suggest_song, practice_setup, compare_songs, annotation_progress, server_info |
| Play | 10 | play_song, stop_playback, pause_playback, set_speed, playback_status, view_piano_roll, score_performance, mute_hand, detect_chord, preview_teaching_cues |
| Practice | 4 | practice_loop, practice_status, score_last_take, view_scored_piano_roll |
| Sing / maker | 5 | sing_along, ai_jam_sessions, verify_harmony, auto_reharmonize, compose_panel |
| Guitar | 6 | view_guitar_tab, list_guitar_voices, list_guitar_tunings, tune_guitar, get_guitar_config, reset_guitar |
| Build | 14 | add_song, import_midi, annotate_song, save_practice_note, read_practice_journal, list_keyboards, tune_keyboard, get_keyboard_config, reset_keyboard, score_annotation, validate_song_entry, transpose_song, list_sections, add_section |
| Listen | 5 | analyze_audio, transcribe_audio, score_audio_take, view_spectrogram, ensemble_now |

**Engines.**
- Oscillator piano: additive, with hammer noise.
- Sample piano: user-supplied Salamander samples.
- Vocal: looped, pitch-shifted "aah" carrier.
- Vocal tract: Pink Trombone, monophonic.
- Synth: vocal-synth-engine, Kokoro presets.
- Guitar: additive.
- A layered engine fans events out to several engines and never analyses the mix.

**Scoring** (src/score-performance.ts):
- Each note is matched greedily within `toleranceMs`, default **150 ms** (:369). A wrong pitch counts as missed.
- Overall score = 0.4·pitch + 0.4·completeness + 0.2·timing (:476).
- The green band is min(tol, max(50 ms, 2.5% of a beat)).
- The **40 ms** house gate (src/audio/onsets.ts:36) is used only by score_audio_take (src/mcp-server.ts:3011).

**Practice loop** (src/practice-loop.ts): starts at 70% speed and adds 5% per clean pass, up to 100%. A pass is clean when completeness is at least 95 and nothing is missed.

**Cockpit** (apps/cockpit): Vite and vanilla TypeScript, using Web Audio, Web MIDI and localStorage. Features: piano roll, recording, undo, a blind A/B panel ranked Bradley–Terry, and `window.__cockpit`.

## 2. Song library

**SongConfig** (src/songs/config/schema.ts:120–138):
- id, title, genre (12 values), composer?, arranger?, difficulty, key
- tempo?, timeSignature?, tags, source?
- musicalLanguage?, measureOverrides?, splitPoint?
- status (raw | annotated | ready), provenance?

**The provenance block** (:57–116):
- source_url, source_site, arrangement_creator
- arrangement_license: CC-BY-SA-3.0-DE, Public-Domain, all-rights-reserved, no-redistribution or unknown
- terms_url, terms_quote, verified_at, verifier
- midi_sha256, midi_title_events, midi_credit_events, credited_parties
- title_verdict (matches, no-title-in-file or contradicts), duplicate_of?, quarantine?

`scripts/provenance-audit.ts` writes the block and `src/songs/provenance.test.ts` re-derives it.

**Publishability.**
- npm ships a .mid only if its licence is CC-BY-SA-3.0-DE or Public-Domain.
- Datasets additionally require title_verdict ≠ contradicts, and exclude clair-de-lune, satie and debussy-arabesque.
- That leaves 11 songs, locked by src/dataset/acoustic-v1/allowlist.ts.

**Counts.**
- 108 library songs, all ready.
- By licence: 84 unknown, 8 all-rights-reserved, 2 no-redistribution, 4 CC-BY-SA, 10 Public-Domain.
- 80 came from bitmidi.
- 12 quarantined; 11 publishable; 14 MIDI files ship.
- Fetch on demand: prints the source's terms, downloads only with `--accept-source-terms`, and writes the file only if the SHA-256 matches.

**Annotations.** musicalLanguage blocks are written by an LLM or a human from a deterministic brief, must score at least 80, and are fact-checked. Who wrote each one is not recorded.

## 3. Datasets

All four share one record shape: id, schema_version, provenance, scope, observation, annotation_target, target_trace, eval_metadata, split.

| Dataset | Schema version | Records | Split | Published | What is distinctive |
|---|---|---|---|---|---|
| jam-actions-v0-public 0.5.1 | jam-actions-v0/1.0.0 | 115 (working set 145) | 103/12, clair-de-lune held out | CC-BY-SA-3.0-DE; HF; Zenodo concept 20279918 | Observation carries MIDI notes, REMI, ABC and an SVG. Records come from hand-written specs; tool calls are replayed against the live server. |
| jam-actions-acoustic-v0 1.0.2 | — | 108 | 72/36 by phrase | HF only, no DOI | 3 phrases, 4 right-hand notes each, 9 perturbation kinds, sine tones. |
| jam-actions-v1 1.1.0 | — (adds `family`) | 213 | 154/59 by song | HF; Zenodo 22679457, filed under v0's concept | Observation carries thresholds, gold and the measurements. Gold is the engine's verdict on YIN/SuperFlux measurements. The final turn shows the arithmetic. |
| jam-actions-v1-probe 1.0.0 | — | 24 | eval only, near-gate | Zenodo 22699571 | — |

**The experiment contract.** experiments/_template/README.md and `ExperimentTask` (src/dataset/experiment/task.ts:14) set seven rules:
1. Construct the gold.
2. Check labels against what the tools measure.
3. Split by the unit that leaks.
4. Report per-class results, trivial baselines and the base model.
5. Put thresholds in the record.
6. Make guard bands wider than the estimator's error.
7. A new corpus gets a new schema version.

v1 adds four more: gold must vary; the answer must not appear in the prompt; include a no-tools baseline; take the licence from evidence.

## 4. Training arcs

- **finetune-arc.** Qwen2.5-7B LoRA ×5 on 78 traces. 0.661 → 0.601; neighbouring skills eroded.
- **v1.** 494 examples reached 0.863, with 12/16 wins against a ≥13 bar, so no claim.
- **v2-B1.** The same frozen adapters scored 0.890 and 29/36 on a 36-record cohort. Published as jam-ft-v1-qwen25.
- **b2.** The tool gain held (0.877). Abstention stayed at 0.000, and the prewritten PASS sentence overstated the result.
- **jam-actions-v0-lora.** Stopped with no adapter.
- **acoustic-sft.** The 3B scored 36/36, against 35/36 for a fairly prompted base.
- **coverage-v1-sft.** The form of the target decided what was learned:
  - a bare label taught a class prior;
  - a worded comparison taught the model to read the sign;
  - arithmetic digits taught the comparison itself;
  - with 11 verified songs the 3B needed four takes per song.
- **maker-arc.** E-R: base 9%, Claude 86%, analysis-trained adapters 0/22, and base + deterministic verifier + best-of-n about 91%.
- **analysis-arc.** Deterministic root accuracy 50.0% vs 24.4%.
- **rollout-arc.**
  - P0–P1e were $0 no-gos; P1f passed only because the sample size doubled.
  - P2 aborted: go/no-go was measured at 4-bit while training ran in bf16.
  - P4: GRPO + LoRA on Qwen3-4B-Instruct-2507.
    - A random reward moved the prior more than prefix-forcing did.
    - The memorisation claim was withdrawn because the pools were mislabelled.
    - Held-out gain +4.71pp [+0.46, +11.03]; all 4 sealed predictions failed.
    - β=0 was unresolved, and LoRA init was never pinned.
    - The prior was degenerate: about 1.84 distinct openings out of 16.
    - Coverage: base 93%, instruct 39%, falling to 37%/33% after training.
    - The base with one worked example reached 91% for $0; comparing that to RL was retracted.
    - **A 50-line nearest-tone heuristic scores 32/32 on the same pool (p4/RESULTS.md:114).**

## 5. Lessons a rewrite should design out

| Class | Lesson | Source |
|---|---|---|
| Licence | A hard-coded default stamped "Krueger CC-BY-SA" on ten configs. | library-provenance-audit.md:89 |
| Licence | 12/120 files were the wrong piece but marked ready; five scored 82–94 on the rubric. | — |
| Licence | Licences were checked on the web page, not the bytes (v0 ATTRIBUTION.md:41). Published v0-public (58/115) and acoustic-v0 (36/108) still declare cc-by-sa-3.0 for songs the audit found unknown. The audit left `datasets/**` untouched (audit :158). jam-ft-v1-qwen25 was trained on them. | — |
| Licence | Adapters inherit their base's licence: Qwen2.5-3B is non-commercial. | — |
| Licence | The allowlist capped the RL environment at 11 songs. | — |
| Reproducibility | A fresh clone has 14 MIDI files, so a paid run was void. | 3350fca |
| Determinism | wav_sha256 depends on V8's Math.pow/sin. | — |
| Determinism | A shared RNG, and `--seed` did not pin LoRA init. | — |
| Publishing | File ordering and line endings moved checksums, and a gate missed 6/115 paths. | — |
| Publishing | A card edited on HF broke its checksum. | — |
| Publishing | A DOI was filed under the wrong concept. | — |
| Publishing | npm shipped all 120 MIDI before 2.6.0. | — |
| Publishing | package.json still says 53 tools. | — |
| Dataset | 95/305 records had constant gold. | — |
| Dataset | The answer sat in the observation: 18/59 were solvable without tools. | — |
| Dataset | The target format decides what is learned. | — |
| Dataset | Placeholders shipped: composition_year=1 on 237 records, and leakage_check "pending" on all 115 v0 records. | — |
| Dataset | Every acoustic-v1 take perturbs only the first note (f5-acoustic.ts:293). | — |
| Dataset | Label verification covered 6/108. | — |
| Dataset | Hand-written prose described a prelude that is not in the file. | — |
| Evaluation | One format line was worth 0.639, so report the fair base. | — |
| Evaluation | A grader regex reversed a result, so keep raw outputs. | — |
| Evaluation | Held-out status was checked against the wrong file, and a genre-ordered slice confounded a result. | — |
| Evaluation | One statistic hid a degenerate distribution. | — |
| Evaluation | Learnability was measured at 4-bit while training ran in bf16. | — |
| Evaluation | Metrics stayed healthy while half the intervention never ran (083467e). | — |
| Serving | Ollama's template renders tools differently; 4-bit took the 7B from 17/17 to 4/17. | — |
| Serving | Training traces carry JSON tool results; the live tool returns markdown (mcp-server.ts:2964–3047). | — |
| API | list_measures returns the whole song by default, and titles do not match ids. | — |
| Real-time audio | JACK writes to stdout and corrupts JSON-RPC; only one AudioContext is allowed; leaked handles force an exit. | — |
| CI | A CRLF shebang fails only on Windows; the cockpit tsconfig lacks Node types. | — |

## 6. Language-bound vs portable

**Tied to Node/V8/node-web-audio-api:**
- a single AudioContext;
- ScriptProcessorNode taps;
- process.exit on handle leaks;
- a POSIX stdout supervisor;
- OfflineAudioContext;
- V8 transcendentals in hashed audio;
- pink-trombone-mod, the git-pinned vocal-synth-engine and JZZ;
- the Python sidecars, Ollama and the browser cockpit.

**Portable:**
- tool-schemas.json, SongConfig + ProvenanceSchema, and the note grammar;
- the dataset formats, schema_versions, splits.json and checksum format;
- ExperimentTask, the registry and the env contract;
- the scoring and practice constants, and score-clock.v1.json;
- the dependency-free DSP in src/audio, which needs pinned math and a new schema_version.

## 7. Numbers

| Item | Figure |
|---|---|
| Tools / prompts | 54 / 4 |
| Library JSON | 108 |
| MIDI shipped / withheld | 14 / 94 |
| Quarantined | 12 |
| Publishable | 11 |
| Records | v0-public 115 (working set 145), acoustic-v0 108, v1 213, probe 24 |
| Tests | 3,389 passing / 1 skipped, recorded for v2.5.0; 189 test files in the tree now |
| Package | 2.6.1 |
| HF adapters | jam-ft-v1-qwen25, jam-actions-v1-qwen25-7b, -qwen25-3b, -qwen3-4b, -qwen25-1.5b |
