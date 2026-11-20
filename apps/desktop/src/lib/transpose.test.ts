import { describe, expect, it } from "vitest";
import { needsKeyRender, semitoneLabel, transposedKey } from "./transpose";

describe("transpose key names", () => {
  it("E steps down and up like a musician reads it", () => {
    expect([-2, -1, 0, 1, 2].map((s) => transposedKey("E", s))).toEqual(["D", "D#", "E", "F", "F#"]);
  });
  it("keeps flat spelling for flat keys and handles minor", () => {
    expect(transposedKey("Bb", 1)).toBe("B");
    expect(transposedKey("Eb", -1)).toBe("D");
    expect(transposedKey("Ab", 2)).toBe("Bb");
    expect(transposedKey("Am", 2)).toBe("Bm");
    expect(transposedKey("C", -4)).toBe("G#");
    expect(transposedKey(null, 2)).toBe("+2");
    expect(semitoneLabel(-3)).toBe("−3");
  });
  it("knows when a chosen key still has to be rendered", () => {
    expect(needsKeyRender({ transpose: 2, transposed: [], stems: { other: "stems/other.wav" } })).toBe(true);
    expect(needsKeyRender({ transpose: 2, transposed: [2], stems: { other: "stems/other.wav" } })).toBe(false);
    expect(needsKeyRender({ transpose: 0, transposed: [], stems: { other: "stems/other.wav" } })).toBe(false);
    expect(needsKeyRender({ transpose: 1, transposed: [], stems: { other: null } })).toBe(false);
  });
});
