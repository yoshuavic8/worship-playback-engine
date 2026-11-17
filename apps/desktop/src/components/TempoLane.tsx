import { useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from "react";
import { toast } from "sonner";
import {
  addPoint,
  barBeatOf,
  beatToTime,
  bpmAtBeat,
  curveAtBeat,
  fmtPoint,
  movePoint,
  qOf,
  removePoint,
  setCurve,
  timeToBeat,
  yScale,
  type Curve,
  type Grid,
} from "@/lib/tempoMap";

interface Props {
  grid: Grid;
  duration: number;
  selectedId: string | null;
  draw: boolean;
  snap: "bar" | "beat" | "off";
  timeAt: (clientX: number) => number;
  onSelect: (id: string | null) => void;
  onStart: () => void;
  onChange: (grid: Grid) => void;
  onEnd: (grid: Grid) => void;
  onRejectDelete: () => void;
}

function gestureSnap(toolbar: "bar" | "beat" | "off", ev: { altKey: boolean; metaKey: boolean; ctrlKey: boolean }): "bar" | "beat" | "free" {
  if (ev.metaKey || ev.ctrlKey) return "free";
  if (ev.altKey) return "bar";
  if (toolbar === "off") return "free";
  if (toolbar === "bar") return "bar";
  return "beat";
}

/** Yellow tempo automation under the ruler. Double-click adds a breakpoint. */
export function TempoLane({ grid, duration, selectedId, draw, snap, timeAt, onSelect, onStart, onChange, onEnd, onRejectDelete }: Props) {
  const [ghost, setGhost] = useState<{ t: number; bpm: number; label: string } | null>(null);
  const drag = useRef<(() => void) | null>(null);

  const points = useMemo(() => {
    const bpb = grid.beatsPerBar;
    return [...grid.points]
      .sort((a, b) => qOf(a.bar, a.beat, bpb) - qOf(b.bar, b.beat, bpb))
      .map((p, i) => {
        const q = i === 0 ? 0 : qOf(p.bar, p.beat, bpb);
        return { ...p, q, t: beatToTime(grid, q) };
      });
  }, [grid]);

  const scale = yScale(points.map((p) => p.bpm));
  const yOf = (bpm: number) => ((scale.hi - bpm) / Math.max(1, scale.hi - scale.lo)) * 100;
  const pct = (t: number) => (duration > 0 ? (Math.max(0, t) / duration) * 100 : 0);

  const line = useMemo(() => {
    if (!points.length || duration <= 0) return "";
    const samples: { t: number; bpm: number }[] = [];
    const push = (t: number, bpm: number) => {
      if (t >= -0.01 && t <= duration + 0.01) samples.push({ t: Math.max(0, Math.min(duration, t)), bpm });
    };
    const qStart = timeToBeat(grid, 0);
    push(0, bpmAtBeat(grid, qStart));
    for (let i = 0; i < points.length; i++) {
      const q0 = points[i].q;
      const q1 = points[i + 1]?.q;
      push(beatToTime(grid, q0), points[i].bpm);
      if (q1 == null) break;
      const t1 = beatToTime(grid, q1);
      if (points[i].curve === "linear") {
        for (let s = 1; s <= 24; s++) {
          const q = q0 + ((q1 - q0) * s) / 24;
          push(beatToTime(grid, q), bpmAtBeat(grid, q));
        }
      } else {
        push(t1, points[i].bpm);
        push(t1, points[i + 1].bpm);
      }
    }
    push(duration, bpmAtBeat(grid, timeToBeat(grid, duration)));
    return samples.map((s) => `${s.t},${scale.hi - s.bpm}`).join(" ");
  }, [grid, points, duration, scale.hi]);

  const snapQ = (t: number, ev: { altKey: boolean; metaKey: boolean; ctrlKey: boolean }) => {
    let q = timeToBeat(grid, t);
    const mode = gestureSnap(snap, ev);
    if (mode === "free") q = Math.round(q * 16) / 16;
    else if (mode === "bar") q = Math.round(q / grid.beatsPerBar) * grid.beatsPerBar;
    else q = Math.round(q);
    return q;
  };

  const place = (t: number, ev: { altKey: boolean; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }) => {
    const mode = gestureSnap(snap, ev);
    const next = addPoint(grid, t, mode);
    if (!next) return;
    const added = next.points.find((p) => !grid.points.some((o) => o.id === p.id));
    onStart();
    onEnd(next);
    if (added) onSelect(added.id);
  };

  const begin = (id: string, e: ReactPointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    onSelect(id);
    const index = points.findIndex((p) => p.id === id);
    const origin = points[index];
    if (!origin) return;
    const start = { x: e.clientX, y: e.clientY, grid, q: origin.q, bpm: origin.bpm, index };
    let axis: "x" | "y" | null = null;
    let latest = grid;
    onStart();
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - start.x;
      const dy = ev.clientY - start.y;
      const freeAxis = ev.shiftKey && (ev.metaKey || ev.ctrlKey);
      if (!axis && !freeAxis && Math.hypot(dx, dy) > 4) axis = Math.abs(dx) > Math.abs(dy) ? "x" : "y";
      let bpm = start.bpm;
      let bar = origin.bar;
      let beat = origin.beat;
      if (axis !== "x") bpm = start.bpm - dy * (ev.shiftKey ? 0.01 : 0.1);
      if (axis !== "y" && start.index > 0) {
        const q = snapQ(timeAt(ev.clientX), ev);
        const pos = barBeatOf(q, grid.beatsPerBar);
        bar = pos.bar;
        beat = pos.beat;
      }
      latest = movePoint(start.grid, id, { bar, beat, bpm });
      onChange(latest);
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      drag.current = null;
      move(ev);
      onEnd(latest);
    };
    drag.current = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const onMove = (e: ReactMouseEvent) => {
    const t = timeAt(e.clientX);
    const q = snapQ(t, e);
    const bpm = bpmAtBeat(grid, q);
    const pos = barBeatOf(Math.round(q), grid.beatsPerBar);
    setGhost({ t: beatToTime(grid, q), bpm, label: fmtPoint(pos.bar, pos.beat, bpm) });
  };

  return (
    <div
      className="relative z-[35] border-b border-line/80 bg-[#141208]"
      style={{ height: 96 }}
      onMouseMove={onMove}
      onMouseLeave={() => setGhost(null)}
      onDoubleClick={(e) => {
        e.stopPropagation();
        place(timeAt(e.clientX), e);
      }}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        if (draw) {
          e.stopPropagation();
          place(timeAt(e.clientX), e);
          return;
        }
        onSelect(null);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        const q = timeToBeat(grid, timeAt(e.clientX));
        const owner = [...points].reverse().find((p) => p.q <= q + 0.01) ?? points[0];
        if (!owner) return;
        const curve: Curve = owner.curve === "linear" ? "step" : "linear";
        onStart();
        onEnd(setCurve(grid, owner.id, curve));
        onSelect(owner.id);
      }}
      onKeyDown={(e) => {
        if ((e.key === "Backspace" || e.key === "Delete") && selectedId) {
          e.preventDefault();
          e.stopPropagation();
          const next = removePoint(grid, selectedId);
          if (!next) {
            onRejectDelete();
            toast.message("Beat 1 keeps its tempo. Delete a later point.");
          }
          else {
            onStart();
            onEnd(next);
            onSelect(null);
          }
        }
      }}
    >
      <svg className="pointer-events-none absolute inset-0 h-full w-full" viewBox={`0 0 ${Math.max(duration, 0.001)} ${scale.hi - scale.lo}`} preserveAspectRatio="none">
        {line && (
          <>
            <polygon points={`0,${scale.hi - scale.lo} ${line} ${duration},${scale.hi - scale.lo}`} fill="rgba(226,179,64,0.13)" />
            <polyline points={line} fill="none" stroke="#e2b340" strokeWidth="2" vectorEffect="non-scaling-stroke" />
          </>
        )}
      </svg>
      <div className="pointer-events-none absolute top-1 left-1 text-[10px] font-semibold text-[#e2b340]/80">{scale.hi}</div>
      <div className="pointer-events-none absolute bottom-1 left-1 text-[10px] font-semibold text-[#e2b340]/50">{scale.lo}</div>
      {ghost && !drag.current && (
        <div className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-1/2" style={{ left: `${pct(ghost.t)}%`, top: `${yOf(ghost.bpm)}%` }}>
          <div className="size-2.5 rounded-full border border-dashed border-[#e2b340]" />
          <div className="absolute top-3 left-3 whitespace-nowrap rounded bg-black/80 px-1.5 py-0.5 text-[10px] text-[#f3e2a4]">Double-click to add point · {ghost.label}</div>
        </div>
      )}
      {points.map((p) => {
        const sel = p.id === selectedId;
        return (
          <button
            key={p.id}
            type="button"
            aria-label={fmtPoint(p.bar, p.beat, p.bpm)}
            title={fmtPoint(p.bar, p.beat, p.bpm)}
            className="absolute z-20 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-[#e2b340] bg-[#1a1408]"
            style={{
              left: `${pct(p.t)}%`,
              top: `${yOf(p.bpm)}%`,
              boxShadow: sel ? "0 0 0 2px #fff" : undefined,
            }}
            onPointerDown={(e) => begin(p.id, e)}
            onDoubleClick={(e) => e.stopPropagation()}
          >
            {sel && (
              <span className="pointer-events-none absolute bottom-4 left-1/2 -translate-x-1/2 whitespace-nowrap rounded bg-white px-1.5 py-0.5 text-[10px] font-bold text-black">
                {fmtPoint(p.bar, p.beat, p.bpm)}
              </span>
            )}
          </button>
        );
      })}
      {points.map((p, i) => {
        const next = points[i + 1];
        const label = p.curve === "linear" && next ? `${p.bpm.toFixed(0)} → ${next.bpm.toFixed(0)}` : p.bpm.toFixed(2);
        return (
          <div key={`lab-${p.id}`} className="pointer-events-none absolute text-[10px] font-semibold text-[#f3e2a4]" style={{ left: `calc(${pct(p.t)}% + 8px)`, top: `calc(${yOf(p.bpm)}% - 16px)` }}>
            {label}
          </div>
        );
      })}
    </div>
  );
}

export function tempoCurveLabel(grid: Grid, id: string | null, atSec: number): "Step" | "Ramp" {
  if (id) {
    const p = grid.points.find((x) => x.id === id);
    if (p) return p.curve === "linear" ? "Ramp" : "Step";
  }
  return curveAtBeat(grid, timeToBeat(grid, atSec)) === "linear" ? "Ramp" : "Step";
}

export function pointTime(grid: Grid, id: string): number | null {
  const bpb = grid.beatsPerBar;
  const ordered = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, bpb) - qOf(b.bar, b.beat, bpb));
  const i = ordered.findIndex((p) => p.id === id);
  if (i < 0) return null;
  return beatToTime(grid, i === 0 ? 0 : qOf(ordered[i].bar, ordered[i].beat, bpb));
}
