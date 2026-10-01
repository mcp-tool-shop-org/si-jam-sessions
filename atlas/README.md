# si-jam-sessions: how it works

Mapped at 2026-10-01 from commit 7811372 by Atlas 1.24.0.

## What this is

15 parts, mostly Rust (62 files), TypeScript (4), Python (3), shell (3), Astro (2), CSS (2) and JavaScript (1). Work enters through 8 doors; CI and Container each reach 8 parts, and CI is followed because it comes first by name. It publishes to npm and a container image. It deploys a site to GitHub Pages. People run host. engine-js and write-golden are commands built from crates/golden (nothing ships them).

## What changed since the last map

This is the first map.

## What comes in

1. **CI.** On a pull request touching 11 paths; on a push to main touching 11 paths; or by hand. Runs .github/engines/verify.sh, crates/golden/src/bin/engine-js.rs, crates/golden/src/bin/write-golden.rs and 45 more; checks golden/battle-hymn-glm-5.3.golden, golden/battle-hymn-kimi-k3.golden, golden/entertainer.golden and 4 more.
2. **Container.** On a pull request touching 9 paths; when a tag matching `v*` is pushed; or by hand. Runs crates/host/src/main.rs and docker/entrypoint.sh; packs Cargo.lock, Cargo.toml, crates/ and 2 more into an image.
3. **Host (Windows).** On a pull request touching 5 paths; on a push to main touching 5 paths; or by hand. Runs crates/host/src/alloc_free.rs, crates/host/src/anchor.rs, crates/host/src/callback.rs and 16 more.
4. **Deploy site to GitHub Pages.** On a push to main touching 2 paths; or by hand. Runs site/astro.config.mjs and site/src/.
5. **Release.** When a tag matching `v*` is pushed. Runs no file this map can see.
6. **host** (a command people run). Runs crates/host/src/main.rs.
7. **engine-js** (a command built from crates/golden, which nothing ships). Runs crates/golden/src/bin/engine-js.rs.
8. **write-golden** (a command built from crates/golden, which nothing ships). Runs crates/golden/src/bin/write-golden.rs.

## What happens through CI

1. The workflow runs .github/engines/verify.sh in .github, 7 files in golden-harness, 19 files in host, crates/ingest/src/tests.rs and crates/ingest/tests/ in ingest, 12 files in law, and 7 files in 2 more parts; it checks golden/battle-hymn-glm-5.3.golden, golden/battle-hymn-kimi-k3.golden and golden/entertainer.golden in golden, crates/golden/src/lib.rs in golden-harness, crates/ingest/src/lib.rs in ingest, crates/law/src/lib.rs in law, and crates/provenance/src/lib.rs in provenance.
   1. Inside crates/golden/src/bin/engine-js.rs, `main` does, in order:
      1. `lib.rs` (4 steps)
      2. `run.rs` (4 steps)
      3. `new` (Error)
      4. `render`
      5. `new` (Error)
      6. `run.rs` (4 steps)
      7. `new` (Error)
      8. `read_dir` (Inputs)
      9. `compute`
      10. `new` (Error)
      11. `parse` (GoldenFile)
      12. `new` (Error)
   2. Or, when `role != Role::Check`, `main` does `new` (Error) instead.
   3. Or, when `committed.all(key) != regenerated.all(key)`, `main` does `new` (Error) and `new` (Error) instead.
   4. **`compute`** runs, in order:
      1. `ingest` (Law, law)
      2. `new` (Error)
      3. `draw`
      4. `construct`
      5. `encode_take`
      6. `new` (Error)
      7. `law_alloc`
      8. `law_free`
   5. **`compute`** runs, in order:
      1. `new` (Error)
      2. `ingest` (Law, law)
      3. `new` (Error)
      4. `encode_frames`
      5. `pass`
      6. `snapshot_hash`
      7. `law_snapshot_ptr`
      8. `law_snapshot_len`
      9. `read_out`
      10. `law_step`
   6. Inside crates/golden/src/bin/write-golden.rs, `main` does, in order: `repo_root`, `read` (Inputs), `compute`, `compute_at` and `run.rs` (3 steps).
   7. Or, when `check`, `main` does `run.rs` (4 steps) instead.
   8. **`compute`** runs, in order:
      1. `ingest` (Law, law)
      2. `new` (Error)
      3. `draw`
      4. `construct`
      5. `encode_take`
      6. `new` (Error)
      7. `law_alloc`
      8. `law_free`
   9. **`compute_at`** runs, in order:
      1. `read_dir` (Inputs)
      2. `new` (Error)
      3. `ingest` (Law, law)
      4. `new` (Error)
      5. `encode_frames`
      6. `pass`
      7. `snapshot_hash`
      8. `law_snapshot_ptr`
      9. `law_snapshot_len`
      10. `read_out`
   10. Inside crates/host/src/main.rs, `main` does, in order:
      1. `parse` (Name)
      2. `outputs`
      3. `root`
      4. `load` (Piece)
      5. `choose_output`
      6. `dir_or_default`
      7. `verified`
      8. `default` (Needs)
      9. `open` (Bank)
      10. `acquire` (Law)
      11. `default` (Shared)
      12. `new` (Scheduler), and 12 more

