// Browser-only preview of the UI (`npm run dev`, open http://localhost:1420). Never bundled into the app:
// it is only imported in dev mode when no Tauri backend is present. Song titles are invented.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { toast } from "sonner";
import type * as T from "@/lib/types";

const desktopOnlyNotified = new Set<string>();

function desktopOnly(cmd: string): null {
  if (!desktopOnlyNotified.has(cmd)) {
    desktopOnlyNotified.add(cmd);
    toast.message("Desktop app only", {
      id: `mock-${cmd}`,
      description: `"${cmd}" is not simulated in browser preview. Run \`npm run tauri dev\` for the full app.`,
    });
  }
  return null;
}

const now = new Date().toISOString();
const sec = (label: string, kind: string, start: number): T.Section => ({ id: `${label}-${start}`.replace(/\s/g, ""), label, kind, start });
const prep = (p: Partial<T.PrepSettings> = {}): T.PrepSettings => ({ presetId: "FULL_BAND", vocals: "remove", drums: "remove", bass: "keep", bassDb: -6, otherDb: 0, click: true, cue: true, ...p });

function song(id: string, title: string, artist: string, bpm: number | null, key: string | null, duration: number, status: T.SongStatus, sections: [string, string, number][] = []): T.Song {
  const prepared = status === "ready" || status === "warning";
  return {
    id, title, artist, bpm, key, timeSignature: "4/4", beatOffset: 0.12, duration, sampleRate: 48000, channels: 2, dir: `/Users/demo/Library/songs/${id}`,
    sourcePath: "source/original.mp3",
    stems: prepared ? { vocals: "stems/vocals.wav", drums: "stems/drums.wav", bass: "stems/bass.wav", other: "stems/other.wav" } : { vocals: null, drums: null, bass: null, other: null },
    rendered: prepared ? { backing: "rendered/backing.wav", backup: "rendered/backup.wav" } : { backing: null, backup: null },
    status, prep: prep(), routing: { backingToFoh: true, backingToMonitor: true, guideVocalToMonitor: false, clickEnabled: true, cueEnabled: true },
    sections: sections.map(([l, k, s]) => sec(l, k, s)), arrangement: [], countInBars: id === "kau-setia" ? 2 : 1, cueLeadBeats: 4,
    notes: id === "kau-setia" ? "Count-in 2 bars.\nBridge soft, then build. Key change after the last chorus is played live." : "",
    lastError: status === "error" ? "Demucs model not downloaded" : null,
    analysis: prepared ? {
      bpmConfidence: 0.94, keyConfidence: 0.7, renderedWith: prep(),
      separator: { engine: "demucs-mlx", model: "htdemucs", device: "mlx-gpu", seconds: 41, realtimeFactor: 6.3 },
      warnings: status === "warning" ? ["BPM 72 detected with low confidence — please verify"] : [],
      tempoMode: status === "warning" ? "changing" : "fixed",
      tempoMap: status === "warning" ? [
        { startBeat: 1, startTime: 0.4, bpm: 96, timeSignature: "4/4" },
        { startBeat: 48, startTime: 120, bpm: 112, timeSignature: "4/4" },
        { startBeat: 96, startTime: 200, bpm: 84, timeSignature: "4/4" },
      ] : [],
    } : null,
    createdAt: now, updatedAt: now, preparedAt: prepared ? now : null,
  };
}

