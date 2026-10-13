// Mirrors the Rust models (serde camelCase). Keep in sync with src-tauri/src/models.rs.

import type { Grid } from "./tempoMap";

export type { Grid };

export type SongStatus = "imported" | "queued" | "processing" | "ready" | "warning" | "error";
export type StemMode = "remove" | "keep" | "optional";

export interface PrepSettings {
  presetId: string | null;
  vocals: StemMode;
  drums: StemMode;
  bass: StemMode;
  bassDb: number;
  otherDb: number;
  click: boolean;
  cue: boolean;
}

export interface Preset {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  settings: PrepSettings;
}

export interface StemPaths {
  vocals: string | null;
  drums: string | null;
  bass: string | null;
  other: string | null;
}

export interface RenderedPaths {
  backing: string | null;
  backup: string | null;
}

export type SectionKind =
  | "intro" | "verse" | "prechorus" | "chorus" | "bridge" | "instrumental" | "tag" | "outro" | "other";

export interface Section {
  id: string;
  label: string;
  kind: SectionKind | string;
  start: number;
}

export interface SongRouting {
  backingToFoh: boolean;
  backingToMonitor: boolean;
  guideVocalToMonitor: boolean;
  clickEnabled: boolean;
  cueEnabled: boolean;
}

export interface TempoRegion {
  startBeat: number;
  startTime: number;
  bpm: number;
  timeSignature: string;
}

export interface SongAnalysis {
  bpmConfidence?: number | null;
  keyConfidence?: number | null;
  detected?: { bpm?: number | null; key?: string | null; beatOffset?: number | null; sectionCount?: number; bpmAlternatives?: number[] };
  separator?: { engine: string; model: string; device: string; seconds: number; realtimeFactor: number };
  timings?: Record<string, number>;
  renderedWith?: PrepSettings;
  warnings?: string[];
  // Musical timeline — distinguish audio start vs. musical start vs. first downbeat
  audioStart?: number | null;        // first non-silent audio frame (seconds)
  musicalStart?: number | null;      // first instrumental onset — after crowd / WL speech
  // Tempo stability
  tempoMode?: "fixed" | "drifting" | "changing" | null;
  stability?: number | null;         // 0–1: how consistent the tempo is
  downbeatConfidence?: number | null;// 0–1: how confident the downbeat position is
  // TempoMap: only populated when tempoMode != "fixed"
  tempoMap?: TempoRegion[] | null;
  // Beat grid lock: user must confirm before click uses explicit beats
  beatGridLocked?: boolean;
  [k: string]: unknown;
}

export interface Song {
  id: string;
  title: string;
  artist: string;
  bpm: number | null;
  key: string | null;
  keyPitchClass?: number | null;
  keyMode?: "major" | "minor" | string | null;
  timeSignature: string;
  beatOffset: number;
  duration: number;
  sampleRate: number;
  channels: number;
  dir: string;
  sourcePath: string;
  stems: StemPaths;
  rendered: RenderedPaths;
  status: SongStatus;
  prep: PrepSettings;
  routing: SongRouting;
  sections: Section[];
  arrangement: string[];
  countInBars: number;
  cueLeadBeats: number;
  notes: string;
  lastError: string | null;
  analysis: SongAnalysis | null;
  /** Tempo automation. Missing on older songs — the editor builds one step point from bpm. */
  grid?: Grid | null;
  createdAt: string;
  updatedAt: string;
  preparedAt: string | null;
}

export type Transition = "stop" | "auto";

export interface SetlistItem {
  id: string;
  songId: string;
  position: number;
  presetId: string | null;
  clickEnabled: boolean | null;
  cueEnabled: boolean | null;
  fohEnabled: boolean | null;
  monitorBacking: boolean | null;
  startSectionId: string | null;
  arrangement: string[] | null;
  transition: Transition;
  notes: string;
}

