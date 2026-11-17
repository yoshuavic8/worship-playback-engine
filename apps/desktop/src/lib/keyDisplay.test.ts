import { describe, expect, it } from "vitest";
import { formatKeyDisplay, keyFromPitchClass, pitchClassFromKey } from "./keyDisplay";
import { setUiLocale } from "./locale";

describe("key display", () => {
  it("maps G major to pitch class 7", () => {
    expect(pitchClassFromKey("G")).toBe(7);
    expect(keyFromPitchClass(7, "major")).toBe("G");
  });

  it("shows Indonesian solfège label", () => {
    setUiLocale("id");
    expect(formatKeyDisplay("G")).toBe("Nada dasar: sol = G");
    setUiLocale("en");
    expect(formatKeyDisplay("G")).toBe("G");
  });
});
