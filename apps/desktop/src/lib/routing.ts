// Mirrors src-tauri/src/audio/routing.rs for instant UI feedback. The backend validates again on save.
import type { OutputTarget, RoutingIssue, RoutingProfile } from "./types";

export const PROFILES: Record<"split" | "multi" | "rehearsal", RoutingProfile> = {
  split: { id: "split", name: "Compatibility — L/R Split", mode: "split", foh: { kind: "mono", channel: 0 }, monitor: { kind: "mono", channel: 1 } },
  multi: { id: "multi", name: "Multi-output — FOH 1-2 / Monitor 3-4", mode: "multi", foh: { kind: "stereo", left: 0, right: 1 }, monitor: { kind: "stereo", left: 2, right: 3 } },
  rehearsal: { id: "rehearsal", name: "Rehearsal — Monitor mix on 1-2", mode: "rehearsal", foh: { kind: "none" }, monitor: { kind: "stereo", left: 0, right: 1 } },
};

export function targetChannels(t: OutputTarget): number[] {
  if (t.kind === "mono") return [t.channel];
  if (t.kind === "stereo") return [t.left, t.right];
  return [];
}

export function validateRouting(p: RoutingProfile, channels: number): RoutingIssue[] {
  const issues: RoutingIssue[] = [];
  const foh = targetChannels(p.foh);
  const mon = targetChannels(p.monitor);
  for (const c of [...foh, ...mon]) {
    if (c >= channels) issues.push({ level: "error", message: `Output ${c + 1} does not exist on this device (it has ${channels} outputs).` });
  }
  if (foh.some((c) => mon.includes(c))) {
    issues.push({ level: "error", message: "FOH and Monitor share an output. Click and cue would be audible in the house." });
  }
  if (p.monitor.kind === "none") issues.push({ level: "warning", message: "Monitor bus is not routed — the band will not hear click or cue." });
  if (p.foh.kind === "none" && p.mode !== "rehearsal") issues.push({ level: "warning", message: "FOH bus is not routed — nothing will be sent to the house." });
  if (p.mode === "rehearsal") issues.push({ level: "warning", message: "Rehearsal profile: only the monitor mix is played (no FOH output)." });
  return issues;
}

export const isLiveSafe = (p: RoutingProfile, channels: number) => validateRouting(p, channels).every((i) => i.level !== "error");

/** Which bus feeds each physical output (for the routing diagram). */
export function channelMap(p: RoutingProfile, channels: number): ("foh" | "mon" | "conflict" | null)[] {
  const foh = targetChannels(p.foh);
  const mon = targetChannels(p.monitor);
  return Array.from({ length: channels }, (_, c) => {
    const f = foh.includes(c);
    const m = mon.includes(c);
    return f && m ? "conflict" : f ? "foh" : m ? "mon" : null;
  });
}

export function encodeTarget(t: OutputTarget): string {
  if (t.kind === "mono") return `m${t.channel}`;
  if (t.kind === "stereo") return `s${t.left}-${t.right}`;
  return "none";
}

export function decodeTarget(v: string): OutputTarget {
  if (v.startsWith("m")) return { kind: "mono", channel: Number(v.slice(1)) };
  if (v.startsWith("s")) {
    const [l, r] = v.slice(1).split("-").map(Number);
    return { kind: "stereo", left: l, right: r };
  }
  return { kind: "none" };
}

export function targetOptions(channels: number): { value: string; label: string }[] {
  const o = [{ value: "none", label: "Off" }];
  for (let c = 0; c + 1 < channels; c += 2) o.push({ value: `s${c}-${c + 1}`, label: `Stereo ${c + 1}-${c + 2}` });
  for (let c = 0; c < channels; c++) o.push({ value: `m${c}`, label: `Mono ${c + 1}` });
  return o;
}
