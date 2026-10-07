// The only place the UI talks to Rust. Every call is typed; arguments are camelCase (Tauri maps
// them to the snake_case Rust parameters).
import { invoke } from "@tauri-apps/api/core";
import type * as T from "./types";

const call = <R>(cmd: string, args?: Record<string, unknown>) => invoke<R>(cmd, args);

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const ipc = {
  // bootstrap & settings
  bootstrap: () => call<T.Bootstrap>("get_bootstrap"),
  saveSettings: (settings: T.AppSettings) => call<T.AppSettings>("save_settings", { settings }),
  audioStatus: () => call<T.AudioStatus>("audio_status"),
  listDevices: () => call<T.DeviceInfo[]>("list_audio_devices"),
  saveAudio: (audio: T.AudioSettings) => call<T.AudioStatus>("save_audio_settings", { audio }),
  testSignal: (kind: "foh" | "monitor" | "click" | "cue" | "stop") => call<void>("test_signal", { kind }),
  listVoices: () => call<T.Voice[]>("list_voices"),
  previewCue: (text: string, voice: string | null) => call<void>("preview_cue", { text, voice }),

  // songs
  listSongs: () => call<T.Song[]>("list_songs"),
  getSong: (id: string) => call<T.Song>("get_song", { id }),
  importSongs: (paths: string[]) => call<T.ImportResult>("import_songs", { paths }),
  saveSong: (song: T.Song) => call<T.Song>("save_song", { song }),
  deleteSong: (id: string) => call<void>("delete_song", { id }),
  duplicateSong: (id: string) => call<T.Song>("duplicate_song", { id }),
  waveform: (id: string) => call<T.Waveform>("get_waveform", { id }),
  rerenderBacking: (id: string) => call<T.Song>("rerender_backing", { id }),

  // preparation
  prepare: (ids: string[], options: T.PrepOptions) => call<T.PreparationJob[]>("prepare_songs", { ids, options }),
  cancelJob: (id: string) => call<void>("cancel_job", { id }),
  listJobs: () => call<T.JobList>("list_jobs"),

  // presets
  listPresets: () => call<T.Preset[]>("list_presets"),
  savePreset: (preset: T.Preset) => call<T.Preset>("save_preset", { preset }),
  deletePreset: (id: string) => call<void>("delete_preset", { id }),

  // setlists
  listSetlists: () => call<T.Setlist[]>("list_setlists"),
  getSetlist: (id: string) => call<T.Setlist>("get_setlist", { id }),
  saveSetlist: (setlist: T.Setlist) => call<T.Setlist>("save_setlist", { setlist }),
  deleteSetlist: (id: string) => call<void>("delete_setlist", { id }),
  duplicateSetlist: (id: string) => call<T.Setlist>("duplicate_setlist", { id }),
  exportSetlist: (id: string, path: string) => call<void>("export_setlist", { id, path }),
  importSetlist: (path: string) => call<T.Setlist>("import_setlist", { path }),
  preflight: (setlistId: string) => call<T.PreflightReport>("run_preflight", { setlistId }),

  // player
  load: (songId: string, context: T.PlayContext) => call<T.LoadedInfo>("player_load", { songId, context }),
  play: (countIn: boolean) => call<void>("player_play", { countIn }),
  pause: () => call<void>("player_pause"),
  toggle: (countIn: boolean) => call<void>("player_toggle", { countIn }),
  stop: () => call<void>("player_stop"),
  panic: () => call<void>("player_panic"),
  fade: (seconds: number) => call<void>("player_fade", { seconds }),
  seek: (seconds: number) => call<void>("player_seek", { seconds }),
  jump: (index: number, immediate: boolean) => call<void>("player_jump", { index, immediate }),
  cancelJump: () => call<void>("player_cancel_jump"),
  loop: (on: boolean) => call<void>("player_loop", { on }),
  setTrack: (role: T.TrackRole, gainDb: number, muted: boolean) => call<void>("player_set_track", { role, gainDb, muted }),
  setClick: (enabled: boolean) => call<void>("player_set_click", { enabled }),
  setCue: (enabled: boolean) => call<void>("player_set_cue", { enabled }),
  status: () => call<T.PlaybackStatus>("player_status"),
  loaded: () => call<T.LoadedInfo | null>("player_loaded"),
  unload: () => call<void>("player_unload"),

  // live
  liveOpen: (setlistId: string) => call<T.LiveInfo>("live_open", { setlistId }),
  liveGoto: (index: number) => call<T.LoadedInfo>("live_goto", { index }),
  liveStep: (delta: number) => call<T.LoadedInfo>("live_step", { delta }),
  liveClose: () => call<void>("live_close"),
  liveInfo: () => call<T.LiveInfo | null>("live_info"),

  // diagnostics & resources
  readiness: () => call<T.Readiness>("system_readiness"),
  listResources: () => call<T.ResourceStatus[]>("list_resources"),
  downloadResource: (id: string) => call<void>("download_resource", { id }),
  cancelDownload: (id: string) => call<void>("cancel_download", { id }),
  removeResource: (id: string) => call<void>("remove_resource", { id }),
  installPythonEnv: () => call<void>("install_python_env"),
  logs: (limit?: number, category?: string | null) => call<T.LogEntry[]>("get_logs", { limit, category }),
  appPaths: () => call<T.AppPathsInfo>("app_paths"),

  // AI (local Ollama only — optional)
  aiStatus: () => call<T.AiStatus>("ai_status"),
  aiSong: (songId: string, text: string) => call<T.AiProposal>("ai_song_command", { songId, text }),
  aiSetlist: (text: string) => call<T.AiProposal>("ai_setlist_command", { text }),

  // backup
  exportLibrary: (path: string) => call<number>("export_library", { path }),
  importLibrary: (path: string) =>
    call<{ songs: number; skippedSongs: number; setlists: number; presets: number }>("import_library", { path }),
};

export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}
