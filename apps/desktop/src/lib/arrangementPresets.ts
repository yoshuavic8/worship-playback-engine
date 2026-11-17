import type { Section } from "./types";

export type ArrangementPresetId = "standard-worship";

const PRESETS: Record<ArrangementPresetId, { label: string; kinds: string[] }> = {
  "standard-worship": {
    label: "Intro · V · C · V · C · B · C · Outro",
    kinds: ["intro", "verse", "chorus", "verse", "chorus", "bridge", "chorus", "outro"],
  },
};

export function arrangementPresetOptions(): { id: ArrangementPresetId; label: string }[] {
  return (Object.keys(PRESETS) as ArrangementPresetId[]).map((id) => ({ id, label: PRESETS[id].label }));
}

/** Map a kind sequence to section ids (first match per kind slot; reuses ids for repeats). */
export function applyArrangementPreset(sections: Section[], preset: ArrangementPresetId): string[] {
  const kinds = PRESETS[preset].kinds;
  const out: string[] = [];
  for (const kind of kinds) {
    const match = sections.find((s) => s.kind === kind && !out.includes(s.id))
      ?? sections.find((s) => s.kind === kind);
    if (match) out.push(match.id);
  }
  return out;
}
