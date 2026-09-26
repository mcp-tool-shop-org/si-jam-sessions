# Ship Gate

> No repo is "done" until every applicable line is checked.
> Copy this into your repo root. Check items off per-release.

**Tags:** `[all]` every repo · `[npm]` `[pypi]` `[vsix]` `[desktop]` `[container]` published artifacts · `[mcp]` MCP servers · `[cli]` CLI tools

This repository is a Rust workspace whose product is the `host` command-line program and the law's WebAssembly
module. Nothing is published to a package registry. The 0.1.0 release carries the two exemplar recordings.

---

## A. Security Baseline

- [x] `[all]` SECURITY.md exists (report email, supported versions, response timeline) (2026-09-26; `shipcheck security-docs` passes)
- [x] `[all]` README includes threat model paragraph (data touched, data NOT touched, permissions required) (2026-09-26: "Trust model"; `shipcheck security-docs` passes)
- [x] `[all]` No secrets, tokens, or credentials in source or diagnostics output (2026-09-26: `shipcheck secrets` has no registry package to scan, so the identity scan of the git-tracked tree was run instead and is clean; the code reads, stores and sends no credentials)
- [x] `[all]` No telemetry by default — stated in the README's trust model and in SECURITY.md (2026-09-26)

### Default safety posture

- [ ] `[cli|mcp|desktop]` SKIP: the host has no kill, delete or restart action. The one overwrite, of a file the user names for `render`, `notes` or `preview`, is documented, and asking first is tracked in #15.
- [x] `[cli|mcp|desktop]` File operations constrained to known directories: the per-user piano cache and the paths the user names; the unpacker refuses links and paths that climb out (2026-09-26)
- [ ] `[mcp]` SKIP: not an MCP server. The host uses the network only for `fetch-piano`.
- [ ] `[mcp]` SKIP: not an MCP server.

## B. Error Handling

- [x] `[all]` Errors follow the Structured Error Shape: the law's refusals carry a numeric code and a reason; the host reports `host: <message>`, a usage error adds a hint, and the exit status is the error's class (2026-09-26)
- [x] `[cli]` Exit codes: 0 ok · 1 user error · 2 runtime error (2026-09-26; tested in the binary's tests; there is no partial success)
- [x] `[cli]` No raw stack traces without `--debug`: errors are one-line messages; a panic aborts with Rust's one-line panic message, and no backtrace unless `RUST_BACKTRACE` is set (2026-09-26)
- [ ] `[mcp]` SKIP: not an MCP server.
- [ ] `[mcp]` SKIP: not an MCP server.
- [ ] `[desktop]` SKIP: not a desktop application.
- [ ] `[vscode]` SKIP: not a VS Code extension.

## C. Operator Docs

- [x] `[all]` README is current: what it does, install, usage, supported platforms + runtime versions (2026-09-26; reviewed against the code by two external models, round 10)
- [x] `[all]` CHANGELOG.md (Keep a Changelog format) (2026-09-26)
- [x] `[all]` LICENSE file present and repo states support status (MIT; SECURITY.md states the supported version) (2026-09-26)
- [x] `[cli]` `--help` output accurate for all commands and flags (2026-09-26; the handbook's reference page is built from it)
- [ ] `[cli|mcp|desktop]` SKIP: the host has one output level. Each command prints its report, and the piano prints its credit; there are no secrets to redact.
- [ ] `[mcp]` SKIP: not an MCP server.
- [ ] `[complex]` SKIP: not an operated service. The Starlight handbook covers use, and `fetch-piano` resumes or refetches on its own.

## D. Shipping Hygiene

- [x] `[all]` `verify` script exists (test + build + smoke in one command): `verify.sh` runs every gate of CI's rust job (2026-09-26)
- [x] `[all]` Version in manifest matches git tag: every crate is 0.1.0 and the release tag is `v0.1.0` (2026-09-26; `shipcheck manifest` has no npm or PyPI manifest to read)
- [x] `[all]` Dependency scanning runs in CI: `cargo deny --locked check licenses advisories` checks every dependency against the RustSec advisory database (2026-09-26). `shipcheck ci` does not recognise cargo-deny yet and reports no scanner; that is a gap in shipcheck, not in this repository.
- [x] `[all]` No known high/critical vulnerabilities in any dependency tree, and Dependabot alerts are enabled (2026-09-26: `cargo deny check advisories` ok; `npm audit` on `site/` finds 0; the vulnerability-alerts API answers 204)
- [ ] `[all]` SKIP: optional. The org rule reserves the update bot for when it is asked for; alerts are on.
- [ ] `[npm]` SKIP: nothing is published to npm (`site/` is a private package).
- [ ] `[npm]` SKIP: nothing is published to npm.
- [ ] `[npm]` SKIP: nothing is published to npm or PyPI.
- [ ] `[npm]` SKIP: nothing is published to npm or PyPI.
- [ ] `[vsix]` SKIP: not a VS Code extension.
- [ ] `[desktop]` SKIP: not a desktop application.

## E. Identity (soft gate — does not block ship)

- [ ] `[all]` Logo in README header
- [ ] `[all]` Translations (polyglot-mcp, 8 languages)
- [ ] `[org]` Landing page (@mcptoolshop/site-theme)
- [ ] `[all]` GitHub repo metadata: description, homepage, topics

---

## Gate Rules

**Hard gate (A–D):** Must pass before any version is tagged or published.
If a section doesn't apply, mark `SKIP:` with justification — don't leave it unchecked.

**Soft gate (E):** Should be done. Product ships without it, but isn't "whole."

**Executed vs attested.** `shipcheck audit` only *counts these checkboxes* — it does not read your repo, so a box can be green while the fact is false. The lines that say **"executed by `npx @mcptoolshop/shipcheck <gate>`"** are backed by a command that reads the real artifact and exits 1 on the real defect. Run those gates (they are wired into shipcheck's own `verify`); don't just tick their boxes. Executed today: **A1/A2** (`security-docs`), **A3** (`secrets`), **D2/D6/D7** (`manifest`), **D3-config + OIDC/provenance** (`ci`), **real vulnerabilities + alerting** (`deps`), **D5** (`pack`), plus front-door (`front-door`) and dogfood freshness (`dogfood`). Every other line is still an attestation you are vouching for. Note the two dependency layers: `ci` proves a scanner is *configured*; `deps` proves there are *no known vulnerabilities* — a repo can pass the first while failing the second.
