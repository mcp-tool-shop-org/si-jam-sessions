# Lane 1 — publish-friendly music (Opus research seat, 2026-09-25; retrieval claimed by the seat, NOT yet gated)

Not opened by the seat: piano-midi.de licence page; US Copyright Office Compendium / Circular 50; Aria-MIDI and Morreale full texts. No official ruling found on MIDI transcriptions specifically — findings 1–3 apply by analogy.

## Rights layers
1. Hirtle, Cornell University Library, "Copyright Term and the Public Domain in the United States", updated 2026-01-01, https://guides.library.cornell.edu/copyright/publicdomain — US works published before 1931 are PD; sound recordings before 1926 PD, 1926–46 protected 100 years from publication. 17 U.S.C. §103(b) (https://www.law.cornell.edu/uscode/text/17/103) — copyright in a derivative work covers only its new material. Implication: US cut-off in 2026 is publication ≤1930 and moves every January; a date test clears only the composition layer.
2. Germany UrhG §70, https://www.gesetze-im-internet.de/urhg/__70.html; EU Directive 2006/116/EC Art. 5, https://www.legislation.gov.uk/eudr/2006/116/article/5 — Germany protects scholarly editions of PD works (substantially different) for 25 years from publication; EU lets states grant up to 30 years. Implication: store source edition + year; an encoding made from a recent Urtext is not clean in Germany.
3. Hyperion Records v Sawkins [2005] EWCA Civ 565, https://vlex.co.uk/vid/sawkins-v-hyperion-records-793310109 — performing editions of PD Lalande held original works with their own copyright. Contrast Feist v. Rural 499 U.S. 340 (1991), https://www.law.cornell.edu/supremecourt/text/499/340 — US originality needs creativity, not labour. IMSLP policy: Canadian law creates no new copyright for literal copying. Implication: honour the encoder's stated licence everywhere rather than argue originality.

## Corpora (licence read on each corpus's own page)
| Corpus | Licence | Commercial training | Caveat |
|---|---|---|---|
| OpenScore Lieder (~1,500), String Quartets (~200), fourscoreandmore.org/openscore | CC0 | Yes | source editions not listed |
| Mutopia (2,124 pieces), mutopiaproject.org/legal.html | per piece: PD, CC BY, CC BY-SA | Yes | intake: contributors dead 70+ yrs, publication before 1923, named source |
| PDMX, arXiv:2409.10831 | collection CC BY 4.0; files PD Mark or CC0 as uploaded | Yes (paper body) | see 7 |
| IMSLP, imslp.org/wiki/IMSLP:Licensing_Policy_and_Guidelines | scans PD in Canada only; user typesets CC0/CC BY/CC BY-SA; NC deprecated, ND barred | per file | Canadian test only |
| KernScores/Humdrum (bach-370-chorales; essen-folksong-collection) | per file | mostly no | Bach-370 encodings CC BY-NC-SA 4.0; Essen "Copyright 1995, estate of Helmut Schaffrath", licence bars commercial editions |
| MusicNet (330 recordings), doi:10.5281/zenodo.5120004 | CC BY 4.0 | Yes | audio described only as CC or PD, no per-file licence |
| GiantMIDI-Piano (10,855), github.com/bytedance/GiantMIDI-Piano | CC BY 4.0 | yes by its licence | transcribed from YouTube performances |
| MAESTRO (1,276), ASAP (1,067), Aria-MIDI (1,186,253) | CC BY-NC-SA 4.0 | No | |
| GigaMIDI (2.1M+), huggingface.co/datasets/Metacreation/GigaMIDI | CC BY-NC 4.0, gated | No | Canadian fair dealing; includes Lakh |
| Lakh (176,581), colinraffel.com/projects/lmd | CC-BY 4.0 | no in practice | scraped; PDMX body notes documented copyrighted songs |
| The Session, github.com/adactio/TheSession-data/blob/main/LICENSE.md | custom + ODbL | **No** | commercial reuse allowed but any processing with LLMs forbidden |
| Nottingham, abc.sourceforge.net/NMD | none stated | No | rights assigned to a named individual |

## Provenance failures
4. Longpre et al. 2023, The Data Provenance Initiative, arXiv:2310.16787 — 1,800+ text datasets: hosting sites omitted licences 70%+ and miscategorised 50%+ (abs). Implication: a platform tag is a claim, not evidence.
5. Morreale, Sharma & Wei 2023, ISMIR, doi:10.5281/zenodo.10265217 — 42.6% of music-generation training sets collected from the web without permission (abs). Implication: deny inherited corpora by default.
6. Agnew et al. 2024, Sound Check, arXiv:2410.13114 — seven prominent audio datasets contain significant copyrighted material (abs). Implication: render our own audio.
7. Long et al. 2025, PDMX Zenodo v9, doi:10.5281/zenodo.15571083 — 31,221 songs (12.29%) carry a different licence in-file than on the MuseScore page; authors keep the page value, flag conflicts, recommend the 222,856 conflict-free subset. Implication: read the in-file licence and halt on mismatch — the old library's failure again.

## ShareAlike and weights
8. Creative Commons 2025, Using CC-licensed Works for AI Training, https://creativecommons.org/using-cc-licensed-works-for-ai-training-2/ — CC licences apply only where permission is needed; models/outputs often not derivative works; still advises publicly shared models/outputs based on SA content take the same licence, NC holds at every stage, ND cautious reading: don't train. Implication: one SA file makes CC BY-SA the safe adapter licence — keep SA in its own lane.

## Recommendation (seat)
Tier A: OpenScore, Mutopia PD, IMSLP CC0 typesets. Tier B: gated PDMX, Mutopia/IMSLP CC BY, MusicNet. Tier C: Mutopia/IMSLP CC BY-SA, piano-midi.de. Held: GiantMIDI-Piano (inference: transcriber's licence over others' performances). Out: NC, gated, scraped, LLM-barred — every MIDI corpus ≥1M files. Scale: ~1,700 CC0 OpenScore + Mutopia PD share (count not retrieved) + 222,856 PDMX candidates before gating. Sample-library licence for rendered audio not researched.
Per-file gate: composer death year + first-publication year (US <1931, life+70); named source edition + year, reject scholarly editions <25 yrs; read in-file licence (MusicXML rights, MIDI copyright meta, Humdrum !!!YEC/!!!YEM) and require match with host page; allowlisted licences with no AI-use clause; title + opening notes match the claimed work; store URL, retrieval date, file hash.
