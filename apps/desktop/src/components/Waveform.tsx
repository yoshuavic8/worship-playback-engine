import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { cn } from "@/lib/format";
import { sectionColor, type ResolvedSection } from "@/lib/sections";
import type { Waveform as WaveformData } from "@/lib/types";

interface Props {
  data: WaveformData | null;
  duration: number;
  sections?: ResolvedSection[];
  position?: number;
  height?: number;
  onSeek?: (t: number) => void;
  /** Editor mode: draggable boundaries (not the first). */
  onMoveMarker?: (id: string, t: number) => void;
  onSelectSection?: (id: string) => void;
  selectedId?: string | null;
  beatGrid?: { bpm: number; offset: number; beatsPerBar: number } | null;
  className?: string;
  activeSectionId?: string | null;
}

export function Waveform({
  data, duration, sections = [], position, height = 120, onSeek, onMoveMarker, onSelectSection, selectedId,
  beatGrid, className, activeSectionId,
}: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(800);
  const [drag, setDrag] = useState<{ id: string; t: number } | null>(null);
  const [hover, setHover] = useState<number | null>(null);

  useLayoutEffect(() => {
    if (!wrap.current) return;
    const ro = new ResizeObserver((e) => setWidth(Math.max(100, e[0].contentRect.width)));
    ro.observe(wrap.current);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.floor(width * dpr);
    c.height = Math.floor(height * dpr);
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, width, height);
    const mid = height / 2;

    if (beatGrid && beatGrid.bpm > 0 && duration > 0) {
      const beat = 60 / beatGrid.bpm;
      const pxPerBeat = (beat / duration) * width;
      if (pxPerBeat > 3) {
        let k = Math.ceil(-beatGrid.offset / beat);
        for (let t = beatGrid.offset + k * beat; t < duration; t += beat, k++) {
          const x = (t / duration) * width;
          const bar = ((k % beatGrid.beatsPerBar) + beatGrid.beatsPerBar) % beatGrid.beatsPerBar === 0;
          if (!bar && pxPerBeat < 7) continue;
          g.fillStyle = bar ? "rgba(255,255,255,0.09)" : "rgba(255,255,255,0.035)";
          g.fillRect(Math.round(x), 0, 1, height);
        }
      }
    }

    if (!data || !data.peaks.length) {
      g.fillStyle = "rgba(255,255,255,0.06)";
      g.fillRect(0, mid - 0.5, width, 1);
      return;
    }
    const n = data.peaks.length;
    const bars = Math.floor(width / 2);
    const per = n / bars;
    const played = position != null && duration > 0 ? (position / duration) * width : -1;
    for (let i = 0; i < bars; i++) {
      const a = Math.floor(i * per);
      const b = Math.max(a + 1, Math.floor((i + 1) * per));
      let pk = 0;
      let rms = 0;
      for (let j = a; j < b && j < n; j++) {
        if (data.peaks[j] > pk) pk = data.peaks[j];
        if (data.rms[j] > rms) rms = data.rms[j];
      }
      const x = i * 2;
      const ph = Math.max(1, (pk / 255) * (height * 0.46));
      const rh = Math.max(1, (rms / 255) * (height * 0.46) * 1.35);
      const isPlayed = x < played;
      g.fillStyle = isPlayed ? "rgba(244,183,64,0.45)" : "rgba(180,186,196,0.28)";
      g.fillRect(x, mid - ph, 1.4, ph * 2);
      g.fillStyle = isPlayed ? "rgba(255,207,107,0.95)" : "rgba(214,219,226,0.62)";
      g.fillRect(x, mid - Math.min(rh, ph), 1.4, Math.min(rh, ph) * 2);
    }
  }, [data, width, height, position, duration, beatGrid]);

  const tAt = (clientX: number) => {
    const r = wrap.current!.getBoundingClientRect();
    return Math.max(0, Math.min(duration, ((clientX - r.left) / r.width) * duration));
  };

  const startDrag = (id: string, e: React.PointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    const move = (ev: PointerEvent) => setDrag({ id, t: tAt(ev.clientX) });
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      setDrag(null);
      onMoveMarker?.(id, tAt(ev.clientX));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    setDrag({ id, t: tAt(e.clientX) });
  };

  const pct = (t: number) => `${duration > 0 ? (t / duration) * 100 : 0}%`;

  return (
    <div
      ref={wrap}
      className={cn("relative w-full overflow-hidden rounded-xl bg-[#0b0c0e] ring-1 ring-line", onSeek && "cursor-pointer", className)}
      style={{ height }}
      onPointerDown={(e) => onSeek && e.button === 0 && onSeek(tAt(e.clientX))}
      onPointerMove={(e) => setHover(tAt(e.clientX))}
      onPointerLeave={() => setHover(null)}
    >
      {sections.map((s) => {
        const start = drag?.id === s.id ? drag.t : s.start;
        const color = sectionColor(s.kind);
        const sel = selectedId === s.id || activeSectionId === s.id;
        return (
          <div
            key={s.id}
            className="absolute top-0 bottom-0"
            style={{ left: pct(start), width: pct(Math.max(0, s.end - start)) }}
            onPointerDown={(e) => {
              if (onSelectSection) {
                onSelectSection(s.id);
                if (!onSeek) e.stopPropagation();
              }
            }}
          >
            <div className="absolute inset-0" style={{ background: color, opacity: sel ? 0.13 : 0.05 }} />
            <div className="absolute inset-x-0 top-0 h-[3px]" style={{ background: color, opacity: sel ? 1 : 0.55 }} />
            <div
              className="pointer-events-none absolute top-1.5 left-1.5 truncate rounded px-1.5 py-px text-[0.68rem] font-semibold"
              style={{ color, background: "rgba(10,11,13,0.75)", maxWidth: "calc(100% - 8px)" }}
            >
              {s.label}
            </div>
          </div>
        );
      })}
      <canvas ref={canvas} className="pointer-events-none absolute inset-0" style={{ width, height }} />
      {onMoveMarker &&
        sections.slice(1).map((s) => {
          const t = drag?.id === s.id ? drag.t : s.start;
          return (
            <div
              key={`m-${s.id}`}
              className="group absolute top-0 bottom-0 z-10 w-3 -translate-x-1/2 cursor-ew-resize"
              style={{ left: pct(t) }}
              onPointerDown={(e) => startDrag(s.id, e)}
              title="Drag to move this boundary"
            >
              <div className="mx-auto h-full w-[2px] bg-white/40 group-hover:bg-white" style={{ background: drag?.id === s.id ? "white" : undefined }} />
            </div>
          );
        })}
      {position != null && duration > 0 && (
        <div className="pointer-events-none absolute top-0 bottom-0 z-20 w-[2px] bg-white shadow-[0_0_8px_rgba(255,255,255,0.7)]" style={{ left: pct(position) }} />
      )}
      {hover != null && onSeek && (
        <div className="pointer-events-none absolute top-0 bottom-0 z-10 w-px bg-white/25" style={{ left: pct(hover) }} />
      )}
      {drag && (
        <div className="tnum pointer-events-none absolute bottom-1.5 z-30 -translate-x-1/2 rounded bg-white px-1.5 text-[0.7rem] font-semibold text-black" style={{ left: pct(drag.t) }}>
          {formatShort(drag.t)}
        </div>
      )}
    </div>
  );
}

function formatShort(t: number) {
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  return `${m}:${s.toFixed(1).padStart(4, "0")}`;
}
