import { Pause, Play, Radio, Repeat, Square, Volume1 } from "lucide-react";
import { Metronome } from "./icons";
import { useApp, usePlayback } from "@/store/app";
import { cn, fmtTime } from "@/lib/format";
import { sectionColor } from "@/lib/sections";
import { fadeOut, isRolling, panicStop, stop, toggleClick, toggleLoop, togglePlay } from "@/lib/transport";
import { IconButton, Meter, Tip } from "./ui";

export function PanicButton({ size = "md" }: { size?: "md" | "xl" }) {
  return (
    <Tip label="Instantly silence all outputs (Esc)">
      <button
        onClick={panicStop}
        className={cn(
          "panic-btn shrink-0 rounded-xl font-black tracking-[0.12em] text-white uppercase transition-transform",
          size === "xl" ? "h-16 px-8 text-lg" : "h-9 px-4 text-xs",
        )}
      >
        Panic
      </button>
    </Tip>
  );
}

export function StateBadge({ state, fading }: { state?: string; fading?: boolean }) {
  const map: Record<string, [string, string]> = {
    PLAYING: ["Playing", "text-ok"],
    COUNT_IN: ["Count-in", "text-accent"],
    PAUSED: ["Paused", "text-warn"],
    READY: ["Ready", "text-fg-2"],
    STOPPED: ["Stopped", "text-muted"],
    LOADING: ["Loading", "text-mon"],
    NEXTING: ["Next song", "text-mon"],
    ERROR: ["Error", "text-err"],
    PANIC: ["Panic stop", "text-err"],
  };
  const [label, cls] = map[state ?? "STOPPED"] ?? map.STOPPED;
  return <span className={cn("text-[0.68rem] font-bold tracking-[0.12em] uppercase", cls)}>{fading ? "Fading" : label}</span>;
}

/** Persistent bottom bar: what is loaded, where we are, and the big safety controls. */
export function MiniTransport() {
  const loaded = useApp((s) => s.loaded);
  const navigate = useApp((s) => s.navigate);
  const live = useApp((s) => s.live);
  const status = usePlayback((s) => s.status);
  const rolling = isRolling(status?.state);

  const seg = loaded?.segments[status?.segmentIndex ?? 0];
  const queued = status?.queuedIndex != null ? loaded?.segments[status.queuedIndex] : null;
  const dur = loaded?.duration ?? 0;
  const pos = status?.songId === loaded?.songId ? status?.position ?? 0 : 0;

  return (
    <div className="flex h-[68px] shrink-0 items-center gap-4 border-t border-line bg-bg-2/95 px-4 backdrop-blur">
      <div className="flex items-center gap-1">
        <IconButton label={rolling ? "Pause (Space)" : "Play (Space)"} size="lg" variant="secondary" onClick={togglePlay} disabled={!loaded}
          className={cn("rounded-full", rolling && "bg-accent text-accent-ink hover:bg-accent-2")}>
          {rolling ? <Pause className="size-5" fill="currentColor" /> : <Play className="size-5 translate-x-px" fill="currentColor" />}
        </IconButton>
        <IconButton label="Stop (S)" onClick={stop} disabled={!loaded}><Square className="size-4" fill="currentColor" /></IconButton>
        <IconButton label="Fade out (F)" onClick={() => fadeOut(4)} disabled={!rolling}><Volume1 className="size-4" /></IconButton>
      </div>

      <div className="min-w-0 flex-1">
        {loaded ? (
          <div className="flex min-w-0 items-center gap-4">
            <button className="min-w-0 max-w-[16rem] text-left" onClick={() => navigate(live && loaded.context === "live" ? "live" : "song", loaded.songId)}>
              <div className="flex items-center gap-2">
                <StateBadge state={status?.state} fading={status?.fading} />
                {loaded.context === "live" && <span className="flex items-center gap-1 text-[0.65rem] font-bold text-live"><Radio className="size-3" />LIVE</span>}
              </div>
              <div className="truncate text-[0.92rem] font-semibold">{loaded.title}</div>
            </button>
            <div className="min-w-0 flex-1">
              <div className="mb-1 flex items-center justify-between text-xs">
                <span className="flex min-w-0 items-center gap-2">
                  {seg && <span className="truncate font-semibold" style={{ color: sectionColor(seg.kind) }}>{seg.label}</span>}
                  {queued && <span className="truncate text-mon">→ {queued.label}</span>}
                  {status?.looping && <span className="flex items-center gap-1 text-accent"><Repeat className="size-3" />Loop</span>}
                </span>
                <span className="tnum shrink-0 text-muted">{fmtTime(pos)} / {fmtTime(dur)}</span>
              </div>
              <div className="relative h-1.5 overflow-hidden rounded-full bg-line">
                {loaded.segments.map((s, i) => (
                  <div key={i} className="absolute top-0 bottom-0 border-r border-bg-2" style={{
                    left: `${(segOffset(loaded.segments, i) / planLen(loaded.segments)) * 100}%`,
                    width: `${((s.end - s.start) / planLen(loaded.segments)) * 100}%`,
                    background: sectionColor(s.kind), opacity: i === status?.segmentIndex ? 0.5 : 0.18,
                  }} />
                ))}
                <div className="absolute top-0 bottom-0 left-0 bg-white/80" style={{ width: `${Math.min(100, (planPos(loaded.segments, status?.segmentIndex ?? 0, pos) / planLen(loaded.segments)) * 100)}%` }} />
              </div>
            </div>
          </div>
        ) : (
          <div className="text-sm text-muted">Nothing loaded — open a song or start Live mode.</div>
        )}
      </div>

      <div className="flex items-center gap-1">
        <IconButton label="Loop current section (L)" active={status?.looping} onClick={toggleLoop} disabled={!loaded}><Repeat className="size-4" /></IconButton>
        <IconButton label={loaded?.clickAvailable ? "Click on/off (C)" : "Click unavailable — no BPM"} active={loaded?.clickEnabled} onClick={toggleClick} disabled={!loaded}>
          <Metronome className="size-4" />
        </IconButton>
      </div>

      <BusMeters meters={status?.meters} />
      <PanicButton />
    </div>
  );
}

export function BusMeters({ meters, className }: { meters?: number[]; className?: string }) {
  const m = meters ?? [0, 0, 0, 0];
  return (
    <div className={cn("grid w-36 grid-cols-[auto_1fr] items-center gap-x-2 gap-y-1", className)}>
      <span className="text-[0.62rem] font-bold tracking-wider text-foh">FOH</span>
      <div className="space-y-0.5"><Meter value={m[0]} className="h-1" /><Meter value={m[1]} className="h-1" /></div>
      <span className="text-[0.62rem] font-bold tracking-wider text-mon">MON</span>
      <div className="space-y-0.5"><Meter value={m[2]} className="h-1" /><Meter value={m[3]} className="h-1" /></div>
    </div>
  );
}

// Position along the arranged plan (segments may repeat or jump around the file).
export const planLen = (segs: { start: number; end: number }[]) => Math.max(0.001, segs.reduce((a, s) => a + (s.end - s.start), 0));
export const segOffset = (segs: { start: number; end: number }[], i: number) => segs.slice(0, i).reduce((a, s) => a + (s.end - s.start), 0);
export function planPos(segs: { start: number; end: number }[], i: number, filePos: number) {
  if (!segs.length) return filePos;
  const s = segs[Math.min(i, segs.length - 1)];
  return segOffset(segs, i) + Math.max(0, Math.min(s.end - s.start, filePos - s.start));
}