export interface Setlist {
  id: string;
  name: string;
  date: string | null;
  notes: string;
  items: SetlistItem[];
  createdAt: string;
  updatedAt: string;
}

export type OutputTarget =
  | { kind: "none" }
  | { kind: "mono"; channel: number }
  | { kind: "stereo"; left: number; right: number };

export interface RoutingProfile {
  id: string;
  name: string;
  mode: "split" | "multi" | "rehearsal" | "custom" | string;
  foh: OutputTarget;
  monitor: OutputTarget;
}

export interface AudioSettings {
  deviceName: string | null;
  sampleRate: number;
  bufferSize: number;
  routing: RoutingProfile;
  clickVolumeDb: number;
  cueVolumeDb: number;
  monitorBackingDb: number;
  clickSound: string;
}

export interface AppSettings {
  userName: string;
  libraryPath: string | null;
  ollamaEnabled: boolean;
  ollamaUrl: string;
  ollamaModel: string | null;
  cueVoice: string | null;
  uiScale: number;
  shortcuts: Record<string, string>;
  defaultCountInBars: number;
  separationModel: string;
  separationEngine: string;
  separationDevice: string;
  /** worship = Inst HQ 3 vocals + Demucs instruments. fast = Demucs only. */
  separationProfile: string;
  autoAdvance: boolean;
  onboardingDone: boolean;
  /** Display language for dates and a few UI strings (full UI remains English). */
  uiLocale: "en" | "id";
}

export interface SetupStep {
  id: string;
  title: string;
  detail: string;
  sizeBytes: number;
  ready: boolean;
}

export interface SetupStatus {
  complete: boolean;
  phase: "idle" | "engine" | "models" | "done" | "error";
  message: string;
  progress: number;
  etaSecs: number | null;
  paused: boolean;
  steps: SetupStep[];
}

export interface Bootstrap {
  settings: AppSettings;
  audio: AudioSettings;
  presets: Preset[];
  version: string;
  dataDir: string;
  clickSounds: [string, string][];
  setup: SetupStatus;
}

export interface DeviceInfo {
  name: string;
  channels: number;
  sampleRates: number[];
  defaultSampleRate: number;
  isDefault: boolean;
  bufferMin: number | null;
  bufferMax: number | null;
}

export interface RunningInfo {
  deviceName: string;
  channels: number;
  sampleRate: number;
  bufferSize: number | null;
  latencyMs: number | null;
}

export interface RoutingIssue {
  level: "error" | "warning" | "info" | string;
  message: string;
}

export interface AudioStatus {
  running: RunningInfo | null;
  settings: AudioSettings;
  issues: RoutingIssue[];
  error: string | null;
}

export type TrackRole = "backing" | "backup" | "original" | "vocals" | "drums" | "bass" | "other";

export interface TrackInfo {
  role: TrackRole;
  gainDb: number;
  muted: boolean;
  toFoh: boolean;
  toMon: boolean;
}

export interface PlanSegment {
  sectionId: string;
  label: string;
  kind: string;
  start: number;
  end: number;
}

export type PlayContext = "live" | "rehearsal";

export interface LoadedInfo {
  generation: number;
  songId: string;
  title: string;
  artist: string;
  key: string | null;
  bpm: number | null;
  timeSignature: string;
  duration: number;
  segments: PlanSegment[];
  startSegment: number;
  tracks: TrackInfo[];
  source: string;
  warnings: string[];
  clickEnabled: boolean;
  clickAvailable: boolean;
  cueEnabled: boolean;
  cueAvailable: boolean;
  countInBars: number;
  context: PlayContext;
  liveIndex: number | null;
}

export type PlaybackState =
  | "STOPPED" | "LOADING" | "READY" | "COUNT_IN" | "PLAYING" | "PAUSED" | "NEXTING" | "ERROR" | "PANIC";
export type SetlistState =
  | "SETLIST_READY" | "SONG_LOADING" | "SONG_READY" | "SONG_PLAYING" | "SONG_COMPLETED" | "TRANSITIONING";

