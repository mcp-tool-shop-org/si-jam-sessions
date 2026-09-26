# External review brief — round 5, packet H1 (2026-09-26): si-jam-sessions PR #5, the law's frames and live notes

You are the **external verifier**: a different model family from the authors (Claude agents), seeing only the
change and its evidence, never the authors' reasoning. **Read-only**: you cannot run anything; reason from the
text below. Every finding names a file and line (from the patch), what is wrong, why it matters, and the smallest
fix. Say "no findings" when the packet is sound. Do not assume the contents of files you were not given.

**Before you judge, list the cases a hostile reviewer would try** against each check below (window edges, a beat
on a window's last sample, a note on a quantum boundary, ties, chords, repeated notes, a very late or
out-of-order delivery, overflow at the largest sample, an empty take, a proposal into a live take), then check
each case against the code.

## The change

- PR #5, head `1e7f92e`, base `main` at `1a36b36` (law version 3, golden `145c7af9…`). CI green on all 7 jobs,
  including the golden under V8, SpiderMonkey and JavaScriptCore.
- Files that follow: `host-law-1e7f92e.patch` (the whole PR) and `host-law-NOTES-1e7f92e.md` (the authors' facts).
- The design lock this must obey (from `docs/PHASE-0.md`): the law steps a fixed quantum Q = 48 samples; a
  proposal is admissible only past a commit horizon of H = 100 quanta; a committed quantum never changes; live
  input is a record, admitted at its own sample onset and never refused for lateness; training rows are a
  printout of what the law committed; all hashed state is integer; status codes cross the boundary, panics do not.

## Check

1. **law_frames.** Little-endian and versioned encoding; only quanta at or before the committed horizon; every
   event exactly once across consecutive windows, including a beat or note on a window's first and last sample.
2. **The live verb** (note-on and note-off). It admits through the same code as the constructed take and grades
   the same way; it is never refused for lateness; its refusal codes are appended (160–168, 170–173; 166 was
   retired before reaching main) and no existing code is renumbered.
3. **Citation.** Each score note is cited at most once; ties break deterministically (nearest onset, then the
   score note before the live one, then pitch distance, then lower pitch, then lowest id); chords; the reach is
   twice the gate (3,840 samples).
4. **Closing and final rows.** A score note closes when the playhead passes onset + 8,640 samples (the reach plus
   an allowance of H). A closed note is never cited again; a later note cites the next open one or becomes an
   addition; `law_rows` and the snapshot hold only final verdicts, in the order they became final, so the row
   stream is prefix-stable. Is anything a caller can observe still able to change after it is shown?
5. **The three decisions the authors flag** in the NOTES: a take is live if its transport starts with the take
   empty; proposals into a live take are allowed when they cite an open note; a very late note with an earlier
   onset shifts take indices in the snapshot's cross-references while no row text changes. Is each sound?
6. **Law version 4.** The new golden `fd574ccc…`; the claim that the v4 snapshot with byte 12 written back to 3
   hashes to the v3 golden; the snapshot format unchanged; the constants derived from H, Q and the reach.
7. **Guards.** The wasm export list only gains names; the no-imports, no-float and std-binding guards are not
   narrowed.

## Output

```
PACKET H1 — VERDICT: approve | approve with fixes | reject
FINDINGS:
- [CRIT|HIGH|MED|LOW] path:line — what is wrong — why it matters — smallest fix
DID NOT CHECK: what you could not verify, and why
```
