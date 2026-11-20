import { toast } from "sonner";
import { loadSong } from "@/lib/actions";
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

export async function togglePlay() {
  let l = loaded();
  if (!l) {
    const r = useApp.getState().route;
    if (r.page === "song" && r.id) {
      l = await loadSong(r.id, "rehearsal");
      if (!l) return;
    } else return;
  }
  const s = st();
  if (isRolling(s?.state)) return guard(ipc.pause());
  const fromTop = !s || s.position < 0.05 || s.state === "STOPPED" || s.state === "READY";
  return guard(ipc.play(fromTop && l.countInBars > 0 && l.clickAvailable));
}

export const stop = () => guard(ipc.stop());
export const fadeSeconds = () => useApp.getState().settings?.fadeSeconds ?? 4;
export const fadeOut = (seconds = fadeSeconds()) => guard(ipc.fade(seconds));

/** Queue (while playing) or jump (while stopped) relative to the current / queued section. */
export function queueSection(delta: number) {
  const l = loaded();
  const s = st();
  if (!l || !l.segments.length) return;
  const base = s?.queuedIndex ?? s?.segmentIndex ?? 0;
  const target = Math.max(0, Math.min(l.segments.length - 1, base + delta));
  return guard(ipc.jump(target, !isRolling(s?.state)));
}

let lastJump: { index: number; bar: boolean; at: number } | null = null;

/** Mode for a click/key press on section `index`, given what is queued now. */
export function nextJumpMode(
  index: number,
  queued: { index: number | null; bar: boolean },
  recent: { index: number; bar: boolean } | null,
): "section" | "bar" | "cancel" {
  const q = recent ?? (queued.index != null ? { index: queued.index, bar: queued.bar } : null);
  if (!q || q.index !== index) return "section";
  return q.bar ? "cancel" : "bar";
}

/**
 * Section jump while playing. Never cuts mid-bar:
 * 1st press → switch when the current section ends (slow blink),
 * 2nd press → switch at the end of this bar (fast blink), 3rd press → cancel.
 * Stopped: moves the playhead right away. `bar` forces end-of-bar (Shift-click).
 */
export function jumpTo(index: number, bar = false) {
  const s = st();
  if (!isRolling(s?.state)) {
    lastJump = null;
    return guard(ipc.jump(index, true, false));
  }
  const now = Date.now();
  const recent = lastJump && now - lastJump.at < 700 ? lastJump : null;
  let mode = nextJumpMode(index, { index: s?.queuedIndex ?? null, bar: !!s?.queuedBar }, recent);
  if (bar && mode === "section") mode = "bar";
  if (mode === "cancel") {
    lastJump = null;
    return guard(ipc.cancelJump());
  }
  lastJump = { index, bar: mode === "bar", at: now };
  return guard(ipc.jump(index, false, mode === "bar"));
}

export const toggleLoop = () => guard(ipc.loop(!st()?.looping));

/** Hold longer than this = vamp only while held; shorter = tap toggles. */
export const VAMP_HOLD_MS = 400;

/**
 * Vamp (loop the current section) with tap-or-hold:
 * tap = on/off, hold = loop until released (then the song carries on at the section end).
 */
export function vampDecision(wasOn: boolean, heldMs: number): { onPress: boolean; onRelease: boolean } {
  const held = heldMs >= VAMP_HOLD_MS;
  return { onPress: true, onRelease: held ? false : !wasOn };
}

let vampDown: { at: number; wasOn: boolean } | null = null;
export function vampPress() {
  if (vampDown) return;
  const wasOn = !!st()?.looping;
  vampDown = { at: performance.now(), wasOn };
  if (!wasOn) void guard(ipc.loop(true));
}
export function vampRelease() {
  if (!vampDown) return;
  const { onRelease } = vampDecision(vampDown.wasOn, performance.now() - vampDown.at);
  vampDown = null;
  if (onRelease !== !!st()?.looping) void guard(ipc.loop(onRelease));
}

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
  fadeOut: () => fadeOut(),
  nextSection: () => queueSection(1),
  prevSection: () => queueSection(-1),
  loopSection: toggleLoop,
  nextSong: () => stepSong(1),
  prevSong: () => stepSong(-1),
  toggleClick,
};
