/** Tempo map. The same formulas run in Rust (`audio/tempo_map.rs`) and Python.

Time on the timeline is linear. A slower tempo makes bars wider. The audio
file is never time-stretched. One step point is a constant tempo.
*/

export type Curve = "step" | "linear";

export interface TempoPoint {
  id: string;
  bar: number;
  beat: number;
  bpm: number;
  curve: Curve;
}

export interface Grid {
  beat1Sec: number;
  beatsPerBar: number;
  points: TempoPoint[];
  source: "analysis" | "user";
  confidence?: number | null;
  version: 1;
}

export interface BeatTick {
  t: number;
  q: number;
  bar: number;
  beat: number;
  accent: boolean;
  bpm: number;
}

interface Knot {
  q: number;
  t: number;
  bpm: number;
  curve: Curve;
}

const FLAT = 1e-9;

export function qOf(bar: number, beat: number, beatsPerBar: number): number {
  const bpb = Math.max(1, beatsPerBar | 0);
  return (bar - 1) * bpb + (beat - 1);
}

export function barBeatOf(q: number, beatsPerBar: number): { bar: number; beat: number } {
  const bpb = Math.max(1, beatsPerBar | 0);
  let barIndex = Math.floor(q / bpb);
  let beat0 = Math.round(q - barIndex * bpb);
  if (beat0 >= bpb) {
    beat0 = 0;
    barIndex += 1;
  }
  if (beat0 < 0) {
    beat0 += bpb;
    barIndex -= 1;
  }
  return { bar: barIndex + 1, beat: beat0 + 1 };
}

function knotsOf(grid: Grid): Knot[] {
  const bpb = Math.max(1, grid.beatsPerBar | 0);
  const raw = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, bpb) - qOf(b.bar, b.beat, bpb));
  const points = raw.length ? raw : [{ id: "p1", bar: 1, beat: 1, bpm: 120, curve: "step" as Curve }];
  const knots: Knot[] = [];
  let t = grid.beat1Sec || 0;
  points.forEach((p, i) => {
    const q = i === 0 ? 0 : qOf(p.bar, p.beat, bpb);
    const bpm = Math.max(1e-6, p.bpm);
    const curve: Curve = p.curve === "linear" ? "linear" : "step";
    if (i > 0) {
      const prev = knots[i - 1];
      t = timeAcross(prev.t, prev.q, prev.bpm, q, bpm, prev.curve);
    }
    knots.push({ q, t, bpm, curve });
  });
  return knots;
}

function timeAcross(t0: number, q0: number, v0: number, q1: number, v1: number, curve: Curve): number {
  const span = q1 - q0;
  if (span <= 0) return t0;
  if (curve !== "linear" || Math.abs(v1 - v0) < FLAT) return t0 + (60 * span) / v0;
  return t0 + ((60 * span) / (v1 - v0)) * Math.log(v1 / v0);
}

function segment(knots: Knot[], q: number): { i: number; past: boolean } {
  if (q <= knots[0].q) return { i: 0, past: false };
  for (let i = 0; i < knots.length - 1; i++) {
    if (knots[i].q <= q && q < knots[i + 1].q) return { i, past: false };
  }
  return { i: knots.length - 1, past: true };
}

export function bpmAtBeat(grid: Grid, q: number): number {
  const knots = knotsOf(grid);
  const { i, past } = segment(knots, q);
  const v0 = knots[i].bpm;
  if (past || i + 1 >= knots.length || knots[i].curve !== "linear") return v0;
  const v1 = knots[i + 1].bpm;
  const span = knots[i + 1].q - knots[i].q;
  if (span <= 0 || Math.abs(v1 - v0) < FLAT) return v0;
  return v0 + ((v1 - v0) * (q - knots[i].q)) / span;
}

export function beatToTime(grid: Grid, q: number): number {
  const knots = knotsOf(grid);
  const { i, past } = segment(knots, q);
  const { q: q0, t: t0, bpm: v0 } = knots[i];
  if (past || i + 1 >= knots.length || q <= q0) return t0 + (60 * (q - q0)) / v0;
  const v1 = knots[i + 1].bpm;
  const span = knots[i + 1].q - q0;
  if (knots[i].curve !== "linear" || Math.abs(v1 - v0) < FLAT || span <= 0) return t0 + (60 * (q - q0)) / v0;
  const bpm = v0 + ((v1 - v0) * (q - q0)) / span;
  if (bpm <= 0 || v0 <= 0) return t0 + (60 * (q - q0)) / v0;
  return t0 + ((60 * span) / (v1 - v0)) * Math.log(bpm / v0);
}

