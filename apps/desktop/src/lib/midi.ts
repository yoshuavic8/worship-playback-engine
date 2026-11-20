// MIDI Learn helpers: labels, target identity and which mapping a control shows.
import { create } from "zustand";
import { ipc } from "./ipc";
import { keyLabel } from "./shortcuts";
import type { MidiActivity, MidiMapping, MidiSource, MidiStatus, MidiTarget, TrackRole } from "./types";

export const TRACK_LABEL: Record<TrackRole, string> = {
  other: "Keys / guitars",
  bass: "Bass",
  drums: "Drums",
  backing: "Backing",
  backup: "Backup",
  original: "Original",
  vocals: "Guide vocal",
};

/** Stable identity for a target (used to find a control's mapping). */
export function targetKey(t: MidiTarget): string {
  switch (t.type) {
    case "sectionSlot": return `slot:${t.slot}`;
    case "songSection": return `section:${t.sectionId}`;
    case "trackVolume": case "trackMute": case "trackSolo": return `${t.type}:${t.role}`;
    default: return t.type;
  }
}

export function targetLabel(t: MidiTarget): string {
  switch (t.type) {
    case "sectionSlot": return `Section ${t.slot} (any song)`;
    case "songSection": return `${t.label || "Section"} (this song)`;
    case "playPause": return "Play / pause";
    case "stop": return "Stop";
    case "fadeOut": return "Fade out";
    case "nextSong": return "Next song";
    case "prevSong": return "Previous song";
    case "nextSection": return "Next section";
    case "prevSection": return "Previous section";
    case "cancelJump": return "Cancel jump";
    case "vamp": return "Vamp (loop section)";
    case "toggleClick": return "Click on / off";
    case "toggleCue": return "Voice cues on / off";
    case "transposeUp": return "Transpose +1";
    case "transposeDown": return "Transpose −1";
    case "masterVolume": return "FOH master";
    case "clickVolume": return "Click volume";
    case "cueVolume": return "Cue volume";
    case "trackVolume": return `${TRACK_LABEL[t.role]} volume`;
    case "trackMute": return `${TRACK_LABEL[t.role]} mute`;
    case "trackSolo": return `${TRACK_LABEL[t.role]} solo`;
  }
}

export const isContinuous = (t: MidiTarget) =>
  t.type === "masterVolume" || t.type === "clickVolume" || t.type === "cueVolume" || t.type === "trackVolume";
export const isSwitch = (t: MidiTarget) =>
  t.type === "vamp" || t.type === "toggleClick" || t.type === "toggleCue" || t.type === "trackMute" || t.type === "trackSolo";

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
export const noteName = (n: number) => `${NOTE_NAMES[n % 12]}${Math.floor(n / 12) - 1}`;

/** "Note C1 · ch 10", "CC 112", "PC 5", "Key Q". */
export function sourceLabel(s: Pick<MidiSource, "kind" | "channel" | "number" | "key">, withChannel = true): string {
  const ch = withChannel ? ` · ch ${s.channel + 1}` : "";
  switch (s.kind) {
    case "note": return `Note ${noteName(s.number)}${ch}`;
    case "cc": return `CC ${s.number}${ch}`;
    case "pc": return `PC ${s.number + 1}${ch}`;
    case "key": return `Key ${keyLabel(s.key ?? "")}`;
  }
}

/** Short badge text shown on a control in Learn mode. */
export function sourceShort(s: MidiSource): string {
  switch (s.kind) {
    case "note": return noteName(s.number);
    case "cc": return `CC${s.number}`;
    case "pc": return `PC${s.number + 1}`;
    case "key": return keyLabel(s.key ?? "");
  }
}

/** The mapping a control shows: this song's mapping first, then the global one. */
export function mappingFor(maps: MidiMapping[], target: MidiTarget, songId: string | null): MidiMapping | undefined {
  const k = targetKey(target);
  const hits = maps.filter((m) => targetKey(m.target) === k);
  return hits.find((m) => songId != null && m.songId === songId) ?? hits.find((m) => m.songId == null);
}

/** True when a computer key is mapped (so it is forwarded to the mapping engine). */
export function isKeyMapped(maps: MidiMapping[], code: string, songId: string | null): boolean {
  return maps.some((m) => m.source.kind === "key" && m.source.key === code && (m.songId == null || m.songId === songId));
}

interface MidiState {
  learn: boolean;
  armed: { target: MidiTarget; songId: string | null } | null;
  /** Section buttons learned in Learn mode apply to this song only, or to every song by position. */
  sectionScope: "song" | "all";
  mappings: MidiMapping[];
  status: MidiStatus | null;
  activity: MidiActivity | null;
  setLearn: (on: boolean) => void;
  arm: (target: MidiTarget, songId: string | null) => void;
  disarm: () => void;
  setScope: (s: "song" | "all") => void;
  refresh: () => Promise<void>;
  setStatus: (s: MidiStatus) => void;
  setActivity: (a: MidiActivity) => void;
}

export const useMidi = create<MidiState>((set, get) => ({
  learn: false,
  armed: null,
  sectionScope: "song",
  mappings: [],
  status: null,
  activity: null,
  setLearn: (learn) => {
    if (!learn && get().armed) ipc.midiLearnArm(null).catch(() => {});
    set({ learn, armed: learn ? get().armed : null });
  },
  arm: (target, songId) => {
    set({ armed: { target, songId } });
    ipc.midiLearnArm(target, songId).catch(() => {});
  },
  disarm: () => {
    set({ armed: null });
    ipc.midiLearnArm(null).catch(() => {});
  },
  setScope: (sectionScope) => set({ sectionScope }),
  refresh: async () => {
    const [mappings, status] = await Promise.all([ipc.midiMappings(), ipc.midiStatus()]);
    set({ mappings, status });
  },
  setStatus: (status) => set({ status }),
  setActivity: (activity) => set({ activity }),
}));
