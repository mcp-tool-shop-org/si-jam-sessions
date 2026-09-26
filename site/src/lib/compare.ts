// Measures the two Battle Hymn arrangements section by section, from the notes the law committed.
// Used at build time for the comparison table and in the browser for the three.js view, so both show
// the same numbers.

export const RATE = 48_000;

/** `host notes` output: onset and length in samples at 48 kHz, pitch, velocity, track; beats. */
export interface NotesFile {
  piece: string;
  title: string;
  golden: string;
  end: number;
  notes: number[][];
  beats: number[][];
}

/** `tools/site/sections.py` output: where each section starts, in the law's samples. */
export interface SectionsFile {
  sections: string[];
  pieces: Record<string, { starts: number[]; end: number; golden: string; ly_sha256: string }>;
}

export interface Stats {
  seconds: number;
  notes: number;
  perSecond: number;
  low: number;
  high: number;
  median: number;
  velocity: number;
  poly: number;
}

const NAMES = ['C', 'D♭', 'D', 'E♭', 'E', 'F', 'G♭', 'G', 'A♭', 'A', 'B♭', 'B'];

/** A MIDI pitch as a note name, with middle C as C4. */
export const noteName = (p: number): string => `${NAMES[p % 12]}${Math.floor(p / 12) - 1}`;

/** Each section's first and last-plus-one sample in one arrangement. */
export function bounds(sec: SectionsFile, piece: string): [number, number][] {
  const { starts, end } = sec.pieces[piece];
  return starts.map((s, i) => [s, i + 1 < starts.length ? starts[i + 1] : end]);
}

/** Length, density, range, loudness and texture of each section. */
export function stats(file: NotesFile, sec: SectionsFile): Stats[] {
  return bounds(sec, file.piece).map(([a, b]) => {
    const ns = file.notes.filter((n) => n[0] >= a && n[0] < b);
    const pitches = ns.map((n) => n[2]).sort((x, y) => x - y);
    // The most notes sounding at once: a sweep in which a note that ends where another starts has ended.
    const events: [number, number][] = [];
    for (const n of ns) events.push([n[0], 1], [n[0] + n[1], -1]);
    events.sort((x, y) => x[0] - y[0] || x[1] - y[1]);
    let sounding = 0;
    let poly = 0;
    for (const [, d] of events) {
      sounding += d;
      poly = Math.max(poly, sounding);
    }
    const seconds = (b - a) / RATE;
    return {
      seconds,
      notes: ns.length,
      perSecond: ns.length / seconds,
      low: pitches[0],
      high: pitches[pitches.length - 1],
      median: pitches[Math.floor(pitches.length / 2)],
      velocity: ns.reduce((s, n) => s + n[3], 0) / ns.length,
      poly,
    };
  });
}

export interface Difference {
  /** 0 when the sections have the same shape, 1 at or past a large difference. */
  score: number;
  text: string;
}

/**
 * The largest of five differences between the same section in two arrangements, in words. The scale only
 * picks which difference to state; the numbers stated are exact.
 */
export function difference(a: Stats, b: Stats, an: string, bn: string): Difference {
  const pick = <T,>(bigger: boolean, x: T, y: T): [string, T, T] => (bigger ? [an, x, y] : [bn, y, x]);
  const candidates = [
    {
      score: Math.abs(Math.log2(a.perSecond / b.perSecond)),
      text: () => {
        const [m, l, r] = pick(a.perSecond > b.perSecond, a, b);
        return `${m} plays ${(l.perSecond / r.perSecond).toFixed(1)}× as many notes a second`;
      },
    },
    {
      score: Math.abs(a.median - b.median) / 12,
      text: () => {
        const [m, l, r] = pick(a.median > b.median, a, b);
        return `${m}'s median note sits ${l.median - r.median} semitones higher`;
      },
    },
    {
      score: Math.abs(a.velocity - b.velocity) / 24,
      text: () => {
        const [m, l, r] = pick(a.velocity > b.velocity, a, b);
        return `${m} plays louder: average velocity ${Math.round(l.velocity)} against ${Math.round(r.velocity)}`;
      },
    },
    {
      score: Math.abs(Math.log2(a.seconds / b.seconds)) * 2,
      text: () => {
        const [m, l, r] = pick(a.seconds > b.seconds, a, b);
        return `${m} takes ${(l.seconds - r.seconds).toFixed(1)} s longer`;
      },
    },
    {
      score: Math.abs(a.poly - b.poly) / 4,
      text: () => {
        const [m, l, r] = pick(a.poly > b.poly, a, b);
        return `${m} stacks up to ${l.poly} notes at once, against ${r.poly}`;
      },
    },
  ];
  const best = candidates.reduce((x, y) => (y.score > x.score ? y : x));
  if (best.score < 0.1) return { score: best.score, text: 'Close in length, density, register, loudness and texture' };
  return { score: Math.min(1, best.score), text: best.text() };
}

/**
 * A shared time axis on which the same section of each arrangement has the same width: the mean of their
 * lengths, in seconds. `at` maps an arrangement's sample to the axis, linearly within its section.
 */
export function alignment(sec: SectionsFile, ids: string[]) {
  const all = ids.map((id) => bounds(sec, id));
  const widths = sec.sections.map((_, i) => all.reduce((s, bs) => s + (bs[i][1] - bs[i][0]) / RATE, 0) / ids.length);
  const x0: number[] = [];
  widths.forEach((_, i) => x0.push(i === 0 ? 0 : x0[i - 1] + widths[i - 1]));
  const total = x0[x0.length - 1] + widths[widths.length - 1];
  const at = (id: string, sample: number): number => {
    const bs = all[ids.indexOf(id)];
    let i = bs.findIndex(([, b]) => sample < b);
    if (i < 0) i = bs.length - 1;
    const [a, b] = bs[i];
    return x0[i] + Math.min(1, Math.max(0, (sample - a) / (b - a))) * widths[i];
  };
  const section = (x: number): number => {
    const i = x0.findIndex((s, k) => x < s + widths[k]);
    return i < 0 ? x0.length - 1 : i;
  };
  return { widths, x0, total, at, section };
}
