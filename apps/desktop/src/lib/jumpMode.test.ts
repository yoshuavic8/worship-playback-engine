import { describe, expect, it } from "vitest";
import { nextJumpMode } from "./transport";

describe("section jump click rule", () => {
  it("first press queues end of part", () => {
    expect(nextJumpMode(2, { index: null, bar: false }, null)).toBe("section");
    expect(nextJumpMode(2, { index: 1, bar: true }, null)).toBe("section");
  });
  it("second press on same section switches at end of bar", () => {
    expect(nextJumpMode(2, { index: 2, bar: false }, null)).toBe("bar");
    expect(nextJumpMode(2, { index: null, bar: false }, { index: 2, bar: false })).toBe("bar");
  });
  it("third press cancels", () => {
    expect(nextJumpMode(2, { index: 2, bar: true }, null)).toBe("cancel");
  });
});

import { vampDecision } from "./transport";

describe("vamp tap or hold", () => {
  it("tap toggles", () => {
    expect(vampDecision(false, 120).onRelease).toBe(true);
    expect(vampDecision(true, 120).onRelease).toBe(false);
  });
  it("hold loops only while held", () => {
    expect(vampDecision(false, 900).onRelease).toBe(false);
    expect(vampDecision(true, 900).onRelease).toBe(false);
  });
});