const STD: [string, string, number][] = [["Intro", "intro", 0], ["Verse 1", "verse", 10], ["Pre-Chorus", "prechorus", 30], ["Chorus", "chorus", 40], ["Verse 2", "verse", 60], ["Chorus", "chorus", 80], ["Bridge", "bridge", 100], ["Chorus", "chorus", 130], ["Outro", "outro", 150]];
const songs: T.Song[] = [
  song("morning-grace", "Morning Grace", "Northside Worship", 72, "G", 268, "ready", STD.map(([l, k, s]) => [l, k, s * 1.6])),
  song("kau-setia", "Kau Setia Selamanya", "Komunitas Pujian", 128, "D", 241, "ready", STD.map(([l, k, s]) => [l, k, s * 1.45])),
  song("open-heaven-skies", "Open Heaven Skies", "Riverside Collective", 96, "A", 302, "warning", STD.map(([l, k, s]) => [l, k, s * 1.8])),
  song("ruang-kudus", "Ruang Kudus", "Kapel Senja", 68, "E", 289, "processing"),
  song("rise-again", "Rise Again", "Lightfield Music", 140, "Bb", 223, "imported"),
  song("still-waters", "Still Waters", "Harbor Hymns", null, null, 255, "error"),
];
const presets: T.Preset[] = [
  { id: "FULL_BAND", name: "Full Band", description: "Live drummer & bassist. Backing keeps keys/guitars, bass at −6 dB.", builtin: true, settings: prep() },
  { id: "NO_DRUMMER", name: "No Drummer", description: "Drums stay in the track, bass full level.", builtin: true, settings: prep({ presetId: "NO_DRUMMER", drums: "keep", bassDb: 0, click: false, cue: false }) },
  { id: "NO_BASS_PLAYER", name: "No Bass Player", description: "Drummer plays live, bass stays in the track.", builtin: true, settings: prep({ presetId: "NO_BASS_PLAYER", bassDb: 0 }) },
  { id: "FULL_TRACK", name: "Full Track", description: "Everything except vocals — no band needed.", builtin: true, settings: prep({ presetId: "FULL_TRACK", drums: "keep", bassDb: 0, click: false, cue: false }) },
];
const item = (songId: string, i: number, transition: T.Transition = "stop"): T.SetlistItem => ({
  id: `it-${i}`, songId, position: i, presetId: null, clickEnabled: null, cueEnabled: null, fohEnabled: null, monitorBacking: null, startSectionId: null, arrangement: null, transition, notes: "",
});
const sunday = new Date(Date.now() + 3 * 86400000).toISOString().slice(0, 10);
const setlists: T.Setlist[] = [
  { id: "sl-1", name: "Sunday Service · Morning", date: sunday, notes: "Communion after song 3.", items: [item("kau-setia", 0, "auto"), item("morning-grace", 1), item("open-heaven-skies", 2), item("ruang-kudus", 3)], createdAt: now, updatedAt: now },
  { id: "sl-2", name: "Youth Night", date: null, notes: "", items: [item("rise-again", 0), item("kau-setia", 1)], createdAt: now, updatedAt: now },
];
let audio: T.AudioSettings = {
  deviceName: "Scarlett 4i4 USB", sampleRate: 48000, bufferSize: 256, clickVolumeDb: -6, cueVolumeDb: -3, monitorBackingDb: -6, clickSound: "classic",
  routing: { id: "multi", name: "Multi-output — FOH 1-2 / Monitor 3-4", mode: "multi", foh: { kind: "stereo", left: 0, right: 1 }, monitor: { kind: "stereo", left: 2, right: 3 } },
};
let settings: T.AppSettings = {
  userName: "yoshua", libraryPath: null, ollamaEnabled: true, ollamaUrl: "http://127.0.0.1:11434", ollamaModel: null, cueVoice: "Samantha", uiScale: 1,
  shortcuts: {}, defaultCountInBars: 1, separationModel: "htdemucs", separationEngine: "auto", separationDevice: "auto", separationProfile: "worship", autoAdvance: true, onboardingDone: true, uiLocale: "en",
};
const jobs: T.PreparationJob[] = [
  { id: "j1", songId: "ruang-kudus", songTitle: "Ruang Kudus", status: "running", stage: "separate", progress: 0.46, message: "Separating (MLX, Apple GPU)", error: null, model: "htdemucs", device: "mlx-gpu", processingSeconds: null, createdAt: now, startedAt: now, finishedAt: null },
  { id: "j0", songId: "morning-grace", songTitle: "Morning Grace", status: "completed", stage: "done", progress: 1, message: "Live-ready", error: null, model: "htdemucs", device: "mlx-gpu", processingSeconds: 41, createdAt: now, startedAt: now, finishedAt: now },
  { id: "jx", songId: "still-waters", songTitle: "Still Waters", status: "failed", stage: "separate", progress: 0.1, message: "", error: "The separation model is not downloaded. Open Diagnostics → AI models.", model: null, device: null, processingSeconds: null, createdAt: now, startedAt: now, finishedAt: now },
];

let loaded: T.LoadedInfo | null = null;
let live: T.LiveInfo | null = null;
const st = { state: "STOPPED" as T.PlaybackState, position: 0, seg: 0, queued: null as number | null, looping: false };

