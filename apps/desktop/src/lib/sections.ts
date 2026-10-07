import type { PlanSegment, Section, SectionKind, Song } from "./types";

export const SECTION_KINDS: { kind: SectionKind; label: string }[] = [
  { kind: "intro", label: "Intro" },
  { kind: "verse", label: "Verse" },
  { kind: "prechorus", label: "Pre-Chorus" },
  { kind: "chorus", label: "Chorus" },
  { kind: "bridge", label: "Bridge" },
  { kind: "instrumental", label: "Instrumental" },
  { kind: "tag", label: "Tag" },
  { kind: "outro", label: "Outro" },
  { kind: "other", label: "Other" },
];

const COLORS: Record<string, string> = {
  intro: "var(--color-sec-intro)",
  verse: "var(--color-sec-verse)",
  prechorus: "var(--color-sec-prechorus)",
  chorus: "var(--color-sec-chorus)",
  bridge: "var(--color-sec-bridge)",
  instrumental: "var(--color-sec-instrumental)",
  interlude: "var(--color-sec-instrumental)",
  tag: "var(--color-sec-tag)",
  outro: "var(--color-sec-outro)",
  ending: "var(--color-sec-outro)",
};

export const sectionColor = (kind: string) => COLORS[kind] ?? "var(--color-sec-other)";

/** Guess the kind from a free-text label ("Pre Chorus 2" → prechorus). */
export function kindFromLabel(label: string): SectionKind {
  const l = label.toLowerCase().replace(/[^a-z]/g, "");
  if (l.startsWith("pre")) return "prechorus";
  if (l.startsWith("intro")) return "intro";
  if (l.startsWith("verse") || l.startsWith("bait")) return "verse";
  if (l.startsWith("chorus") || l.startsWith("reff") || l.startsWith("ref") || l.startsWith("refrain")) return "chorus";
  if (l.startsWith("bridge") || l.startsWith("jembatan")) return "bridge";
  if (l.startsWith("instr") || l.startsWith("interlude") || l.startsWith("musik") || l.startsWith("solo")) return "instrumental";
  if (l.startsWith("tag") || l.startsWith("vamp")) return "tag";
  if (l.startsWith("outro") || l.startsWith("ending") || l.startsWith("coda")) return "outro";
  return "other";
}

/** Short badge text: "Verse 2" → "V2", "Pre-Chorus" → "PC". */
export function sectionShort(label: string): string {
  const num = label.match(/\d+/)?.[0] ?? "";
  const k = kindFromLabel(label);
  const map: Record<string, string> = {
    intro: "In", verse: "V", prechorus: "PC", chorus: "C", bridge: "B", instrumental: "Ins", tag: "T", outro: "Out",
  };
  return (map[k] ?? label.slice(0, 2)) + num;
}

export interface ResolvedSection extends Section {
  end: number;
}

/** Sections sorted with computed end times (mirrors audio::arrangement::resolve_sections). */
export function resolveSections(song: Pick<Song, "sections" | "duration">): ResolvedSection[] {
  const sorted = [...song.sections].sort((a, b) => a.start - b.start);
  if (sorted.length === 0) return [{ id: "__song", label: "Song", kind: "other", start: 0, end: song.duration }];
  if (sorted[0].start > 0) sorted[0] = { ...sorted[0], start: 0 };
  return sorted.map((s, i) => ({ ...s, end: i + 1 < sorted.length ? sorted[i + 1].start : song.duration }));
}

/** Playback plan for a song: custom arrangement order, or natural section order. */
export function buildPlan(song: Pick<Song, "sections" | "duration" | "arrangement">, arrangement?: string[] | null): PlanSegment[] {
  const secs = resolveSections(song);
  const order = arrangement && arrangement.length ? arrangement : song.arrangement;
  const pick = order.length ? order.map((id) => secs.find((s) => s.id === id)).filter(Boolean) as ResolvedSection[] : secs;
  const list = pick.length ? pick : secs;
  return list.map((s) => ({ sectionId: s.id, label: s.label, kind: s.kind, start: s.start, end: s.end }));
}

/** Next free numbered label: existing "Verse 1","Verse 2" + kind verse → "Verse 3". */
export function nextLabel(sections: Section[], kind: SectionKind): string {
  const base = SECTION_KINDS.find((k) => k.kind === kind)?.label ?? "Section";
  // Choruses repeat with the same name; verses, bridges etc. are numbered.
  if (!["verse", "bridge", "instrumental", "tag"].includes(kind)) return base;
  const nums = sections
    .filter((s) => s.label.startsWith(base))
    .map((s) => Number(s.label.slice(base.length).trim()) || 1);
  return nums.length ? `${base} ${Math.max(...nums) + 1}` : `${base} 1`;
}
