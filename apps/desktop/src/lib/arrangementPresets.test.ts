import { describe, expect, it } from "vitest";
import { applyArrangementPreset } from "./arrangementPresets";
import type { Section } from "./types";

const sections: Section[] = [
  { id: "i", label: "Intro", kind: "intro", start: 0 },
  { id: "v1", label: "Verse 1", kind: "verse", start: 10 },
  { id: "c", label: "Chorus", kind: "chorus", start: 30 },
  { id: "v2", label: "Verse 2", kind: "verse", start: 50 },
  { id: "b", label: "Bridge", kind: "bridge", start: 70 },
  { id: "o", label: "Outro", kind: "outro", start: 90 },
];

describe("arrangement presets", () => {
  it("builds Intro-V-C-V-C-B-C-Outro from section kinds", () => {
    const arr = applyArrangementPreset(sections, "standard-worship");
    expect(arr).toEqual(["i", "v1", "c", "v2", "c", "b", "c", "o"]);
  });
});