function plan(s: T.Song): T.PlanSegment[] {
  const sorted = [...s.sections].sort((a, b) => a.start - b.start);
  if (!sorted.length) return [{ sectionId: "__song", label: "Song", kind: "other", start: 0, end: s.duration }];
  return sorted.map((x, i) => ({ sectionId: x.id, label: x.label, kind: x.kind, start: x.start, end: sorted[i + 1]?.start ?? s.duration }));
}
function load(id: string, context: T.PlayContext, liveIndex: number | null = null): T.LoadedInfo {
  const s = songs.find((x) => x.id === id)!;
  Object.assign(st, { state: "READY", position: 0, seg: 0, queued: null, looping: false });
  loaded = {
    generation: Date.now(), songId: id, title: s.title, artist: s.artist, key: s.key, bpm: s.bpm, timeSignature: s.timeSignature, duration: s.duration,
    segments: plan(s), startSegment: 0, warnings: [], clickEnabled: !!s.bpm, clickAvailable: !!s.bpm,
    cueEnabled: s.sections.length > 0, cueAvailable: s.sections.length > 0, countInBars: s.countInBars, context, liveIndex,
    source: s.stems.other ? "stems" : "original",
    tracks: s.stems.other ? [
      { role: "other", gainDb: s.prep.otherDb, muted: false, toFoh: true, toMon: true },
      { role: "bass", gainDb: s.prep.bassDb, muted: s.prep.bass === "remove", toFoh: true, toMon: true },
      { role: "drums", gainDb: 0, muted: s.prep.drums !== "keep", toFoh: true, toMon: true },
      { role: "vocals", gainDb: -3, muted: !s.routing.guideVocalToMonitor, toFoh: false, toMon: true },
    ] : [{ role: "original", gainDb: 0, muted: false, toFoh: true, toMon: true }],
  };
  return loaded;
}

const waveform = (d: number): T.Waveform => {
  const n = Math.floor(d * 50);
  const peaks: number[] = [];
  const rms: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = i / 50;
    const env = 0.35 + 0.45 * Math.abs(Math.sin(t / 22)) + 0.15 * Math.sin(t * 3.1);
    const beat = Math.exp(-((t * 1.6) % 1) * 5);
    peaks.push(Math.min(255, Math.round(255 * Math.min(1, env * (0.6 + 0.4 * beat)))));
    rms.push(Math.round(peaks[i] * 0.55));
  }
  return { version: 1, peaksPerSecond: 50, duration: d, peaks, rms };
};

