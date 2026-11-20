// Transpose −4…+4: key names and the render-job store.
import { create } from "zustand";
import { ipc } from "./ipc";
import { KEYS, MINOR_KEYS } from "./keys";
import type { TransposeJob } from "./types";

export const TRANSPOSE_RANGE = [-4, -3, -2, -1, 0, 1, 2, 3, 4] as const;
const SHARP = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/** Key name after transposing: ("E", -1) → "D#". Flat keys stay flat; others use sharps. */
export function transposedKey(key: string | null | undefined, semitones: number): string {
  if (!key) return semitones === 0 ? "Original" : semitoneLabel(semitones);
  const minor = MINOR_KEYS.indexOf(key);
  const major = KEYS.indexOf(key);
  const pc = major >= 0 ? major : minor;
  if (pc < 0) return semitones === 0 ? key : `${key} ${semitoneLabel(semitones)}`;
  const i = (((pc + semitones) % 12) + 12) % 12;
  const flats = key.includes("b");
  const root = flats ? KEYS[i]! : SHARP[i]!;
  return minor >= 0 && major < 0 ? `${root}m` : root;
}

export const semitoneLabel = (s: number) => (s === 0 ? "0" : s > 0 ? `+${s}` : `−${-s}`);

export const jobKey = (songId: string, s: number) => `${songId}:${s}`;

interface TransposeState {
  jobs: Record<string, TransposeJob>;
  set: (j: TransposeJob) => void;
  refresh: () => Promise<void>;
}

export const useTranspose = create<TransposeState>((set) => ({
  jobs: {},
  set: (j) => set((st) => ({ jobs: { ...st.jobs, [jobKey(j.songId, j.semitones)]: j } })),
  refresh: async () => {
    const list = await ipc.transposeJobs();
    set({ jobs: Object.fromEntries(list.map((j) => [jobKey(j.songId, j.semitones), j])) });
  },
}));

/** Songs whose chosen key still needs rendering. */
export function needsKeyRender(s: { transpose?: number; transposed?: number[]; stems: { other: string | null } }): boolean {
  const t = s.transpose ?? 0;
  return t !== 0 && !!s.stems.other && !(s.transposed ?? []).includes(t);
}
