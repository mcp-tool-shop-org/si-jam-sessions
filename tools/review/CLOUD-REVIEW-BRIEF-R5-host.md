# External review brief — round 5, packet H2 (2026-09-26): si-jam-sessions PR #6, the native host

You are the **external verifier**: a different model family from the authors (Claude agents), seeing only the
change and its evidence, never the authors' reasoning. **Read-only**: you cannot run anything; reason from the
text below. Every finding names a file and line (from the patch), what is wrong, why it matters, and the smallest
fix. Say "no findings" when the packet is sound. Do not assume the contents of files you were not given.

**Before you judge, list the cases a hostile reviewer would try** against each check below (device loss, an
xrun, a callback larger than the ring's cover, a sample-rate or buffer-size change, a MIDI port unplugged
mid-jam, two ports, a note held for minutes, clock drift, a Bluetooth output whose latency exceeds the 100 ms
allowance, a keystroke that arrives before law time starts, shutdown in every order), then check each case
against the code.

## The change

- PR #6, head `a4c2117`, stacked on PR #5 at `1e7f92e` (the law's frames and live-note verb, law version 4).
  CI green: 7 jobs on ubuntu (the host's 45 tests), and a Windows job (47 tests, clippy `-D warnings`).
- Files that follow: `host-a4c2117.patch` (`git format-patch 1e7f92e..a4c2117`: the host crate, its CI, 23
  files) and `host-NOTES-a4c2117.md` (the authors' facts).
- The design lock (from `docs/PHASE-0.md`): the host owns the audio device and the callback, plays committed
  frames, timestamps human input, and decides nothing; cpal 0.18.2, WASAPI shared mode, an rtrb SPSC ring, no
  allocation on the callback; a person's own note reaches their ears through the host's shortest path; live MIDI
  input is timestamped by the host and anchored to the law's sample clock, re-anchored for drift.
- MIDI input calls WinMM directly through the `windows` crate (midir was rejected: its dependency tree fails the
  licence allowlist). This is unsafe FFI the project owns.

## Check

1. **WinMM FFI.** Nothing inside the input callback calls a multimedia function; it pushes lock-free and never
   allocates or blocks; the context pointer outlives every callback (stop, reset, close, then free; freed only
   after a successful close); MIM_DATA, MIM_ERROR, MIM_LONGDATA, MIM_OPEN/CLOSE handling; the timestamp unit
   (milliseconds since midiInStart) and its anchoring to the audio clock.
2. **The audio callback.** No allocation, lock or syscall; the counting-allocator test covers the callback's
   whole body with a negative control; the ring is created before the stream; the error callback cannot panic
   across FFI; device loss stops the command and exits 1.
3. **Timing.** The silent pre-roll (no event late for any score, with its negative control); the law stepped on
   the clock of what is heard in a jam; delivery lag measured and reported; what happens when the output's
   latency exceeds the 100 ms allowance.
4. **The render proof.** The test that finds each moved note in the rendered audio exactly +1,440, +2,160,
   +2,880, −2,160 and 0 samples from its score note: does it measure the audio or re-read the schedule?
5. **The keyboard fallback.** It reads `CONIN$` in a console with no window; can it take keystrokes from the
   owner's desktop?
6. **CI and licences.** The ubuntu job builds and tests the host (with `libasound2-dev`); the Windows workflow is
   paths-gated (including the workspace `Cargo.toml`), has concurrency, `workflow_dispatch` and a timeout, and
   pins actions by SHA; `deny.toml` is unchanged.
7. Name anything that could make a live note sound late, be graded against the wrong clock, or be lost.

## Output

```
PACKET H2 — VERDICT: approve | approve with fixes | reject
FINDINGS:
- [CRIT|HIGH|MED|LOW] path:line — what is wrong — why it matters — smallest fix
DID NOT CHECK: what you could not verify, and why
```