export function install() {
  mockWindows("main");
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
    switch (cmd) {
      case "get_bootstrap": return { settings, audio, presets, version: "0.1.0", dataDir: "/Users/demo/Library/Application Support/app.worshipplayback.engine", clickSounds: [["classic", "Classic"], ["woodblock", "Woodblock"], ["hihat", "Hi-hat"], ["cowbell", "Cowbell"]] };
      case "list_songs": return songs;
      case "save_song": { const i = songs.findIndex((s) => s.id === a.song.id); songs[i] = { ...a.song, updatedAt: new Date().toISOString() }; return songs[i]; }
      case "accept_song_review": { const s = songs.find((x) => x.id === a.id); if (!s) return null; if (s.analysis) s.analysis = { ...s.analysis, warnings: (s.analysis.warnings ?? []).filter((w) => !/bpm|tempo|uncertain/i.test(w)) }; if (!s.analysis?.warnings?.length && s.rendered.backing) s.status = "ready"; s.updatedAt = new Date().toISOString(); return s; }
      case "live_close": live = null; st.state = "STOPPED"; return null;
      case "list_setlists": return setlists;
      case "get_setlist": return setlists.find((s) => s.id === a.id);
      case "save_setlist": { const i = setlists.findIndex((s) => s.id === a.setlist.id); const s = { ...a.setlist, id: a.setlist.id || `sl-${Date.now()}`, updatedAt: new Date().toISOString() }; if (i >= 0) setlists[i] = s; else setlists.push(s); return s; }
      case "list_jobs": return { jobs, queue: ["rise-again"] };
      case "list_presets": return presets;
      case "audio_status": return { running: { deviceName: audio.deviceName, channels: 4, sampleRate: 48000, bufferSize: 256, latencyMs: 5.3 }, settings: audio, issues: [], error: null };
      case "save_audio_settings": audio = a.audio; return { running: { deviceName: audio.deviceName, channels: 4, sampleRate: 48000, bufferSize: 256, latencyMs: 5.3 }, settings: audio, issues: [], error: null };
      case "list_audio_devices": return [
        { name: "Scarlett 4i4 USB", channels: 4, sampleRates: [44100, 48000, 96000], defaultSampleRate: 48000, isDefault: false, bufferMin: 32, bufferMax: 4096 },
        { name: "MacBook Air Speakers", channels: 2, sampleRates: [48000], defaultSampleRate: 48000, isDefault: true, bufferMin: 32, bufferMax: 4096 },
      ];
      case "save_settings": settings = a.settings; return settings;
      case "player_loaded": return loaded;
      case "live_info": return live;
      case "player_load": return load(a.songId, a.context);
      case "player_play": st.state = "PLAYING"; return null;
      case "player_pause": st.state = "PAUSED"; return null;
      case "player_stop": case "player_panic": Object.assign(st, { state: cmd === "player_panic" ? "PANIC" : "STOPPED", position: 0, seg: 0, queued: null }); return null;
      case "player_seek": st.position = a.seconds; return null;
      case "player_jump": if (a.immediate) { st.seg = a.index; st.position = loaded!.segments[a.index].start; } else st.queued = a.index; return null;
      case "player_cancel_jump": st.queued = null; return null;
      case "player_loop": st.looping = a.on; return null;
      case "live_open": { const sl = setlists.find((s) => s.id === a.setlistId)!; live = { setlist: sl, songs: sl.items.map((i) => songs.find((s) => s.id === i.songId)!), index: 0, state: "SONG_READY" }; load(sl.items[0].songId, "live", 0); return live; }
      case "live_goto": case "live_step": { const idx = cmd === "live_goto" ? a.index : (loaded?.liveIndex ?? 0) + a.delta; return load(live!.setlist.items[idx].songId, "live", idx); }
      case "get_waveform": return waveform(songs.find((s) => s.id === a.id)?.duration ?? 200);
      case "run_preflight": return { verdict: "WARNING", checks: [
        { id: "device", label: "Audio device connected", level: "ok", detail: "Scarlett 4i4 USB · 4 outputs · 48 kHz" },
        { id: "routing", label: "Routing is live-safe", level: "ok", detail: "FOH 1-2 · Monitor 3-4" },
        { id: "songs", label: "Songs prepared", level: "warning", detail: "1 song is still preparing" },
        { id: "click", label: "Click available", level: "ok", detail: "All songs have a BPM" },
      ], songs: [{ songId: "ruang-kudus", title: "Ruang Kudus", level: "warning", issues: ["Still preparing"] }] };
      case "system_readiness": return { checks: [
        { id: "silicon", label: "Apple Silicon", status: "ok", detail: "Apple M3 detected", hint: null },
        { id: "macos", label: "macOS", status: "ok", detail: "macOS 26.5 · 16 GB unified memory", hint: null },
        { id: "audio", label: "Audio engine", status: "ok", detail: "Scarlett 4i4 USB · 48000 Hz · 4 outputs", hint: null },
        { id: "ffmpeg", label: "FFmpeg", status: "ok", detail: "ffmpeg version 8.0", hint: null },
        { id: "python", label: "Python environment", status: "ok", detail: "Python 3.12.13", hint: null },
        { id: "mlx", label: "MLX (Apple Silicon)", status: "ok", detail: "demucs-mlx available", hint: null },
        { id: "model", label: "Separation model", status: "ok", detail: "htdemucs · demucs-mlx", hint: null },
        { id: "ollama", label: "Ollama (optional)", status: "ok", detail: "3 models · using qwen2.5:7b", hint: null },
      ], playback: "READY", preparation: "READY", ai: "READY", ollamaModels: ["qwen2.5:7b", "llama3.1:8b"], appVersion: "0.1.0", dataDir: "/Users/demo/Library/Application Support/app.worshipplayback.engine", libraryDir: "/Users/demo/…/songs", schemaVersion: 1, diskUsage: { songs: 2.1e9, models: 1.68e8, pythonEnv: 1.1e9, cues: 3.2e6 } };
      case "list_resources": return [
        { id: "demucs-mlx-htdemucs", kind: "model", engine: "demucs-mlx", model: "htdemucs", name: "HT-Demucs (MLX)", description: "Fastest on Apple Silicon. 4 stems.", version: "1", license: "MIT", source: "https://huggingface.co/ssmall256/demucs-mlx", recommended: true, sizeBytes: 168005865, installed: true, path: "", progress: null },
        { id: "demucs-htdemucs", kind: "model", engine: "demucs", model: "htdemucs", name: "HT-Demucs (PyTorch)", description: "Fallback engine (GPU via MPS or CPU).", version: "4", license: "MIT", source: "https://github.com/adefossez/demucs", recommended: false, sizeBytes: 84141911, installed: false, path: "", progress: { downloaded: 31e6, total: 84141911, message: "Downloading 955717e8-8726e21a.th", error: null, active: true } },
      ];
      case "get_logs": return [{ time: now, level: "info", category: "audio", message: "Audio running on \"Scarlett 4i4 USB\" — 4 ch, 48000 Hz" }, { time: now, level: "info", category: "preparation", message: "separated with demucs-mlx/mlx-gpu in 41.0s (6.3x realtime)" }];
      case "ai_status": return { provider: "ollama", model: "qwen2.5:7b", available: true, models: ["qwen2.5:7b", "llama3.1:8b"] };
      case "set_downbeat": { const s = songs.find((x) => x.id === a.id); if (s) { s.beatOffset = a.offsetSecs; if (s.grid) s.grid = { ...s.grid, beat1Sec: a.offsetSecs }; } return s ?? null; }
      case "set_grid": {
        const s = songs.find((x) => x.id === a.id);
        if (s && a.grid) {
          s.grid = a.grid;
          const bpb = Number(a.grid.beatsPerBar) || 4;
          const pts = [...(a.grid.points ?? [])] as { bar: number; beat: number; bpm: number }[];
          pts.sort((p, q) => (p.bar - 1) * bpb + (p.beat - 1) - ((q.bar - 1) * bpb + (q.beat - 1)));
          s.bpm = pts[0]?.bpm ?? s.bpm;
          s.beatOffset = a.grid.beat1Sec ?? s.beatOffset;
          const n = Number(String(s.timeSignature).split("/")[0]);
          if (n !== bpb) s.timeSignature = bpb === 6 ? "6/8" : `${bpb}/4`;
        }
        return s ? { ...s } : null;
      }
      case "ai_song_command": {
        const text = String(a.text ?? "").toLowerCase();
        const song = songs.find((s) => s.id === a.songId);
        if (/count-in|downbeat|tempo|hitung/.test(text)) {
          return {
            intent: "update_song",
            summary: "Count-in 2 bar sebelum intro.",
            changes: [
              { field: "Count-in", from: "1 bar (~3.3s, 4 beats)", to: "2 bars (~6.7s, 8 beats)" },
            ],
            prep: song?.prep ?? null,
            routing: song?.routing ?? null,
            arrangement: null,
            timeline: { countInBars: 2 },
            setlistSongIds: [], setlistName: null, warnings: [], requiresRerender: false, provider: "ollama", model: "qwen2.5:7b",
          };
        }
        return { intent: "set_preset", summary: "Drummer is out, bassist plays: keep drums in the track, remove bass.", changes: [{ field: "Drums", from: "remove", to: "keep" }, { field: "Bass", from: "keep (−6 dB)", to: "remove" }], prep: prep({ presetId: null, drums: "keep", bass: "remove" }), routing: null, arrangement: null, timeline: null, setlistSongIds: [], setlistName: null, warnings: [], requiresRerender: true, provider: "ollama", model: "qwen2.5:7b" };
      }
      case "list_voices": return [{ name: "Samantha", locale: "en_US" }, { name: "Damayanti", locale: "id_ID" }];
      default: return desktopOnly(cmd);
    }
  }, { shouldMockEvents: true });

  setInterval(() => {
    if (!loaded) return;
    if (st.state === "PLAYING") {
      st.position += 1 / 30;
      const seg = loaded.segments[st.seg];
      if (seg && st.position >= seg.end) {
        if (st.looping) st.position = seg.start;
        else if (st.queued != null) { st.seg = st.queued; st.position = loaded.segments[st.seg].start; st.queued = null; }
        else st.seg = Math.min(loaded.segments.length - 1, st.seg + 1);
      }
    }
    const m = st.state === "PLAYING" ? 0.3 + Math.random() * 0.5 : 0;
    const status: T.PlaybackStatus = {
      state: st.state, generation: loaded.generation, songId: loaded.songId, position: st.position, duration: loaded.duration, segmentIndex: st.seg,
      queuedIndex: st.queued, looping: st.looping, countInRemaining: 0, fading: false, meters: [m, m * 0.95, m * 0.8, m * 0.85],
      liveIndex: loaded.liveIndex, setlistState: live ? "SONG_PLAYING" : null, engineRunning: true,
    };
    emit("playback:position", status);
  }, 33);
}
