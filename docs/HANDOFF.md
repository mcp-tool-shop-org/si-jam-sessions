# Handoff — si-jam-sessions, 2026-09-26

Read this first, then [`PHASE-0.md`](PHASE-0.md). It says where the project stands, what is in flight, what
was decided and why, and exactly what to do next.

## Where it stands

**Slice 1 is complete and merged.** `main` holds:

| Piece | PR | What it does |
|---|---|---|
| The law core | #1 | Integer time at PPQ 3360, Q = 48 samples, H = 100 quanta, an inclusive ±1,920-sample gate, a hashed little-endian snapshot, a raw C ABI, `no_std`, no floating point anywhere in the wasm |
| Ingest and provenance | #2 | SMF read by midly's lazy parser under `strict`; the licence predicate (version 2) and the receipt for *The Entertainer* |
| Decisions | #3 | The decisions slice 1 settled, in `PHASE-0.md` |
| The golden hash | #4 | A constructed take graded and hashed, checked natively on x86_64 and ARM64 and as wasm under V8, SpiderMonkey and JavaScriptCore, each engine pinned by the SHA-256 of every file it runs |
| Frames and live notes | #5 | A read-only committed-frames export and a live-note verb; score notes close 180 ms after their onset and rows, once shown, never change |
| The host | #6 | cpal (WASAPI shared), rtrb, a callback that allocates nothing, oscillator voices and a click, MIDI through WinMM directly, a computer-keyboard fallback, a silent pre-roll; `devices`, `play`, `render`, `jam` |
| Research | #8 | The Battle Hymn's evidence and its 1862 reference transcription |