export function timeToBeat(grid: Grid, t: number): number {
  const knots = knotsOf(grid);
  if (t <= knots[0].t) return knots[0].q + ((t - knots[0].t) * knots[0].bpm) / 60;
  for (let i = 0; i < knots.length - 1; i++) {
    const t0 = knots[i].t;
    const t1 = knots[i + 1].t;
    if (t0 <= t && (t < t1 || (i === knots.length - 2 && t <= t1))) {
      const q0 = knots[i].q;
      const q1 = knots[i + 1].q;
      const v0 = knots[i].bpm;
      const v1 = knots[i + 1].bpm;
      const span = q1 - q0;
      if (t >= t1) return q1;
      if (knots[i].curve !== "linear" || Math.abs(v1 - v0) < FLAT || span <= 0) return q0 + ((t - t0) * v0) / 60;
      const bpm = v0 * Math.exp(((t - t0) * (v1 - v0)) / (60 * span));
      return q0 + (span * (bpm - v0)) / (v1 - v0);
    }
  }
  const last = knots[knots.length - 1];
  return last.q + ((t - last.t) * last.bpm) / 60;
}

export function bpmAtTime(grid: Grid, t: number): number {
  return bpmAtBeat(grid, timeToBeat(grid, t));
}

/** Curve of the segment that contains this beat (the point you are leaving). */
export function curveAtBeat(grid: Grid, q: number): Curve {
  const knots = knotsOf(grid);
  return knots[segment(knots, q).i].curve;
}

export function isAccent(q: number, beatsPerBar: number): boolean {
  const bpb = Math.max(1, beatsPerBar | 0);
  const beat0 = ((Math.round(q) % bpb) + bpb) % bpb;
  return beat0 === 0 || (bpb === 6 && beat0 === 3);
}

export function beatsInRange(grid: Grid, fromSec: number, toSec: number, limit = 4000): BeatTick[] {
  const q0 = Math.floor(timeToBeat(grid, fromSec));
  const q1 = Math.ceil(timeToBeat(grid, toSec));
  const out: BeatTick[] = [];
  for (let q = q0; q <= q1; q++) {
    const t = beatToTime(grid, q);
    if (t < fromSec - 1e-4 || t > toSec + 1e-4) continue;
    const { bar, beat } = barBeatOf(q, grid.beatsPerBar);
    out.push({ t, q, bar, beat, accent: isAccent(q, grid.beatsPerBar), bpm: bpmAtBeat(grid, q) });
    if (out.length >= limit) break;
  }
  return out;
}

/** Relabel points for a new meter without moving them in time. Beat index q is unchanged. */
export function remapBeatsPerBar(grid: Grid, beatsPerBar: number): Grid {
  const bpb = Math.max(1, Math.round(beatsPerBar));
  const points = sorted(grid).map((p, i) => {
    if (i === 0) return { ...p, bar: 1, beat: 1 };
    const pos = barBeatOf(qOf(p.bar, p.beat, grid.beatsPerBar), bpb);
    return { ...p, bar: pos.bar, beat: pos.beat };
  });
  return { ...grid, beatsPerBar: bpb, points, source: "user" };
}

export function constantGrid(bpm: number, beat1Sec: number, beatsPerBar: number, source: Grid["source"] = "analysis"): Grid {
  return {
    beat1Sec,
    beatsPerBar: Math.max(1, beatsPerBar | 0),
    points: [{ id: "p1", bar: 1, beat: 1, bpm: round3(bpm), curve: "step" }],
    source,
    version: 1,
  };
}

/** The song's map, or one step point built from its BPM and Beat 1. */
export function ensureGrid(song: { bpm: number | null; beatOffset: number; timeSignature: string; grid?: Grid | null }): Grid | null {
  const bpb = beatsFromSignature(song.timeSignature);
  const stored = song.grid;
  if (stored && stored.points && stored.points.length > 0 && stored.points.some((p) => p.bpm >= 20 && p.bpm <= 300)) {
    return { ...stored, beatsPerBar: stored.beatsPerBar || bpb, version: 1, points: stored.points.map((p) => ({ ...p, curve: p.curve === "linear" ? "linear" : "step" })) };
  }
  if (song.bpm == null || song.bpm < 20 || song.bpm > 300) return null;
  return constantGrid(song.bpm, song.beatOffset || 0, bpb);
}

function beatsFromSignature(ts: string): number {
  const n = Number(String(ts).split("/")[0]);
  return Number.isFinite(n) && n > 0 ? Math.round(n) : 4;
}

function round3(n: number): number {
  return Math.round(n * 1000) / 1000;
}

function newId(): string {
  return `p${Math.random().toString(36).slice(2, 8)}`;
}

