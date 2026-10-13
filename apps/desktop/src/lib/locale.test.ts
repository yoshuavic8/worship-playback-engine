import { describe, expect, it } from "vitest";
import { fmtDate, fmtDateShort } from "./format";
import { setUiLocale } from "./locale";

describe("locale formatting", () => {
  it("formats dates in English by default", () => {
    setUiLocale("en");
    expect(fmtDate("2026-10-16")).toMatch(/Oct/);
    expect(fmtDateShort("2026-10-16")).toMatch(/16/);
  });

  it("formats dates in Indonesian when selected", () => {
    setUiLocale("id");
    const long = fmtDate("2026-10-16");
    expect(long.toLowerCase()).toMatch(/okt/);
    expect(fmtDateShort("2026-10-16").toLowerCase()).toMatch(/okt/);
    setUiLocale("en");
  });
});
