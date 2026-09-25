# si-jam-sessions phase 0 — study-swarm grounding for the consult reply

Date 2026-09-25. Five retrieval lanes (licences, representation, environment-first training, real-time co-performance, reproducible audio). Synthesised by the advisor seat (Claude). Findings below feed the Claude reply to the phase-0 consult brief; the lock itself was drafted by another family (xAI). Research seats were Claude Opus (retrieval only); verification is family-different.

## Step 1 — the load-bearing questions

1. What music can the product ingest, redistribute and train on with licence evidence, and at what scale?
2. In which forms should a model see and write music, and which evaluations of symbolic music hold up?
3. What must an environment emit so SFT and RL on it do not meet gates later?
4. Where can a slow model sit in a real-time jam, and what quantum and latency budget does the law need?
5. What can a dataset built on rendered and measured audio rest on across platforms?

## Research grounding

### Licences and scale (Q1)

1. **US works published before 1931 are public domain in 2026, and the cut-off moves every January.** Hirtle, Cornell University Library 2026 (https://guides.library.cornell.edu/copyright/publicdomain). A date test clears only the composition layer; the record carries death and publication years.
2. **Germany protects scholarly editions of public-domain works for 25 years from publication.** German Federal Ministry of Justice, UrhG §70 (https://www.gesetze-im-internet.de/urhg/__70.html). The record carries the source edition and its year; a recent Urtext is refused.
3. **A performing edition of public-domain music was held to carry its own copyright in England.** Hyperion Records v Sawkins 2005 ([2005] EWCA Civ 565, https://vlex.co.uk/vid/sawkins-v-hyperion-records-793310109). Honour an encoder's stated licence rather than argue originality.
4. **OpenScore releases about 1,500 Lieder and about 200 string quartets under CC0 and does not list source editions on its page.** OpenScore 2026 (https://fourscoreandmore.org/openscore/). A tier-A shelf at scale exists; the edition field must be recovered per file.
5. **PDMX found 31,221 of its songs (12.29%) carry a different licence inside the file than on the MuseScore page, and recommends the 222,856-song conflict-free subset.** Long et al. 2025 (Zenodo record https://zenodo.org/records/15571083, v9). Read the in-file licence and refuse on mismatch.
6. **PDMX is a public-domain MusicXML dataset built from MuseScore uploads.** Long et al. 2024 (arXiv:2409.10831). A candidate pool, admitted only file by file.
7. **Across more than 1,800 text datasets, licences were omitted by hosting sites over 70% of the time and miscategorised over 50% of the time.** Longpre et al. 2023 (arXiv:2310.16787). A platform's licence tag is a claim, not evidence.
8. **42.6% of the music-generation training sets examined were collected from the web without permission.** Morreale, Sharma and Wei 2023 (ISMIR 2023, Zenodo record https://zenodo.org/records/10265217). Deny inherited corpora by default.
9. **Prominent audio datasets contain significant copyrighted material.** Agnew et al. 2024 (arXiv:2410.13114). Render our own audio rather than import audio sets.
10. **Creative Commons advises that publicly shared models or outputs based on ShareAlike content carry the same licence, and that attribution for model training can be a link to the dataset source.** Creative Commons 2025 (https://creativecommons.org/using-cc-licensed-works-for-ai-training-2/). ShareAlike stays a refusal; CC-BY can enter its own tier with a credit ledger.
11. **The Session's data licence forbids any processing with LLMs.** The Session (github.com/adactio/TheSession-data/blob/main/LICENSE.md). A per-source AI-use clause is a refusal reason even when commercial reuse is allowed.

### Representation and evaluation (Q2)

12. **NotaGen trains on bar-interleaved ABC, and after its preference stage perplexity rose while music-college listeners preferred the outputs.** Wang et al. 2025 (arXiv:2502.18008). Never gate on perplexity; bar-interleaved ABC is a composing form.
13. **Moonbeam uses one compound token per note with absolute onsets because transformers struggle to sum time shifts.** Guo and Dixon 2025 (arXiv:2505.15559). A take is absolute-onset events.
14. **Bar and position tokenisations produce time-syntax errors that time-shift tokenisations cannot make.** Fradet et al. 2023 (arXiv:2310.08497). Corrections target explicit (onset, duration) slots.
15. **The Anticipatory Music Transformer interleaves fixed control events so a model can infill and accompany against them.** Thickstun et al. 2023 (arXiv:2306.08620). Accompaniment = proposals against events the law holds fixed.
16. **Multimodal models perform near ceiling on music perception tasks from MIDI and drop on the same tasks from audio.** Carone et al. 2025 (arXiv:2510.22455). Measuring is the law's job; judging a measured quantity is nearly free for a base model.
17. **On the same score questions, the best ABC-text model outscored the best PDF-image model.** Dai et al. 2025 (arXiv:2511.20697). No piano-roll images in training observations.
18. **Lakh MIDI holds at least 38,134 near-duplicates among 178,561 files, from user re-arrangements and metadata edits.** Choi et al. 2025 (arXiv:2509.16662). Split by similarity-deduplicated work cluster, never by record.
19. **Exact replication in symbolic music can be detected down to one bar.** Ji et al. 2025 (arXiv:2509.13658). A reharmonisation's copy check runs against the training corpus, not only the source.

### Environment-first training (Q3)

20. **RL with verifiable rewards wins at pass@1 while the base model overtakes it at large k.** Yue et al. 2025 (arXiv:2504.13837). Log pass@k to large k for base, instruct and trained checkpoints on one frozen pool.
21. **Groups whose samples all pass or all fail carry zero advantage and zero gradient.** Yu et al. 2025 (arXiv:2503.14476). Emit per-group reward variance and halt a run whose groups are uniform.
22. **Random rewards raised a Qwen2.5-Math model substantially and often failed on Llama and OLMo.** Shao et al. 2025 (arXiv:2506.10947). A random-reward arm and a second family are mandatory controls.
23. **A math-specialised model reproduced a large share of benchmark problems verbatim from a partial prompt, and only correct rewards helped on freshly generated problems.** Wu et al. 2025 (arXiv:2507.10532). Evaluate on items the law generates; probe fixed songs with partial prompts.
24. **RL with an outcome reward generalised to unseen rule variants where SFT memorised.** Chu et al. 2025 (arXiv:2501.17161). Hold out rule variants (unseen works, unseen meters), not only new seeds.
25. **Adaptive-difficulty training across 400 verifiable environments beat continuing the original RL at more than three times the compute.** Zeng et al. 2025 (arXiv:2511.07317). Item id = hash(law, generator, seed, difficulty).
26. **Tool-call data that passed format, execution and semantic checks trained a 7B model past several GPT-4 variants on BFCL.** Liu et al. 2024 (arXiv:2406.18518). Admit a training record only if the law replays it to the same hash.
27. **Live API drift made a tool benchmark unstable until it was replaced by a virtual API server with caching.** Guo et al. 2024 (arXiv:2403.07714). Freeze everything a record depends on.
28. **Given an explicit abort option, a frontier model's rate of "passing" impossible tasks fell sharply.** Zhong, Raghunathan and Carlini 2025 (arXiv:2510.20270). Every pool carries impossible canaries and the model has a refuse-with-reason verb.

### Real-time co-performance (Q4)

29. **In a real-time jamming system, chords inside a commit window can no longer change, the agent plans four beats ahead, and most responses returned within 100 ms.** Scarlatos et al. 2025 (arXiv:2502.21267). Proposals are admitted only beyond a commit horizon; a committed quantum never changes.
30. **Real-time LLM accompaniment is feasible only when the request interval and generation length satisfy an inequality against modelled round-trip latency, and over 60% of cloud configurations violated it.** Zheng et al. 2026 (arXiv:2606.11886). Size the model's planning horizon from measured p95 latency; a late plan is refused.
31. **Between rhythmic pairs, delays under 11.5 ms accelerated tempo and longer delays decelerated it progressively, with no threshold cliff.** Chafe, Cáceres and Gurevich 2010 (doi:10.1068/p6465; summary at https://ccrma.stanford.edu/~cc/shtml/ensDelay.shtml). A person's own note must sound within about 10 ms of the key press.
32. **Minimum-latency piano transcription trades accuracy for delay, and interactive use needs under 30 ms.** Hu et al. 2025 (arXiv:2509.07586). The acoustic tap verifies committed intents after a lag and never gates a quantum.
33. **A real-time music model generates two-second chunks from ten seconds of context.** Lyria Team 2025 (arXiv:2508.04651). The model steers phrases; the law places notes.
34. **Of 184 live music agent systems, few take turns or lead, and latency is the most common technical concern.** Kim et al. 2026 (arXiv:2602.05064). Call-and-response one phrase later is an open slot.
35. **An audio callback must not allocate, perform I/O, switch context or take a mutex.** PortAudio (https://files.portaudio.com/docs/v19-doxydocs/writing_a_callback.html). The callback copies committed frames from a lock-free queue and decides nothing.

### Reproducible audio (Q5)

36. **Re-implementing common MIR metrics transparently changed reported scores by up to about 11% relative, because of conventions such as how an undefined case is scored.** Raffel et al. 2014 (https://archives.ismir.net/ismir2014/paper/000320.pdf). The evaluator is pinned in the law, and its undefined-case rules are written down.
37. **Copies of the same MIR dataset failed checksums, and three hop sizes in circulation for one dataset gave significantly different metrics.** Bittner et al. 2019 (https://archives.ismir.net/ismir2019/paper/000009.pdf). Ship a checksum manifest and a reference loader; store time as integer samples at an integer rate.
38. **MFCC and chroma features computed from lossy copies deviate from the lossless original by several percent, depending on the library.** Urbano et al. 2014 (ISMIR 2014, Zenodo record https://zenodo.org/records/1416276). Labels are measured on lossless PCM only.
39. **Widely used libm implementations are not correctly rounded for common double-precision functions, and one library can differ across CPUs through run-time code selection.** Gladman et al. 2026 (https://members.loria.fr/PZimmermann/papers/accuracy.pdf). Hashed or labelled audio math never calls the host libm.
40. **CORE-MATH publishes MIT-licensed correctly rounded math functions, and Rust's libm already carries a port of one of them.** Inria CORE-MATH (https://core-math.gitlabpages.inria.fr/). Port the renderer's few transcendental functions from correctly rounded sources.
41. **WebAssembly's numerics depart from IEEE 754 only in rounding mode, NaN payloads and non-stop mode, while ARMv7 NEON flushes subnormals to zero.** W3C WebAssembly Core Specification (https://webassembly.github.io/spec/core/exec/numerics.html); Arm DEN0018A (https://developer.arm.com/documentation/den0018/a/NEON-Instruction-Set-Architecture/Flush-to-zero-mode). Flush tiny feedback state by explicit comparison, never by an FP mode flag.
42. **FLAC's checksum is defined over the decoded samples, independent of compression level and container.** van Beurden and Weaver 2024 (RFC 9639, https://www.rfc-editor.org/rfc/rfc9639.html). Content-address sample bytes, not files.
43. **Lossy decoders are held to error tolerances, not bit-exactness, and an MP3 encode-decode round trip adds a fixed sample delay.** Underbit, ISO/IEC 11172-4 summary (https://www.underbit.com/resources/mpeg/audio/compliance); LAME Technical FAQ (https://lame.sourceforge.io/tech-FAQ.txt). Lossy audio is a preview, never a dataset's stored fact.

## Step 5 — where each finding lands in the lock (Claude reply)

- Item 2 (the law): a take as integer sample onsets and a stepping transport with a commit horizon — findings 13, 15, 16, 29, 30, 31, 33.
- Item 3 (verbs and checkers): predicates only; copy check against the corpus; typed claims for teaching — findings 18, 19, 28.
- Item 4 (licence predicate): edition year, in-file licence equality, CC-BY tier, ShareAlike and AI-clause refusals — findings 1–11.
- Item 5 (training rows): replay-to-hash admission, frozen pools, mandatory arms, pass@k — findings 20–27.
- Slice 1: a take plus a click, hearable within about 10 ms — findings 29, 31, 35.
- Audio rows, when they come: the play-path waveform stays picture; a dataset's audio is integer PCM from a separately pinned renderer with its own golden, content-addressed over samples, labels at integer resolution — findings 36–43.
