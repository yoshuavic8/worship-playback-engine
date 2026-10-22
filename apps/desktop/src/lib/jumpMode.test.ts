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
