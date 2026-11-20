import { describe, expect, it } from "vitest";
import { isKeyMapped, mappingFor, sourceLabel, targetKey, targetLabel } from "./midi";
import type { MidiMapping, MidiTarget } from "./types";

const m = (id: string, target: MidiTarget, songId: string | null, key?: string): MidiMapping => ({
  id, songId, target, mode: "trigger", encoder: "absolute", min: 0, max: 1, createdAt: "",
  source: key ? { kind: "key", channel: 0, number: 0, key } : { kind: "note", channel: 9, number: 36 },
});

describe("midi mapping helpers", () => {
  it("labels sources the way a musician reads them", () => {
    expect(sourceLabel({ kind: "note", channel: 9, number: 36 })).toBe("Note C2 · ch 10");
    expect(sourceLabel({ kind: "cc", channel: 0, number: 112 })).toBe("CC 112 · ch 1");
    expect(sourceLabel({ kind: "pc", channel: 0, number: 0 })).toBe("PC 1 · ch 1");
    expect(sourceLabel({ kind: "key", channel: 0, number: 0, key: "KeyQ" })).toBe("Key Q");
  });

  it("shows this song's mapping before the global one", () => {
    const chorus: MidiTarget = { type: "songSection", sectionId: "c1", label: "Chorus" };
    const maps = [m("g", { type: "trackMute", role: "drums" }, null), m("s", chorus, "song-a")];
    expect(mappingFor(maps, chorus, "song-a")?.id).toBe("s");
    expect(mappingFor(maps, chorus, "song-b")).toBeUndefined();
    expect(mappingFor(maps, { type: "trackMute", role: "drums" }, "song-b")?.id).toBe("g");
    expect(targetKey({ type: "trackSolo", role: "bass" })).toBe("trackSolo:bass");
    expect(targetLabel({ type: "sectionSlot", slot: 3 })).toBe("Section 3 (any song)");
  });

  it("forwards only mapped keys (no hardcoded 1–9)", () => {
    const maps = [m("k", { type: "stop" }, null, "KeyQ"), m("s", { type: "sectionSlot", slot: 1 }, "song-a", "Digit1")];
    expect(isKeyMapped(maps, "KeyQ", null)).toBe(true);
    expect(isKeyMapped(maps, "Digit1", "song-a")).toBe(true);
    expect(isKeyMapped(maps, "Digit1", "song-b")).toBe(false);
    expect(isKeyMapped([], "Digit2", "song-a")).toBe(false);
  });
});
