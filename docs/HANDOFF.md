# Handoff — si-jam-sessions, 2026-09-26

Read this first, then [`PHASE-0.md`](PHASE-0.md). It says where the project stands, what was decided and why,
and exactly what to do next.

## Where it stands

**Version 0.1.0 is released, and the project has had the full treatment.** `main` holds:

| Piece | PR | What it does |
|---|---|---|
| The law core | #1 | Integer time at PPQ 3360, Q = 48 samples, H = 100 quanta, an inclusive ±1,920-sample gate, a hashed little-endian snapshot, a raw C ABI, `no_std`, no floating point anywhere in the wasm |
| Ingest and provenance | #2 | SMF read by midly's lazy parser under `strict`; the licence predicate (version 2) and the receipt for *The Entertainer* |
| Decisions | #3 | The decisions slice 1 settled, in `PHASE-0.md` |
| The golden hash | #4 | A constructed take graded and hashed, checked natively on x86_64 and ARM64 and as wasm under V8, SpiderMonkey and JavaScriptCore, each engine pinned by the SHA-256 of every file it runs |
| Frames and live notes | #5 | A read-only committed-frames export and a live-note verb; score notes close 180 ms after their onset and rows, once shown, never change |
| The host | #6 | cpal (WASAPI shared), rtrb, a callback that allocates nothing, oscillator voices and a click, MIDI through WinMM directly, a computer-keyboard fallback, a silent pre-roll; `devices`, `play`, `render`, `jam` |
| Research | #8 | The Battle Hymn's evidence and its 1862 reference transcription |
| The first handoff | #9 | This file, and the external-review tools in `tools/review/` |
| Law version 5 | #10 | Licence predicate version 3: anonymous works (first published by 1930 for the US, by 1955 for the EU, rules year 2026), CC0 1.0 and the Public Domain Mark; the Battle Hymn provenance fixture |
| The grand piano | #11 | The Salamander Grand Piano V3 sampler, `fetch-piano` (archive pinned by SHA-256), `preview`, `notices`, `--voice`; issue #7's three fixes |
| The exemplars | #14 | Both Battle Hymn arrangements admitted as scores (receipts `ee5a82df…` and `d044ffa4…`), a frames golden for each, `--piece` (default `battle-hymn-glm-5.3`), `host notes` |
| The full treatment | this one | SECURITY, CHANGELOG, SHIP_GATE, SCORECARD, `verify.sh`, the RustSec advisory scan, the host's exit status (1 usage, 2 runtime), the README and its translations, the landing page, the handbook, and PHASE-0's amendments |

- **Law version 5**, licence predicate version 3. *The Entertainer*'s golden is
  `66b59807261ff93086eed6b0c17cb4a7673e40937a83e0bd32db7ae25ef02946`. The exemplar goldens are glm-5.3
  `1c0789b1…` and kimi-k3 `0f93de92…`, checked natively on both architectures.
- **Try it:** `cargo run -p host --release -- fetch-piano` once, then `-- play`. `play` opens on the Battle Hymn
  as glm-5.3 arranged it; `--piece battle-hymn-kimi-k3` or `--piece entertainer` picks another. `-- notes
  out.json --piece <name>` writes the notes the law commits. Use a wired output for `jam`: a Bluetooth output's
  delay is past the 100 ms delivery allowance.
- **Public:**
  - the landing page is <https://mcp-tool-shop-org.github.io/si-jam-sessions/>, with the handbook under
    `handbook/`;
  - the release [v0.1.0](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0) holds both
    WAVs, the MP3 and Opus copies, both notes files, `sections.json` and `SHA256SUMS`.

## The exemplars

Two models arranged *Battle Hymn of the Republic* from this project's transcription of the anonymous 1862
Ditson edition. The owner chose to publish both side by side, because how two models' arrangements of the same
source differ is itself useful to show.

| | glm-5.3 | kimi-k3 |
|---|---|---|
| The project's change to the answer | the Markdown code fence removed | `\language "english"` added |
| Length, notes | 5:02, 1,720 | 4:37, 1,924 |
| WAV SHA-256 | `85b2d567…2eed` (115,998,136 bytes) | `bf49d310…a017` (106,486,216 bytes) |

- **Rendered through the law** on the piano, with no click. A second render on this Windows machine gives the
  same bytes. A render on Linux has not been compared yet (see next steps).
- **The page's recordings are release assets.** `pages.yml` downloads the MP3 and Opus files from v0.1.0 and
  checks each against `site/audio.sha256` before it builds. Deleting or replacing the release breaks the next
  Pages build on purpose.
- **The comparison view** (`site/src/components/Exemplars.astro`, `site/src/lib/view.ts`) draws both
  arrangements from their `host notes` exports, aligned section by section. `tools/site/sections.py` finds the
  section starts:
  - it places a MIDI program change at each section of a copy of each `.ly`;
  - it converts the ticks through the tempo map, first checked against every beat the law committed;
  - it snaps each start to a committed onset.

  `site/src/lib/compare.ts` measures each section: length, notes a second, median note, average velocity, and
  the most notes sounding at once. It states the largest difference.
