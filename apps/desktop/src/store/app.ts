import { create } from "zustand";
import { errMsg, ipc } from "@/lib/ipc";
import type {
  AppSettings, AudioSettings, AudioStatus, Bootstrap, LiveInfo, LoadedInfo, PlaybackStatus, PreparationJob,
  Preset, Setlist, Song,
} from "@/lib/types";

export type Page =
  | "dashboard" | "songs" | "song" | "setlists" | "setlist" | "live" | "prep" | "devices" | "settings" | "diagnostics";

export interface Route {
  page: Page;
  id?: string;
}

interface AppState {
  ready: boolean;
  bootError: string | null;
  version: string;
  dataDir: string;
  clickSounds: [string, string][];
  settings: AppSettings | null;
  audio: AudioSettings | null;
  audioStatus: AudioStatus | null;
  presets: Preset[];
  songs: Song[];
  setlists: Setlist[];
  jobs: Record<string, PreparationJob>;
  queue: string[];
  loaded: LoadedInfo | null;
  live: LiveInfo | null;
  route: Route;
  history: Route[];
  importing: boolean;

  init: () => Promise<void>;
  navigate: (page: Page, id?: string) => void;
  back: () => void;
  refreshSongs: () => Promise<void>;
  refreshSetlists: () => Promise<void>;
  refreshJobs: () => Promise<void>;
  refreshPresets: () => Promise<void>;
  refreshAudio: () => Promise<void>;
  upsertSong: (s: Song) => void;
  upsertJob: (j: PreparationJob) => void;
  setSettings: (s: AppSettings) => void;
  setAudioStatus: (s: AudioStatus) => void;
  setLoaded: (l: LoadedInfo | null) => void;
  setLive: (l: LiveInfo | null) => void;
  setImporting: (v: boolean) => void;
}

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  bootError: null,
  version: "",
  dataDir: "",
  clickSounds: [],
  settings: null,
  audio: null,
  audioStatus: null,
  presets: [],
  songs: [],
  setlists: [],
  jobs: {},
  queue: [],
  loaded: null,
  live: null,
  route: { page: "dashboard" },
  history: [],
  importing: false,

  init: async () => {
    try {
      const b: Bootstrap = await ipc.bootstrap();
      const [songs, setlists, jobs, audioStatus, loaded, live] = await Promise.all([
        ipc.listSongs(), ipc.listSetlists(), ipc.listJobs(), ipc.audioStatus(), ipc.loaded(), ipc.liveInfo(),
      ]);
      set({
        ready: true,
        version: b.version,
        dataDir: b.dataDir,
        clickSounds: b.clickSounds,
        settings: b.settings,
        audio: b.audio,
        presets: b.presets,
        songs,
        setlists,
        jobs: Object.fromEntries(jobs.jobs.map((j) => [j.id, j])),
        queue: jobs.queue,
        audioStatus,
        loaded,
        live,
      });
      document.documentElement.style.setProperty("--ui-scale", String(b.settings.uiScale || 1));
    } catch (e) {
      set({ bootError: errMsg(e) });
    }
  },

  navigate: (page, id) => {
    const cur = get().route;
    if (cur.page === page && cur.id === id) return;
    set({ route: { page, id }, history: [...get().history.slice(-30), cur] });
  },
  back: () => {
    const h = get().history;
    if (!h.length) return set({ route: { page: "dashboard" } });
    set({ route: h[h.length - 1], history: h.slice(0, -1) });
  },

  refreshSongs: async () => set({ songs: await ipc.listSongs() }),
  refreshSetlists: async () => set({ setlists: await ipc.listSetlists() }),
  refreshJobs: async () => {
    const r = await ipc.listJobs();
    set({ jobs: Object.fromEntries(r.jobs.map((j) => [j.id, j])), queue: r.queue });
  },
  refreshPresets: async () => set({ presets: await ipc.listPresets() }),
  refreshAudio: async () => {
    const s = await ipc.audioStatus();
    set({ audioStatus: s, audio: s.settings });
  },
  upsertSong: (s) => {
    const songs = get().songs;
    const i = songs.findIndex((x) => x.id === s.id);
    set({ songs: i < 0 ? [s, ...songs] : songs.map((x) => (x.id === s.id ? s : x)) });
  },
  upsertJob: (j) => set({ jobs: { ...get().jobs, [j.id]: j } }),
  setSettings: (settings) => {
    document.documentElement.style.setProperty("--ui-scale", String(settings.uiScale || 1));
    set({ settings });
  },
  setAudioStatus: (s) => set({ audioStatus: s, audio: s.settings }),
  setLoaded: (loaded) => set({ loaded }),
  setLive: (live) => set({ live }),
  setImporting: (importing) => set({ importing }),
}));

/** High-frequency playback status (30 Hz). Separate store so it does not re-render the app. */
export const usePlayback = create<{ status: PlaybackStatus | null; set: (s: PlaybackStatus) => void }>((set) => ({
  status: null,
  set: (status) => set({ status }),
}));

export const songById = (id: string | undefined | null) => useApp.getState().songs.find((s) => s.id === id);
