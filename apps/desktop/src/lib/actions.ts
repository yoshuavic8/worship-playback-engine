import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { useApp } from "@/store/app";
import { errMsg, ipc } from "./ipc";
import type { PlayContext, PrepOptions } from "./types";

export const AUDIO_EXTENSIONS = ["mp3", "wav", "flac", "m4a", "aiff", "aif", "ogg", "aac"];

export async function pickAndImport() {
  const picked = await open({
    multiple: true,
    title: "Import songs",
    filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
  });
  if (!picked) return [];
  return importPaths(Array.isArray(picked) ? picked : [picked]);
}

export async function importPaths(paths: string[]) {
  const audio = paths.filter((p) => AUDIO_EXTENSIONS.includes(p.split(".").pop()?.toLowerCase() ?? ""));
  if (!audio.length) {
    toast.error("No supported audio files", { description: "Use MP3, WAV, FLAC, M4A or AIFF." });
    return [];
  }
  const id = toast.loading(`Importing ${audio.length} file${audio.length > 1 ? "s" : ""}…`);
  try {
    const r = await ipc.importSongs(audio);
    await useApp.getState().refreshSongs();
    if (r.imported.length) {
      toast.success(`Imported ${r.imported.length} song${r.imported.length > 1 ? "s" : ""}`, {
        id,
        description: "Prepare them to create backing tracks.",
        action: {
          label: "Prepare now",
          onClick: () => prepareSongs(r.imported.map((s) => s.id)),
        },
      });
    } else toast.dismiss(id);
    for (const e of r.errors) toast.error(`Could not import ${e.path.split("/").pop()}`, { description: e.error });
    return r.imported;
  } catch (e) {
    toast.error("Import failed", { id, description: errMsg(e) });
    return [];
  }
}

export async function prepareSongs(ids: string[], options: Partial<PrepOptions> = {}) {
  if (!ids.length) return;
  try {
    const jobs = await ipc.prepare(ids, { separate: true, analyze: true, overwriteManual: false, ...options });
    toast.message(`${jobs.length} song${jobs.length > 1 ? "s" : ""} added to the preparation queue`, {
      action: { label: "View queue", onClick: () => useApp.getState().navigate("prep") },
    });
  } catch (e) {
    toast.error("Cannot prepare", { description: errMsg(e), duration: 10000,
      action: { label: "Diagnostics", onClick: () => useApp.getState().navigate("diagnostics") } });
  }
}

export async function loadSong(songId: string, context: PlayContext = "rehearsal") {
  try {
    const info = await ipc.load(songId, context);
    useApp.getState().setLoaded(info);
    for (const w of info.warnings) toast.warning(w, { id: `w-${w}` });
    return info;
  } catch (e) {
    toast.error("Could not load song", { description: errMsg(e) });
    return null;
  }
}

export async function playSong(songId: string) {
  const cur = useApp.getState().loaded;
  const info = cur?.songId === songId ? cur : await loadSong(songId, "rehearsal");
  if (!info) return;
  try {
    await ipc.play(false);
  } catch (e) {
    toast.error(errMsg(e));
  }
}

export async function panic() {
  await ipc.panic().catch(() => {});
}

export async function saveDialog(defaultPath: string, name: string, ext: string) {
  return save({ defaultPath, filters: [{ name, extensions: [ext] }] });
}

export async function openDialog(name: string, ext: string) {
  const p = await open({ multiple: false, filters: [{ name, extensions: [ext] }] });
  return typeof p === "string" ? p : null;
}

export async function pickFolder(title: string) {
  const p = await open({ directory: true, multiple: false, title });
  return typeof p === "string" ? p : null;
}

export function safe<T>(p: Promise<T>, msg = "Something went wrong") {
  return p.catch((e) => {
    toast.error(msg, { description: errMsg(e) });
    throw e;
  });
}
