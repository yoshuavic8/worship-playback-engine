import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { toast } from "sonner";
import { useApp, usePlayback } from "@/store/app";
import { importPaths } from "./actions";
import { isTauri } from "./ipc";
import type { LoadedInfo, PlaybackStatus, PreparationJob } from "./types";

/** Wire Rust → UI events once. Returns a cleanup function. */
export async function connectEvents(): Promise<() => void> {
  if (!isTauri()) return () => {};
  const app = useApp.getState;
  const offs: UnlistenFn[] = [];
  let libTimer: number | undefined;
  const refreshLibrary = () => {
    window.clearTimeout(libTimer);
    libTimer = window.setTimeout(() => app().refreshSongs(), 150);
  };

  offs.push(await listen<PlaybackStatus>("playback:position", (e) => usePlayback.getState().set(e.payload)));
  offs.push(await listen<LoadedInfo>("playback:songChanged", (e) => {
    app().setLoaded(e.payload);
    if (e.payload.liveIndex != null) {
      const live = app().live;
      if (live) app().setLive({ ...live, index: e.payload.liveIndex });
    }
  }));
  offs.push(await listen<string>("audio:error", (e) => toast.error(e.payload, { id: "audio-error", duration: 8000 })));
  offs.push(await listen("audio:deviceChanged", () => app().refreshAudio()));
  offs.push(await listen("library:changed", refreshLibrary));
  offs.push(await listen("setlists:changed", () => app().refreshSetlists()));

  const onJob = (j: PreparationJob) => app().upsertJob(j);
  offs.push(await listen<PreparationJob>("preparation:queued", (e) => { onJob(e.payload); app().refreshJobs(); refreshLibrary(); }));
  offs.push(await listen<PreparationJob>("preparation:started", (e) => { onJob(e.payload); refreshLibrary(); }));
  offs.push(await listen<PreparationJob>("preparation:progress", (e) => onJob(e.payload)));
  offs.push(await listen<PreparationJob>("preparation:completed", (e) => {
    onJob(e.payload);
    app().refreshJobs();
    refreshLibrary();
    const note = e.payload.message && e.payload.message !== "Live-ready" ? e.payload.message : "";
    if (note) {
      toast.warning(`“${e.payload.songTitle}” needs a review`, {
        description: note,
        duration: 12000,
      });
    } else {
      toast.success(`“${e.payload.songTitle}” is live-ready`, {
        description: e.payload.processingSeconds ? `Prepared in ${Math.round(e.payload.processingSeconds)} s` : undefined,
      });
    }
  }));
  offs.push(await listen<PreparationJob>("preparation:failed", (e) => {
    onJob(e.payload);
    app().refreshJobs();
    refreshLibrary();
    if (e.payload.status !== "cancelled") {
      toast.error(`Preparation failed: ${e.payload.songTitle}`, { description: e.payload.error ?? undefined, duration: 10000 });
    }
  }));

  // Drop audio files anywhere on the window to import them.
  offs.push(await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === "enter" || e.payload.type === "over") app().setImporting(true);
    else if (e.payload.type === "leave") app().setImporting(false);
    else if (e.payload.type === "drop") {
      app().setImporting(false);
      if (e.payload.paths.length) importPaths(e.payload.paths);
    }
  }));

  return () => offs.forEach((f) => f());
}