## Who reads the results

CI writes nothing this map can see.

## The other doors

**Container** runs crates/host/src/main.rs and docker/entrypoint.sh, packs Cargo.lock, Cargo.toml, crates/ and 2 more into an image, and publishes a container image on a tag push or by hand.

**Host (Windows)** runs crates/host/src/alloc_free.rs, crates/host/src/anchor.rs, crates/host/src/callback.rs and 16 more, and reaches golden-harness, ingest, law, provenance and score-model.

**Deploy site to GitHub Pages** runs site/astro.config.mjs and site/src/, and deploys the site.

**Release** runs no file this map can see, publishes to npm, and creates a GitHub release.

**host** (a command people run) runs crates/host/src/main.rs and reaches golden-harness, ingest, law, provenance and score-model.

**engine-js** (a command built from crates/golden, which nothing ships) runs crates/golden/src/bin/engine-js.rs and reaches ingest, law, provenance and score-model.

**write-golden** (a command built from crates/golden, which nothing ships) runs crates/golden/src/bin/write-golden.rs and reaches ingest, law, provenance and score-model.

## What breaks what

- **score-model** is imported by 4 parts (golden-harness, host, ingest, law) and sits on the path of 6 doors.
- **ingest** is imported by 2 parts (host, law) and sits on the path of 6 doors.
- **law** is imported by 2 parts (golden-harness, host) and sits on the path of 6 doors.
- **provenance** is imported by 2 parts (golden-harness, law) and sits on the path of 6 doors.
- **golden-harness** is imported by 1 part (host) and sits on the path of 6 doors.
- **host** is imported by no other part and sits on the path of 4 doors.

docker holds only shell files, which this map does not read, so what uses it cannot be seen.

## What tends to change together

- **crates/law/src/law.rs** and **crates/law/src/refusal.rs** changed together in 7 of 9 commits, inside the law part.
- **crates/law/src/law.rs** and **crates/law/src/lib.rs** changed together in 8 of 12 commits, inside the law part.
- **crates/law/src/abi.rs** and **crates/law/src/law.rs** changed together in 6 of 9 commits, inside the law part.
- **crates/host/src/alloc_free.rs** and **crates/host/src/synth.rs** changed together in 4 of 6 commits, inside the host part.
- **crates/host/src/alloc_free.rs** and **crates/host/src/winmm.rs** changed together in 4 of 6 commits, inside the host part.

Confidence is low: fewer than 20 source files reach 10 revisions in the window.

Window: 180 days; a pair counts from 3 shared commits, since 5 source files reach 10 revisions; the floor rises to 10 when 25 do.

## What no test touches

- **tools** is imported by no test.

golden-harness is tested only by the unit tests in its own files.

host is tested only by the unit tests in its own files.

ingest is tested only by the unit tests in its own files.

law is tested only by the unit tests in its own files.

provenance is tested only by the unit tests in its own files.

score-model is tested only by the unit tests in its own files.

docker holds only shell files, which this map does not read, so whether a test touches it cannot be seen.

## Written but never read

No place this map can see is written, so none goes unread.

## Helpers that look duplicated

No two parts export a helper that looks alike.

## Generated, never hand-edited

Nothing in this repository writes to a tracked place this map can see.

## Hand-authored

People write .github/, docs/, golden/, research/, the repository root, scores/ and site/; 20 writes with paths built at run time may land here.

## Where to start

.github/workflows/ci.yml → crates/host/src/main.rs → crates/host/src/lib.rs → crates/host/src/bridge.rs

Read those in order to follow one pull request end to end.

## What this map cannot see

- 2 imports could not be resolved: `crates/provenance/src/tests.rs` imports `LicenceRefusal::*`, twice.
- 20 writes and 12 reads use paths built at run time and are not named here.
- 3 writes and 7 reads go to a path their caller passes, not to this repository.
- Statistics confidence is low: fewer than 20 source files reach 10 revisions in the window.

Regenerate with `npx --yes @dogfood-lab/atlas map`.
