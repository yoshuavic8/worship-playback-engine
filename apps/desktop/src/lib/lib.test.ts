import { describe, expect, it } from "vitest";
import { fmtBytes, fmtDb, fmtTime, meterLevel } from "./format";
import { PROFILES, channelMap, decodeTarget, encodeTarget, isLiveSafe, validateRouting } from "./routing";
import { buildPlan, kindFromLabel, nextLabel, resolveSections, sectionShort } from "./sections";
import { keyLabel, matchShortcut, DEFAULT_SHORTCUTS } from "./shortcuts";
import { parseTime, tapBpm } from "@/pages/SongEditor";
import type { RoutingProfile, Section } from "./types";

describe("format", () => {
  it("formats times", () => {
    expect(fmtTime(0)).toBe("0:00");
    expect(fmtTime(83.46)).toBe("1:23");
    expect(fmtTime(83.46, true)).toBe("1:23.4");
    expect(fmtTime(-3)).toBe("0:00");
    expect(fmtTime(NaN)).toBe("0:00");
  });
  it("formats bytes and dB", () => {
    expect(fmtBytes(0)).toBe("0 B");
    expect(fmtBytes(84_141_911)).toBe("80.2 MB");
    expect(fmtDb(-6)).toBe("−6 dB");
    expect(fmtDb(0)).toBe("0 dB");
    expect(fmtDb(-80)).toBe("−∞");
  });
  it("maps meter peaks to a dB scale", () => {
    expect(meterLevel(0)).toBe(0);
    expect(meterLevel(1)).toBe(1);
    expect(meterLevel(0.5)).toBeCloseTo(0.9, 1);
    expect(meterLevel(0.001)).toBe(0);
  });
});

describe("routing safety (mirrors Rust)", () => {
  it("accepts the built-in profiles on suitable devices", () => {
    expect(isLiveSafe(PROFILES.split, 2)).toBe(true);
    expect(isLiveSafe(PROFILES.multi, 4)).toBe(true);
    expect(isLiveSafe(PROFILES.rehearsal, 2)).toBe(true);
  });
  it("rejects outputs the device does not have", () => {
    expect(isLiveSafe(PROFILES.multi, 2)).toBe(false);
  });
  it("never allows FOH and Monitor to share a channel (click would leak)", () => {
    const bad: RoutingProfile = { id: "x", name: "x", mode: "custom", foh: { kind: "stereo", left: 0, right: 1 }, monitor: { kind: "mono", channel: 1 } };
    expect(isLiveSafe(bad, 4)).toBe(false);
    expect(validateRouting(bad, 4).some((i) => i.message.includes("share"))).toBe(true);
    expect(channelMap(bad, 4)).toEqual(["foh", "conflict", null, null]);
  });
  it("round-trips target encodings", () => {
    for (const t of [PROFILES.multi.foh, PROFILES.split.monitor, PROFILES.rehearsal.foh]) {
      expect(decodeTarget(encodeTarget(t))).toEqual(t);
    }
  });
});

describe("sections & arrangement", () => {
  const secs: Section[] = [
    { id: "c", label: "Chorus", kind: "chorus", start: 30 },
    { id: "i", label: "Intro", kind: "intro", start: 2 },
    { id: "v", label: "Verse 1", kind: "verse", start: 10 },
  ];
  it("resolves sorted sections with ends and forces the first to 0", () => {
    const r = resolveSections({ sections: secs, duration: 60 });
    expect(r.map((s) => [s.id, s.start, s.end])).toEqual([["i", 0, 10], ["v", 10, 30], ["c", 30, 60]]);
  });
  it("builds a custom plan with repeats", () => {
    const plan = buildPlan({ sections: secs, duration: 60, arrangement: ["i", "v", "c", "c"] });
    expect(plan.map((p) => p.label)).toEqual(["Intro", "Verse 1", "Chorus", "Chorus"]);
  });
  it("falls back to natural order and to one segment without sections", () => {
    expect(buildPlan({ sections: secs, duration: 60, arrangement: [] }).length).toBe(3);
    expect(buildPlan({ sections: [], duration: 60, arrangement: [] })).toEqual([{ sectionId: "__song", label: "Song", kind: "other", start: 0, end: 60 }]);
  });
  it("understands English and Indonesian labels", () => {
    expect(kindFromLabel("Pre Chorus 2")).toBe("prechorus");
    expect(kindFromLabel("Reff")).toBe("chorus");
    expect(kindFromLabel("Bait 2")).toBe("verse");
    expect(kindFromLabel("Interlude")).toBe("instrumental");
    expect(sectionShort("Verse 2")).toBe("V2");
  });
  it("numbers new verses", () => {
    expect(nextLabel(secs, "verse")).toBe("Verse 2");
    expect(nextLabel(secs, "chorus")).toBe("Chorus");
    expect(nextLabel([], "bridge")).toBe("Bridge 1");
  });
});

describe("shortcuts", () => {
  it("matches configured key codes and ignores modifier combos", () => {
    const ev = (code: string, extra: Partial<KeyboardEvent> = {}) => ({ code, metaKey: false, ctrlKey: false, altKey: false, ...extra }) as KeyboardEvent;
    expect(matchShortcut(ev("Space"), DEFAULT_SHORTCUTS)).toBe("playPause");
    expect(matchShortcut(ev("Escape"), DEFAULT_SHORTCUTS)).toBe("panic");
    expect(matchShortcut(ev("KeyS", { metaKey: true }), DEFAULT_SHORTCUTS)).toBeNull();
    expect(keyLabel("KeyN")).toBe("N");
    expect(keyLabel("ArrowRight")).toBe("→");
  });
});

describe("editor helpers", () => {
  it("parses times", () => {
    expect(parseTime("1:23.4")).toBeCloseTo(83.4);
    expect(parseTime("83,5")).toBeCloseTo(83.5);
    expect(parseTime("abc")).toBeNull();
  });
  it("computes tap tempo", () => {
    expect(tapBpm([0, 500, 1000])).toBeNull();
    expect(tapBpm([0, 500, 1000, 1500, 2000])).toBe(120);
  });
});