- **To regenerate the page's data** after a score changes, from the repository root:
  1. run `host notes site/src/data/<piece>.notes.json --piece <piece>` for each piece;
  2. run `py -3 tools/site/sections.py <lilypond> scores site/src/data site/src/data/sections.json`, with
     LilyPond 2.24.4.

## Decisions taken, and why

- **The exemplar is *Battle Hymn of the Republic*,** played through by a rich grand piano. *God Bless America*
  was the first wish, but Irving Berlin published it in 1938 and died in 1989, so it is protected in the United
  States until 2034 and in the European Union until 2060, and the law refuses it. *The Entertainer* stays as an
  option and as slice 1's golden.
- **The project's own arrangements are CC0.**
- **The tune is recorded as anonymous.** William Steffe is the most-cited claimant, but the Library of
  Congress's authority record says no claim can be sustained, and the 1859 printings name no composer.
- **The piano is the Salamander Grand Piano V3** (Yamaha C5, 16 velocity layers, CC BY 3.0), chosen for
  richness over the CC0 VCSL Steinway B. Its credit is beside the players and in every WAV.
- **Arrangements follow only the 1862 Ditson edition.** Anything published after 1930 is still protected.
- **A receipt says exactly what the project changed.** Round 9 caught a glm-5.3 receipt that claimed nothing
  had changed, when the answer's code fence had been removed. The note now names it, and the receipt digest and
  golden moved with it.
- **Version numbers are frozen when they reach `main`;** before that a number may be refined, but one number
  never names two different goldens.
- **The version stays 0.x;** 1.0 means the product is 1.0.

## Next steps, in order

1. **Compare the exemplar renders on Linux.** Dispatch CI's piano job (`workflow_dispatch` on `ci.yml`). It
   renders each exemplar whole with the real samples. If its SHA-256s equal the release's, say "the same on
   Windows and Linux" on the README, the page and the handbook. If they differ, find out why before claiming
   anything.
2. **The JavaScript engines check the exemplar goldens** (#15): an `engine-js --exemplar <id>` mode.
3. **The listening test,** and a note-by-note check of both arrangements against the 1862 edition. Neither has
   been done, and the surfaces say so.
4. **The owner's MIDI keyboard.** WinMM's device-only premises (serial callbacks per port; none after
   `midiInClose`) meet a real keyboard for the first time. The surfaces say live input is untested until then.
5. **Upstream fixes found during the treatment:**
   - `@mcptoolshop/site-theme`: CodeCardGrid's cards need `min-width: 0`, patched here in `global.css` until
     then; the theme could also offer an audio section.
   - `@mcptoolshop/shipcheck`: `shipcheck ci` should recognise `cargo deny … advisories` as a dependency
     scanner.
6. **Keep this handoff current.**

## Reviews

The external verifier is never the author's model family. Since round 5 it is a panel of two Ollama Cloud
models from different families, run in parallel with thinking raised and each blind to the other; a read-only
Claude consult may add a supplementary check.

- **Round 9** (#14): approve, and approve with fixes. One MED, the glm-5.3 receipt, was fixed in `14f0dce`, and
  the two LOW findings are in #15.
- **Round 10** (the public surfaces, reviewed against the code): approve with fixes.
  - Fixed: "Windows runs everything" now notes that live input has not yet met a real keyboard; "bit for bit,
    every time" is limited to the same machine; the handbook lists the median note among the compared
    measures; the README's replay and training-data lines are written as design; glm-5.3's two calls are
    stated; the cache path names its piano folder.
  - Not changed, because the reviewers lacked the files: the CI import check, `sections.py`'s method, and the
    thinking levels.
- **Round 11** (the host's exit status): see the merge commit's body.

How the reviews run:

- `tools/review/cloud_review.py` sends a brief and a packet to an Ollama Cloud model through the local daemon
  and records the call.
- Model names need the `:cloud` suffix (`kimi-k3:cloud`); without it the daemon answers 404.
- Check the live model list first (`https://ollama.com/api/tags`).
- Each merge commit's body records its reviews by head.

## Open items

- Issue #12: three LOW host findings from round 8.
- Issue #15: the engines and the exemplar goldens; `notes`, `render` and `preview` replace an existing file
  without asking.
- In piano mode the take and live notes still sound on the oscillator. That was the host agent's choice;
  confirm or change it.
- CC0's SPDX forms (`CC0-1.0`) are still refused. Admitting them is one line and one test.
- Two readings the 1862 scans leave open: the octave of the A at bar 19, beat 4, and the alto's last note in
  bar 13.
- An automated-access class (scraping, crawling) for a later predicate version.
- Standard rights statements that contain a negating word ("No known copyright restrictions") are refused
  until curated.
- The npm scope `@si-jam-sessions` is reserved and empty. Publish through trusted publishing, never a token,
  and add its row to PHASE-0's compensators when the first package ships.

## Working rules

- Never quote the owner in any artifact: code, commits, PRs, docs.
- Public surfaces (README, landing page, handbook, this file) are written by the lead, never by a subagent.
- Tests ship with code, red first; never loosen a test to pass.
- Run the identity scan before every public push; pass commit messages and PR bodies through files.
- Agents launched from a local session run locally, whatever isolation they ask for. Work meant for Claude's
  cloud needs a cloud session that the owner starts. Reviews and generation run on Ollama Cloud.