export interface PlaybackStatus {
  state: PlaybackState;
  generation: number;
  songId: string | null;
  position: number;
  duration: number;
  segmentIndex: number;
  queuedIndex: number | null;
  looping: boolean;
  countInRemaining: number;
  fading: boolean;
  meters: [number, number, number, number];
  liveIndex: number | null;
  setlistState: SetlistState | null;
  engineRunning: boolean;
}

export interface LiveInfo {
  setlist: Setlist;
  songs: Song[];
  index: number;
  state: SetlistState;
}

export interface PreparationJob {
  id: string;
  songId: string;
  songTitle: string;
  status: "queued" | "running" | "completed" | "failed" | "cancelled" | string;
  stage: string;
  progress: number;
  message: string;
  error: string | null;
  model: string | null;
  device: string | null;
  processingSeconds: number | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface JobList {
  jobs: PreparationJob[];
  queue: string[];
}

export interface PrepOptions {
  separate: boolean;
  analyze: boolean;
  overwriteManual: boolean;
}

export interface ImportResult {
  imported: Song[];
  errors: { path: string; error: string }[];
}

export interface PreflightItem {
  id: string;
  label: string;
  level: "ok" | "warning" | "error" | string;
  detail: string;
}

export interface PreflightReport {
  verdict: "READY" | "WARNING" | "NOT READY" | string;
  checks: PreflightItem[];
  songs: { songId: string; title: string; level: string; issues: string[] }[];
}

export interface Check {
  id: string;
  label: string;
  status: "ok" | "warning" | "error" | "info" | string;
  detail: string;
  hint: string | null;
}

export interface CompatInfo {
  profile: "full" | "lite";
  reason: string;
  separationBackend: string;
  prepConcurrency: number;
  prepEtaWarning: string | null;
  mlxAvailable: boolean;
  hardware: {
    archLabel: string;
    macosVersion: string;
    ramGb: number;
    rosetta: boolean;
    appleSilicon: boolean;
  };
  ollamaVersion: string | null;
}

export interface Readiness {
  checks: Check[];
  playback: string;
  preparation: string;
  ai: string;
  ollamaModels: string[];
  compat: CompatInfo;
  appVersion: string;
  dataDir: string;
  libraryDir: string;
  schemaVersion: number;
  diskUsage: Record<string, number>;
}

export interface ResourceProgress {
  downloaded: number;
  total: number;
  message: string;
  error: string | null;
  active: boolean;
}

export interface ResourceStatus {
  id: string;
  kind: string;
  engine: string;
  model: string;
  name: string;
  description: string;
  version: string;
  license: string;
  source: string;
  recommended: boolean;
  sizeBytes: number;
  installed: boolean;
  path: string;
  progress: ResourceProgress | null;
}

export interface LogEntry {
  time: string;
  level: string;
  category: string;
  message: string;
}

export interface Voice {
  name: string;
  locale: string;
}

export interface AiStatus {
  provider: string;
  model: string | null;
  available: boolean;
  models: string[];
  rulesOnly: boolean;
  compatProfile: "full" | "lite";
}

/** Validated timeline edits from the local assistant. Missing fields stay as they are. */
export interface TimelinePatch {
  countInBars?: number;
  cueLeadBeats?: number;
  beatOffset?: number;
  bpm?: number;
  timeSignature?: string;
  key?: string;
}

export interface AiProposal {
  intent: string;
  summary: string;
  changes: { field: string; from: string; to: string }[];
  prep: PrepSettings | null;
  routing: SongRouting | null;
  arrangement: string[] | null;
  timeline: TimelinePatch | null;
  setlistSongIds: string[];
  setlistName: string | null;
  warnings: string[];
  requiresRerender: boolean;
  provider: string;
  model: string | null;
}

export interface Waveform {
  version: number;
  peaksPerSecond: number;
  duration: number;
  peaks: number[];
  rms: number[];
}