- **Law version 4**, licence predicate version 2. Golden `fd574ccce7ae9eb389642b5c7372c506c7d2787b0e1f3b190655e49b80695baa`
  (version 3's was `145c7af9…`; a test proves the only snapshot difference is the version word).
- **Try it:** `cargo run -p host --release -- devices`, then `-- play --output <n>` or `-- render out.wav`.
  Use a wired output for `jam`: a Bluetooth output's delay is past the 100 ms delivery allowance.

## In flight

Two Claude cloud agents were dispatched on 2026-09-26. Each opens a PR against `main` and does not merge.

1. **The law agent — licence predicate version 3, law version 5.**
   - Anonymous works: an unknown author is recorded as unknown. The US check uses the first-publication year;
     the EU check uses publication at or before the rules year minus 71 (Directive 2006/116/EC, Art. 1(3)).
   - CC0 and the Public Domain Mark admitted as public-domain equivalents; the project's own engravings may
     state CC0 1.0.
   - "forbade", "restrictive" and "prohibitory" join the negating words.
   - The golden moves with the version words, proven by a byte-patch test back to `fd574ccc…`; the
     Entertainer's receipt digest `e23ba2e9…` must not change.
2. **The host agent — the grand piano.**
   - The three LOW findings in issue #7.
   - The Salamander Grand Piano V3 as a sampled voice: a `fetch-piano` command with a pinned SHA-256, FLAC
     decoded by a crate on the licence allowlist, the CC BY 3.0 credit printed whenever it plays and written
     into every rendered WAV, `--voice piano|osc`.
   - A `preview` command that renders a MIDI file straight through the sampler, labelled as not the law's
     frames, for auditioning drafts.
   - LilyPond 2.24 compiles the arrangement drafts on branch `exemplar/drafts`; compressed previews go to
     branch `exemplar/previews` for the owner to hear.

**Arrangement drafts** (branch `exemplar/drafts`): kimi-k3 wrote a complete one (introduction, five verse
and chorus passes building from *p* to *ff*, a broadening coda). glm-5.3 at high reasoning spent its whole
output budget thinking and returned nothing; it and minimax-m3 are being re-run.

## Decisions taken, and why

- **The exemplar is *Battle Hymn of the Republic*,** played through by a rich grand piano. *God Bless America*
  was the first wish, but Irving Berlin published it in 1938 and died in 1989, so it is protected in the United
  States until 2034 and in the European Union until 2060, and the law refuses it. *The Entertainer* stays as an
  option and as slice 1's golden.
- **The project's own arrangements are CC0.** That settles the PHASE-0 question of their licence.
- **The tune is recorded as anonymous.** William Steffe is the most-cited claimant, but the Library of
  Congress's authority record says no claim can be sustained, and the 1859 printings name no composer.
- **The piano is the Salamander Grand Piano V3** (Yamaha C5, 16 velocity layers, CC BY 3.0), chosen for
  richness over the CC0 VCSL Steinway B. Its credit goes beside the player on the landing page, and
  `PHASE-0.md`'s line that the Salamander piano stays in the sibling needs amending.
- **Arrangements follow only the 1862 Ditson edition.** Anything published after 1930 is still protected.
- **Version numbers are frozen when they reach `main`;** before that a number may be refined, but one number
  never names two different goldens.

## Next steps, in order

1. **Review and merge the two cloud PRs.** Run the external review with the tools in `tools/review/` (below);
   fix, re-check the delta, merge. Settle issue #7's disputed finding by its test.
2. **The owner picks an arrangement by ear** from `exemplar/previews`.
3. **Admit the exemplar.** Put the chosen arrangement in `scores/battle-hymn/` (`.ly`, the `.mid` LilyPond
   renders, and a receipt built from `research/battle-hymn/evidence-manifest.json`), under predicate version
   3. Consider a second golden for it.
4. **Make `play` open on the Battle Hymn,** with *The Entertainer* still available.
5. **The full treatment and publication.**
   - Shipcheck and the identity scan first; the version stays 0.x, because 1.0 means the product is 1.0.
   - A logo in the brand repository, the README, then its translations.
   - The landing page, with **the exemplar on it**: a compressed recording of the full grand-piano render
     playing on the page; a three.js keyboard or falling-notes view driven by the law's committed notes,
     exported as a small data file; the uncompressed WAV as a GitHub release asset with its SHA-256 and the
     one command that reproduces it bit for bit; the Salamander credit beside the player. The landing-page
     theme probably needs an audio section, added upstream in `@mcptoolshop/site-theme`, not patched here.
   - A Starlight handbook, repository metadata, and the repo-knowledge entry; deploy and verify.
6. **Keep this handoff current.**

## Reviews

The external verifier is never the author's model family. Since round 5 it is a panel of two Ollama Cloud
models from different families, run in parallel with thinking raised and each blind to the other; a read-only
Claude consult may add a supplementary check. On the host PRs both panel models found the same HIGH finding in
each packet, independently.

- `tools/review/cloud_review.py` sends a brief and a packet (patches and notes) to an Ollama Cloud model through
  the local daemon and records the call: served model, prompt hash, token counts, reasoning, answer. With
  `--system` it also drives generation, as it did for the arrangement drafts.
- `tools/review/vision_check.py` checks a transcription against page scans with a vision model.
- The round-5 briefs beside them show the shape: every finding names a file and line, and the reviewer lists the
  hostile cases before judging.
- Check the live model list first (`https://ollama.com/api/tags`). On 2026-09-26 the useful ones were kimi-k3,
  glm-5.3 and deepseek-v4-pro. deepseek-v4-pro caps output at 65,536 tokens and exhausted it thinking;
  glm-5.3 at high reasoning did the same on a long creative task.
- Each merge commit's body records its reviews by head.

## Open items

- Issue #7: three LOW host findings (in the host agent's hands).
- Two readings the 1862 scans leave open: the octave of the A at bar 19, beat 4, and the alto's last note in
  bar 13.
- An automated-access class (scraping, crawling) for a later predicate version.
- Standard rights statements that contain a negating word ("No known copyright restrictions") are refused
  until curated.
- WinMM's device-only premises (serial callbacks per port; none after `midiInClose`) meet a real keyboard for
  the first time with the owner's MIDI keyboard.
- The npm scope `@si-jam-sessions` is reserved and empty. Publish through trusted publishing, never a token,
  and add its row to PHASE-0's compensators when the first package ships.

## Working rules

- Never quote the owner in any artifact: code, commits, PRs, docs.
- Public surfaces (README, landing page, handbook, this file) are written by the lead, never by a subagent.
- Tests ship with code, red first; never loosen a test to pass.
- Run the identity scan before every public push; pass commit messages and PR bodies through files.
- Agents run in Claude's cloud where the job allows; reviews and generation run on Ollama Cloud.
