import { describe, expect, it } from "vitest";
import { modelOptions, normalizeOpenAiBase } from "./aiProvider";

describe("FreeLLMAPI base URL", () => {
  it("appends /v1 once and never doubles it", () => {
    expect(normalizeOpenAiBase("http://localhost:31415")).toBe("http://localhost:31415/v1");
    expect(normalizeOpenAiBase("http://localhost:31415/v1")).toBe("http://localhost:31415/v1");
    expect(normalizeOpenAiBase("http://localhost:31415/v1/")).toBe("http://localhost:31415/v1");
    expect(normalizeOpenAiBase("localhost:31415")).toBe("http://localhost:31415/v1");
    expect(normalizeOpenAiBase("http://localhost:31415/v1/chat/completions")).toBe("http://localhost:31415/v1");
    expect(normalizeOpenAiBase("")).toBe("http://localhost:31415/v1");
  });
  it("offers router aliases first", () => {
    expect(modelOptions(["gpt-oss-120b", "auto"], "my-model")).toEqual(["auto", "auto:fast", "auto:smart", "gpt-oss-120b", "my-model"]);
  });
});
