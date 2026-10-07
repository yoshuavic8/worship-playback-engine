import { toast } from "sonner";
import { useApp, usePlayback } from "@/store/app";
import { errMsg, ipc } from "./ipc";

const st = () => usePlayback.getState().status;
const loaded = () => useApp.getState().loaded;

export const isRolling = (state?: string | null) => state === "PLAYING" || state === "COUNT_IN";

async function guard(p: Promise<unknown>) {
  try {
    await p;
  } catch (e) {
    toast.error(errMsg(e), { id: "transport" });
  }
}

export function togglePlay() {
  const l = loaded();
  if (!l) return;
  const s = st();
  if (isRolling(s?.state)) return guard(ipc.pause());
  const fromTop = !s || s.position < 0.05 || s.state === "STOPPED" || s.state === "READY";
  return guard(ipc.play(fromTop && l.countInBars > 0 && l.clickAvailable));
}

export const stop = () => guard(ipc.stop());
export const fadeOut = (seconds = 4) => guard(ipc.fade(seconds));

/** Queue (while playing) or jump (while stopped) relative to the current / queued section. */
export function queueSection(delta: number) {
  const l = loaded();
  const s = st();
  if (!l || !l.segments.length) return;
  const base = s?.queuedIndex ?? s?.segmentIndex ?? 0;
  const target = Math.max(0, Math.min(l.segments.length - 1, base + delta));
  return guard(ipc.jump(target, !isRolling(s?.state)));
}

export function jumpTo(index: number, immediate: boolean) {
  const s = st();
  if (!isRolling(s?.state)) immediate = true;
  if (!immediate && s?.queuedIndex === index) return guard(ipc.cancelJump());
  return guard(ipc.jump(index, immediate));
}

export const toggleLoop = () => guard(ipc.loop(!st()?.looping));

export function toggleClick() {
  const l = loaded();
  if (!l) return;
  if (!l.clickAvailable && !l.clickEnabled) {
    toast.warning("Click needs a BPM", { id: "click", description: "Set the BPM in the song editor." });
    return;
  }
  const next = !l.clickEnabled;
  useApp.getState().setLoaded({ ...l, clickEnabled: next });
  return guard(ipc.setClick(next));
}

export function toggleCue() {
  const l = loaded();
  if (!l) return;
  const next = !l.cueEnabled;
  useApp.getState().setLoaded({ ...l, cueEnabled: next });
  return guard(ipc.setCue(next));
}

export async function stepSong(delta: number) {
  if (!useApp.getState().live) return;
  if (isRolling(st()?.state)) {
    toast.message("Stop or fade the current song first", { id: "step" });
    return;
  }
  try {
    const info = await ipc.liveStep(delta);
    useApp.getState().setLoaded(info);
  } catch (e) {
    toast.error(errMsg(e), { id: "step" });
  }
}

export async function gotoSong(index: number) {
  if (isRolling(st()?.state)) {
    toast.message("Stop or fade the current song first", { id: "step" });
    return;
  }
  try {
    const info = await ipc.liveGoto(index);
    useApp.getState().setLoaded(info);
  } catch (e) {
    toast.error(errMsg(e), { id: "step" });
  }
}

export const panicStop = () => ipc.panic().catch(() => {});

export const ACTIONS: Record<string, () => unknown> = {
  playPause: togglePlay,
  stop,
  panic: panicStop,
  fadeOut: () => fadeOut(4),
  nextSection: () => queueSection(1),
  prevSection: () => queueSection(-1),
  loopSection: toggleLoop,
  nextSong: () => stepSong(1),
  prevSong: () => stepSong(-1),
  toggleClick,
};
