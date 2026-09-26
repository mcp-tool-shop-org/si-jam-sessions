# Finish the full treatment — kickoff for a Claude Code cloud session (2026-09-26)

**Paste target:** a Claude Code session on claude.ai/code, repository `mcp-tool-shop-org/si-jam-sessions`,
branch `treatment/full`. The local session that did the treatment ran out of budget before the release and the
merge. This file is the whole brief. `docs/HANDOFF.md` on this branch is written for the state **after** these
steps. Its banner says so, and step 9 removes the banner.

## State when this was written

- `main` is `39970a3`. PR #14, the two Battle Hymn exemplars, is merged.
- **Branch `treatment/full`** holds the whole treatment, committed and pushed, **not merged**. Before the push
  it passed `verify.sh`, `npm run build` in `site/`, and the owner's identity scan (`RESULT CLEAN`).
- **The draft release `v0.1.0`** holds the assets, which exist only on the owner's machine otherwise:
  - both WAVs;
  - the MP3 and Opus copies;
  - both notes files;
  - `sections.json` and `SHA256SUMS`.

  It is not published, so there is no tag yet.
- **The logo** is live at `https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/si-jam-sessions/readme.png`
  (brand `4164ee6`).
- **Reviews:**
  - **Round 9** (#14): merged with its fixes.
  - **Round 10** (the public surfaces against the code): approve with fixes, all applied.
  - **Round 11** (the host's exit status): approve with fixes, all applied. `status()` is tested, `help` takes
    no argument, reports are written without panicking, and the usage text covers device names. The delta has
    **not** been re-reviewed, because the Ollama panel runs on the owner's machine. Say so in the PR body.

## What the branch adds

- `SECURITY.md`, `CHANGELOG.md` (0.1.0), `SHIP_GATE.md` (A–D all checked or SKIP; the four E items open),
  `SCORECARD.md`, and `verify.sh`.
- CI's cargo-deny step now also checks RustSec advisories (`ci.yml`, `deny.toml`).
- The host's exit status: 0 success, 1 for a wrong command line (with a hint), 2 for a runtime failure. The
  code is in `crates/host/src/main.rs`, and the tests are in its test module.
- The README, rewritten, and its seven translations. The logo, badges and language bar are in place.
- The landing page and handbook in `site/` (Astro, `@mcptoolshop/site-theme` 2.2.0, Starlight):
  - two players;
  - a three.js view of both arrangements aligned section by section (`src/lib/view.ts`, loaded lazily);
  - a static table of where they differ (`src/lib/compare.ts`);
  - seven handbook pages.
- `.github/workflows/pages.yml`. It downloads the MP3 and Opus files from release `v0.1.0` and checks them
  against `site/audio.sha256` before it builds, so **the release must be published before the merge**.
- `tools/site/sections.py`, which regenerates `site/src/data/sections.json` with LilyPond 2.24.4.
- PHASE-0's amendments: the piano is the first sample socket, it no longer stays in the sibling, and the
  host's pin row gains two crates. HANDOFF is rewritten.

## Steps, in order

1. **Read** `docs/HANDOFF.md`, then this file.
2. **Fix the Hindi footer.** In `README.hi.md` the footer lost the words `MCP Tool Shop`. Make its last line
   link text exactly `MCP Tool Shop`, as the other six do. Spot-check `README.ja.md` and `README.zh.md` for
   degenerate output: repeated lines, or untranslated blocks where the others are translated.
3. **Check the gates:**
   - `sh verify.sh` needs the pinned toolchain from `rust-toolchain.toml` and `cargo install cargo-deny
     --version 0.20.2 --locked`.
   - In `site/`, run `npm ci && npm run build`. `dist/index.html`, `dist/handbook/index.html` and
     `dist/pagefind/` must exist.
4. **Tick the four E items** in `SHIP_GATE.md`, with the date: logo, translations, landing page, metadata. Each
   becomes true in the steps below. Then `npx @mcptoolshop/shipcheck audit` must exit 0.
5. **Open the PR** `treatment/full` → `main`, the body passed through a file. Say what the branch adds (the
   list above), the reviews (rounds 10 and 11, fixes applied, round 11's delta not re-reviewed), and that the
   version stays 0.1.0. End it with the Claude Code attribution line. Read CI once when it should be done; do
   not poll in a loop.
6. **Enable Pages with the Actions source:**

   ```bash
   gh api -X POST repos/mcp-tool-shop-org/si-jam-sessions/pages -f build_type=workflow
   ```

   If the site already exists, run the same call with `-X PUT`.
7. **Publish the release at the PR's head**, before merging:
   - `gh release edit v0.1.0 --repo mcp-tool-shop-org/si-jam-sessions --draft=false --latest --target <PR head SHA>`
   - Confirm `gh release view v0.1.0` lists all assets, and that the tag exists.
8. **Merge the PR** with a merge commit. Its body records rounds 10 and 11 by head.
   - The merge runs `pages.yml`. If the build fails at the download step, the release is not published or
     lacks an asset; fix that first.
   - Confirm the deploy with `gh run list --workflow pages.yml --limit 1`, read once.
9. **Remove the banner** at the top of `docs/HANDOFF.md`, in the PR before step 8 or in a one-line follow-up PR.
10. **Set the metadata:**

    ```bash
    gh repo edit mcp-tool-shop-org/si-jam-sessions --homepage https://mcp-tool-shop-org.github.io/si-jam-sessions/ --add-topic piano,lilypond,threejs,provenance,public-domain
    ```
11. **Verify the live site:**
    - `https://mcp-tool-shop-org.github.io/si-jam-sessions/` loads, and both players play (the MP3 and Opus
      files come from the release).
    - The 3D view renders and follows the music, and the section chips and "Whole piece" work.
    - `/handbook/` loads, and `/si-jam-sessions/pagefind/pagefind.js` answers 200.
    - The README's logo renders on GitHub.
12. **Compare the exemplar renders on Linux:** `gh workflow run ci.yml --ref main`. The dispatch-only `piano`
    job fetches the real samples and renders each exemplar whole. If its printed SHA-256s equal the release's
    (`85b2d567…2eed`, `bf49d310…a017`), the surfaces may say "the same on Windows and Linux". Until then they
    say only "on the same machine". HANDOFF's next step 1 is this.

## What a cloud session cannot do (leave for a local session)

- **The identity scan.** Its needles live only on the owner's machine. Keep edits to the ones above, and never
  add a local path, a mailbox, a machine name or a person's name. The next local session re-scans `main`.
- **The Ollama review panel** (`tools/review/cloud_review.py` goes through the owner's local daemon). Round
  11's delta and any new code wait for it.
- **The repo-knowledge entry,** full treatment phase 5, which needs `E:/AI/repo-knowledge` locally.

## Compensators

| Action | Undo | Owner |
|---|---|---|
| Publish `v0.1.0` | `gh release edit v0.1.0 --draft=true`, then `git push --delete origin v0.1.0`. A deleted release breaks the next Pages build on purpose | owner or session |
| Enable Pages | `gh api -X DELETE repos/mcp-tool-shop-org/si-jam-sessions/pages` | owner or session |
| Merge to `main` | `git revert -m 1 <merge sha>` and push; Pages redeploys the prior site | owner or session |
| Metadata | `gh repo edit … --remove-topic <t>` for each topic, and `--homepage ""` | session |

## Rules that still hold

- Never quote the owner in any artifact.
- The session finishing this is the lead, and writes every public surface itself.
- Tests ship red first. Commit messages and PR bodies go through files. Commits end with the attribution line.
- The version stays 0.x.
- Don't poll CI in a loop; read it once when it should be done.
