import { describe, expect, it } from "vitest";
import { visibleReviewWarnings } from "./reviewWarnings";
import type { Song } from "./types";

const base = (): Song => ({
  id: "x", title: "T", artist: "A", bpm: 96, key: "C", timeSignature: "4/4", beatOffset: 0, duration: 100,
  sampleRate: 48000, channels: 2, dir: "/", sourcePath: "a.mp3",
  stems: { vocals: null, drums: null, bass: null, other: null },
  rendered: { backing: null, backup: null },
  status: "warning", prep: { presetId: null, vocals: "remove", drums: "remove", bass: "keep", bassDb: 0, otherDb: 0, click: true, cue: true },
  routing: { backingToFoh: true, backingToMonitor: true, guideVocalToMonitor: false, clickEnabled: true, cueEnabled: true },
  sections: [], arrangement: [], countInBars: 0, cueLeadBeats: 4, notes: "", lastError: null,
  analysis: { warnings: ["BPM 72 detected with low confidence — please verify"], bpmConfidence: 0.94 },
  createdAt: "", updatedAt: "", preparedAt: null,
});

describe("visibleReviewWarnings", () => {
  it("hides tempo warnings that disagree with the current BPM", () => {
    const { active, stale } = visibleReviewWarnings(base());
    expect(active).toHaveLength(0);
    expect(stale).toHaveLength(1);
  });

  it("hides tempo warnings after the user edits the tempo map", () => {
    const s = { ...base(), grid: { beat1Sec: 0, beatsPerBar: 4, source: "user" as const, version: 1 as const, points: [{ id: "p1", bar: 1, beat: 1, bpm: 96, curve: "step" as const }] } };
    const { active, stale } = visibleReviewWarnings(s);
    expect(active).toHaveLength(0);
    expect(stale.length).toBeGreaterThan(0);
  });
});
