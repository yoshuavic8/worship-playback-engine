import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { beatToTime, bpmAtBeat, qOf, remapBeatsPerBar, timeToBeat, type Curve, type Grid } from "./tempoMap";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const fixture = JSON.parse(readFileSync(resolve(root, "fixtures/tempo_map_vectors.json"), "utf8")) as {
  mockup: Case;
  constant: Case;
};

interface Case {
  beat1Sec: number;
  beatsPerBar: number;
  points: { bar: number; beat: number; bpm: number; curve: Curve }[];
  expect: { bar: number; beat: number; time: number; bpm: number; qNeg?: number }[];
}

function gridOf(c: Case): Grid {
  return {
    beat1Sec: c.beat1Sec,
    beatsPerBar: c.beatsPerBar,
    points: c.points.map((p, i) => ({ id: `p${i}`, ...p })),
    source: "analysis",
    version: 1,
  };
}

describe("tempo map golden vectors", () => {
  for (const [name, c] of Object.entries(fixture)) {
    it(name, () => {
      const g = gridOf(c);
      for (const e of c.expect) {
        const q = e.qNeg ?? qOf(e.bar, e.beat, g.beatsPerBar);
        const t = beatToTime(g, q);
        expect(Math.abs(t - e.time)).toBeLessThan(0.0001);
        expect(Math.abs(bpmAtBeat(g, q) - e.bpm)).toBeLessThan(0.0001);
        expect(Math.abs(timeToBeat(g, t) - q)).toBeLessThan(1e-6);
      }
    });
  }

  it("relabels points when the meter changes and keeps the same times", () => {
    const g: Grid = {
      beat1Sec: 2,
      beatsPerBar: 4,
      source: "user",
      version: 1,
      points: [
        { id: "p1", bar: 1, beat: 1, bpm: 120, curve: "step" },
        { id: "p2", bar: 5, beat: 1, bpm: 90, curve: "step" },
      ],
    };
    const next = remapBeatsPerBar(g, 3);
    const q = qOf(5, 1, 4);
    expect(next.beatsPerBar).toBe(3);
    expect(next.points[0]).toMatchObject({ bar: 1, beat: 1, bpm: 120 });
    expect(next.points[1]).toMatchObject({ bar: 6, beat: 2, bpm: 90 });
    expect(Math.abs(beatToTime(next, q) - beatToTime(g, q))).toBeLessThan(1e-9);
  });

  it("lands bar 45 within one 48 kHz sample of the shared frame", () => {
    const g = gridOf(fixture.mockup);
    const frame = Math.round(beatToTime(g, qOf(45, 1, 4)) * 48000);
    expect(Math.abs(frame - 7187585)).toBeLessThanOrEqual(1);
  });
});