function sorted(grid: Grid): TempoPoint[] {
  const bpb = grid.beatsPerBar;
  return [...grid.points].sort((a, b) => qOf(a.bar, a.beat, bpb) - qOf(b.bar, b.beat, bpb));
}

/** Double-click: a point on the beat whose BPM matches the curve, so the line does not jump. */
export function addPoint(grid: Grid, time: number, snap: "bar" | "beat" | "free"): Grid | null {
  const bpb = grid.beatsPerBar;
  let q = timeToBeat(grid, time);
  if (snap === "bar") q = Math.round(q / bpb) * bpb;
  else if (snap === "beat") q = Math.round(q);
  else q = Math.round(q * 16) / 16;
  const points = sorted(grid);
  const qs = points.map((p, i) => (i === 0 ? 0 : qOf(p.bar, p.beat, bpb)));
  if (qs.some((n) => Math.abs(n - q) < 0.9)) return null;
  const { bar, beat } = barBeatOf(q, bpb);
  const bpm = round3(Math.min(300, Math.max(30, bpmAtBeat(grid, q))));
  const curve = curveAtBeat(grid, q);
  const point: TempoPoint = { id: newId(), bar, beat, bpm, curve };
  return { ...grid, source: "user", points: [...points, point] };
}

export function setCurve(grid: Grid, id: string, curve: Curve): Grid {
  return { ...grid, source: "user", points: grid.points.map((p) => (p.id === id ? { ...p, curve } : p)) };
}

/** Point 1 is the tempo at Beat 1. It cannot be removed. */
export function removePoint(grid: Grid, id: string): Grid | null {
  const points = sorted(grid);
  if (!points.length || points[0].id === id) return null;
  return { ...grid, source: "user", points: points.filter((p) => p.id !== id) };
}

export function movePoint(grid: Grid, id: string, next: { bar?: number; beat?: number; bpm?: number }): Grid {
  const bpb = grid.beatsPerBar;
  const points = sorted(grid);
  const i = points.findIndex((p) => p.id === id);
  if (i < 0) return grid;
  const cur = points[i];
  let bpm = next.bpm == null ? cur.bpm : Math.min(300, Math.max(30, next.bpm));
  bpm = round3(bpm);
  if (i === 0) {
    points[i] = { ...cur, bar: 1, beat: 1, bpm };
    return { ...grid, source: "user", points };
  }
  let q = qOf(next.bar ?? cur.bar, next.beat ?? cur.beat, bpb);
  const prevQ = i === 1 ? 0 : qOf(points[i - 1].bar, points[i - 1].beat, bpb);
  const nextQ = i + 1 < points.length ? qOf(points[i + 1].bar, points[i + 1].beat, bpb) : q + bpb * 64;
  q = Math.min(nextQ - 1, Math.max(prevQ + 1, q));
  const pos = barBeatOf(q, bpb);
  points[i] = { ...cur, bar: pos.bar, beat: pos.beat, bpm };
  return { ...grid, source: "user", points };
}

/** Sections that sat on a bar or beat line stay on that line after the map moves. */
export function resnapSections<T extends { start: number }>(sections: T[], prev: Grid | null, next: Grid, duration: number): { sections: T[]; moved: number } {
  if (!prev) return { sections, moved: 0 };
  const bpb = prev.beatsPerBar;
  let moved = 0;
  const out = sections.map((s, index) => {
    if (index === 0) return s;
    const q = timeToBeat(prev, s.start);
    const beatQ = Math.round(q);
    const barQ = Math.round(q / bpb) * bpb;
    const onBar = Math.abs(s.start - beatToTime(prev, barQ)) < 0.02;
    const onBeat = Math.abs(s.start - beatToTime(prev, beatQ)) < 0.02;
    if (!onBar && !onBeat) return s;
    const t = Math.max(0, Math.min(duration || s.start, beatToTime(next, onBar ? barQ : beatQ)));
    if (Math.abs(t - s.start) < 0.0005) return s;
    moved += 1;
    return { ...s, start: Math.round(t * 1000) / 1000 };
  });
  return { sections: out, moved };
}

export function yScale(bpms: number[]): { lo: number; hi: number } {
  const min = Math.min(...bpms);
  const max = Math.max(...bpms);
  let lo = Math.floor((min - 5) / 10) * 10;
  let hi = Math.ceil((max + 5) / 10) * 10;
  if (hi - lo < 10) hi = lo + 10;
  return { lo, hi };
}

export function fmtPoint(bar: number, beat: number, bpm: number): string {
  return `Bar ${bar}.${beat} · ${bpm.toFixed(2)} BPM`;
}
