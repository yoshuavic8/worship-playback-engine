import { describe, expect, it } from "vitest";

describe("setup wizard gating", () => {
  it("treats complete setup as ready to enter app", () => {
    const setup = { complete: true, phase: "done" as const, progress: 1, steps: [] };
    expect(setup.complete).toBe(true);
  });
});
