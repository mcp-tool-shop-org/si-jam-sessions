# Scorecard

> Score a repo before remediation. Fill this out first, then use SHIP_GATE.md to fix.

**Repo:** si-jam-sessions
**Date:** 2026-09-26
**Type tags:** `[all]` `[cli]` `[org]`

## Pre-Remediation Assessment

| Category | Score | Notes |
|----------|-------|-------|
| A. Security | 6/10 | No SECURITY.md and no trust model in the README. The code itself was sound: no telemetry, no credentials, one pinned download with a checked unpacker, and a law with no I/O |
| B. Error Handling | 6/10 | The law's refusals carry codes and reasons. The host exited 1 for every failure, with no hint |
| C. Operator Docs | 5/10 | The README still described the first build as future work, and there was no CHANGELOG. `--help`, PHASE-0 and the handoff were current |
| D. Shipping Hygiene | 7/10 | Strong CI: the golden hash on two architectures and three JavaScript engines, and licences through cargo-deny. There was no verify script and no advisory scan |
| E. Identity (soft) | 2/10 | A description and topics only: no logo, landing page, handbook or translations |
| **Overall** | **26/50** | |

## Key Gaps

1. No SECURITY.md, and no trust model in the README.
2. The host's exit status did not tell a wrong command line from a failed run.
3. The README and CHANGELOG did not describe what had shipped.
4. No RustSec advisory scan in CI, and no one-command verify.
5. No logo, landing page, handbook, translations or homepage.

## Remediation Priority

| Priority | Item | Estimated effort |
|----------|------|-----------------|
| 1 | SECURITY.md, the README's trust model, the CHANGELOG | Small |
| 2 | The host's exit status (1 for usage, 2 for runtime) with a hint; the advisory scan; `verify.sh` | Small |
| 3 | Logo, landing page with both exemplars, handbook, translations, metadata | Large |

## Post-Remediation

| Category | Before | After |
|----------|--------|-------|
| A. Security | 6/10 | 10/10 |
| B. Error Handling | 6/10 | 9/10 |
| C. Operator Docs | 5/10 | 10/10 |
| D. Shipping Hygiene | 7/10 | 9/10 |
| E. Identity (soft) | 2/10 | 10/10 |
| **Overall** | 26/50 | 48/50 |

B keeps one point because the host's errors are messages with a class, not individual codes. D keeps one because
the exemplar goldens are checked natively but not yet under the three JavaScript engines (#15).
