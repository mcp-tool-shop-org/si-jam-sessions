"""Where each section of each Battle Hymn exemplar begins, for the landing page's comparison view.

Both arrangements follow one form: an introduction, five passes of verse and chorus, and a coda. This script
compiles a copy of each exemplar's LilyPond file with a MIDI program change set where each section begins,
reads the ticks of those changes from the upper staff's track, and converts each tick to the law's sample time
through the MIDI file's tempo map, checked first against every beat in `host notes`' export. A start that does not
land on a note onset the law committed stops the script.

    py -3 tools/site/sections.py LILYPOND SCORES_DIR NOTES_DIR OUT_JSON

LILYPOND is LilyPond 2.24.4's executable, SCORES_DIR the repository's scores/, NOTES_DIR the directory holding
battle-hymn-<model>.notes.json from `host notes`, and OUT_JSON the file to write.
"""
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile

SECTIONS = ["Introduction"] + [f"{p} {n}" for n in range(1, 6) for p in ("Verse", "Chorus")] + ["Coda"]
PIECES = ["battle-hymn-glm-5.3", "battle-hymn-kimi-k3"]
MARKS = ['#"bright acoustic"', '#"acoustic grand"']


def marked(ly: str) -> str:
    """The file with a program change before each section of the upper staff."""
    out, n, upper = [], 0, False
    for line in ly.splitlines():
        # glm-5.3 lists one variable per section in the score's upper staff.
        if re.match(r'\s*\\new Staff = "upper"', line) or re.match(r"\s*upper = \{", line):
            upper = True
        elif re.match(r'\s*\\new Staff = "lower"', line) or re.match(r"\s*lower = \{", line):
            upper = False
        glm = upper and re.match(r"\s*\\(intro|verse\w+|chorus\w+|coda)Upper\s*$", line)
        # kimi-k3 writes both staves whole, with a comment where each section starts.
        kimi = upper and re.match(r"\s*% (Introduction|Verse \d|Chorus \d|Coda)\b", line)
        if glm:
            out.append(f"\\set Staff.midiInstrument = {MARKS[n % 2]}")
            n += 1
        out.append(line)
        if kimi:
            out.append(f"\\set Staff.midiInstrument = {MARKS[n % 2]}")
            n += 1
    if n != len(SECTIONS):
        raise SystemExit(f"STOP: found {n} section starts, not {len(SECTIONS)}")
    return "\n".join(out) + "\n"


def varlen(b: bytes, i: int) -> tuple[int, int]:
    v = 0
    while True:
        c = b[i]
        i += 1
        v = (v << 7) | (c & 0x7F)
        if not c & 0x80:
            return v, i


def program_change_ticks(mid: bytes) -> tuple[int, list[list[int]], list[tuple[int, int]]]:
    """The file's division; for each track, the distinct ticks of its program changes; and every tempo
    change as (tick, microseconds per quarter note)."""
    if mid[:4] != b"MThd":
        raise SystemExit("STOP: not a MIDI file")
    ntrks, division = int.from_bytes(mid[10:12], "big"), int.from_bytes(mid[12:14], "big")
    i, tracks, tempos = 14, [], []
    for _ in range(ntrks):
        if mid[i : i + 4] != b"MTrk":
            raise SystemExit("STOP: expected a track")
        end = i + 8 + int.from_bytes(mid[i + 4 : i + 8], "big")
        i, tick, status, ticks = i + 8, 0, 0, []
        while i < end:
            delta, i = varlen(mid, i)
            tick += delta
            if mid[i] & 0x80:
                status, i = mid[i], i + 1
            if status == 0xFF:
                kind = mid[i]
                n, i = varlen(mid, i + 1)
                if kind == 0x51:
                    tempos.append((tick, int.from_bytes(mid[i : i + 3], "big")))
                i += n
            elif status in (0xF0, 0xF7):
                n, i = varlen(mid, i)
                i += n
            else:
                if status & 0xF0 == 0xC0 and tick not in ticks:
                    ticks.append(tick)
                i += 1 if status & 0xF0 in (0xC0, 0xD0) else 2
        tracks.append(ticks)
    return division, tracks, sorted(tempos)


def samples_at(tick: int, division: int, tempos: list[tuple[int, int]]) -> float:
    """The time of a tick in samples at 48 kHz, through the tempo map (120 per minute until the first)."""
    t, last, tempo = 0.0, 0, 500_000
    for at, us in tempos:
        if at >= tick:
            break
        t += (at - last) / division * tempo
        last, tempo = at, us
    t += (tick - last) / division * tempo
    return t * 48_000 / 1_000_000


def snap(approx: float, onsets: list[int], what: str) -> int:
    """The onset the law committed within two samples of the one computed here, or a stop."""
    near = [o for o in onsets if abs(o - approx) <= 2]
    if len(near) != 1:
        raise SystemExit(f"STOP: {what} starts near {approx:.1f}, where the law committed no single onset")
    return near[0]


def main() -> None:
    lilypond, scores, notes_dir, out = sys.argv[1], pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3]), sys.argv[4]
    result = {"format": "si-jam-sessions sections 1", "sections": SECTIONS, "pieces": {}}
    for piece in PIECES:
        ly_path = scores / piece / f"{piece}.ly"
        ly = ly_path.read_text(encoding="utf-8")
        notes = json.loads((notes_dir / f"{piece}.notes.json").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as tmp:
            src = pathlib.Path(tmp) / "marked.ly"
            src.write_text(marked(ly), encoding="utf-8")
            subprocess.run([lilypond, "--loglevel=ERROR", "-o", str(pathlib.Path(tmp) / "marked"), str(src)],
                           check=True)
            division, tracks, tempos = program_change_ticks((pathlib.Path(tmp) / "marked.mid").read_bytes())
        ticks = max(tracks, key=len)
        if len(ticks) != len(SECTIONS):
            raise SystemExit(f"STOP: {piece}: {len(ticks)} program changes in the upper track")
        onsets = sorted({n[0] for n in notes["notes"]})
        # The tempo map must reproduce the law's beats before it is trusted with the sections.
        for i, beat in enumerate(notes["beats"]):
            snap(samples_at(i * division, division, tempos), [beat[0]], f"{piece}: beat {i}")
        starts = [snap(samples_at(t, division, tempos), onsets, f"{piece}: {name}")
                  for name, t in zip(SECTIONS, ticks)]
        result["pieces"][piece] = {
            "ly_sha256": hashlib.sha256(ly_path.read_bytes()).hexdigest(),
            "golden": notes["golden"],
            "starts": starts,
            "end": notes["end"],
        }
        print(piece, [round(s / 48000, 2) for s in starts])
    pathlib.Path(out).write_text(json.dumps(result, separators=(",", ":")) + "\n", encoding="utf-8")
    print("wrote", out)


if __name__ == "__main__":
    main()
