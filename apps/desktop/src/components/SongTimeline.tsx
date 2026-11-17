import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode, type RefObject } from "react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { ArrowLeft, Check, ChevronDown, Crosshair, HelpCircle, Loader2, Pause, Play, Repeat, RotateCcw, Save, SlidersHorizontal, Sparkles, Square, Undo2 } from "lucide-react";
import { SongStatusBadge } from "@/components/SongBits";
import { TapTempo } from "@/components/TapTempo";
import { TempoLane, pointTime, tempoCurveLabel } from "@/components/TempoLane";
import { Button, IconButton, Tip } from "@/components/ui";
import { KEYS, MINOR_KEYS } from "@/lib/keys";
import { clampTimelineZoom } from "@/lib/editorZoom";
import { cn, fmtTime } from "@/lib/format";
import { beatsPerBar, meterChoices, meterHint } from "@/lib/meter";
import { sectionColor, sectionShort, type ResolvedSection } from "@/lib/sections";
import { barBeatOf, beatToTime, beatsInRange, bpmAtTime, ensureGrid, qOf, timeToBeat, type Grid } from "@/lib/tempoMap";
import type { SectionKind, Song, TrackInfo, TrackRole, Waveform as WaveformData } from "@/lib/types";

export type SnapMode = "bar" | "beat" | "off";

const H = { ruler: 34, tempo: 96, arr: 62, cues: 40, click: 34, stem: 48 } as const;

const STEMS: { role: TrackRole; name: string; color: string }[] = [
  { role: "drums", name: "Drums", color: "#e07a4a" },
  { role: "bass", name: "Bass", color: "#6aa2ff" },
  { role: "other", name: "Keys / guitars", color: "#3dbe7a" },
  { role: "vocals", name: "Guide vocal", color: "#c084fc" },
];

const KIND_CHIPS: { kind: SectionKind; short: string }[] = [
  { kind: "intro", short: "Intro" },
  { kind: "verse", short: "Verse" },
  { kind: "prechorus", short: "Pre" },
  { kind: "chorus", short: "Chorus" },
  { kind: "bridge", short: "Bridge" },
  { kind: "instrumental", short: "Inst" },
  { kind: "tag", short: "Tag" },
  { kind: "outro", short: "Outro" },
  { kind: "other", short: "Other" },
];

export interface TimelineActions {
  onBack: () => void;
  onTitle: (title: string) => void;
  onArtist: (artist: string) => void;
  onPlay: () => void;
  onStop: () => void;
  onLoop: () => void;
  onSeek: (t: number) => void;
  onSelect: (id: string) => void;
  onAddSection: (t: number) => void;
  onMoveSection: (id: string, t: number) => void;
  onRename: (id: string, label: string) => void;
  onRenameFocus: (id: string) => void;
  onKind: (id: string, kind: string) => void;
  onDelete: (id: string) => void;
  onMoveBeat: (t: number) => void;
  onBpm: (bpm: number) => void;
  onKey: (key: string | null) => void;
  onTimeSignature: (timeSignature: string) => void;
  onTempoStart: () => void;
  onTempo: (grid: Grid) => void;
  onTempoEnd: (grid: Grid) => void;
  onTempoPoint: (id: string | null) => void;
  onToggleTempoCurve: () => void;
  onCountIn: (bars: number) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSnap: () => void;
  onSnapAll: () => void;
  onSave: () => void;
  onRevert: () => void;
  onDone: () => void;
  onPrepare: () => void;
  onMix: () => void;
  onMute: (role: TrackRole, muted: boolean) => void;
  onSolo: (role: TrackRole | null) => void;
  onClickToggle: (on: boolean) => void;
  onCueToggle: (on: boolean) => void;
  onZoom: (t: number, zoom: number) => void;
}

interface Props {
  song: Song;
  sections: ResolvedSection[];
  wave: WaveformData | null;
  playhead: number;
  rolling: boolean;
  loaded: boolean;
  looping: boolean;
  loopRegion: { start: number; end: number } | null;
  zoom: number;
  scrollerRef: RefObject<HTMLDivElement | null>;
  selectedId: string | null;
  canUndo: boolean;
  canRedo: boolean;
  snap: SnapMode;
  saving: boolean;
  dirty: boolean;
  tracks: TrackInfo[];
  clickOn: boolean;
  cueOn: boolean;
  preparing: boolean;
  activeSectionId: string | null;
  solo: TrackRole | null;
  tempoPoint: string | null;
  drawTempo: boolean;
  actions: TimelineActions;
}

export function SongTimeline(props: Props) {
  const { song, sections, wave, zoom, scrollerRef, actions } = props;
  const duration = song.duration;
  const bpb = beatsPerBar(song.timeSignature);
  const [beatDrag, setBeatDrag] = useState<number | null>(null);
  const [edgeDrag, setEdgeDrag] = useState<{ id: string; t: number } | null>(null);
  const [view, setView] = useState({ start: 0, span: duration, width: 800 });
  const dragCleanup = useRef<(() => void) | null>(null);

  const offset = beatDrag ?? song.beatOffset;
  const storedGrid = ensureGrid({ ...song, beatOffset: offset, timeSignature: song.timeSignature });
  const grid = storedGrid ? { ...storedGrid, beat1Sec: offset, beatsPerBar: bpb } : null;
  const localBpm = grid ? bpmAtTime(grid, props.playhead || offset) : song.bpm;
  const beatSec = localBpm && localBpm > 0 ? 60 / localBpm : null;
  const barSec = beatSec ? beatSec * bpb : null;

  useEffect(() => () => dragCleanup.current?.(), []);

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    let raf = 0;
    const update = () => {
      const width = Math.max(1, Math.round(el.clientWidth));
      const z = Math.max(1, zoom);
      const span = duration > 0 ? duration / z : 0;
      const start = duration > 0 && el.scrollWidth > 0 ? (el.scrollLeft / el.scrollWidth) * duration : 0;
      setView((v) => (Math.abs(v.start - start) < 0.0005 && Math.abs(v.span - span) < 0.0005 && v.width === width ? v : { start, span, width }));
    };
    const onScroll = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(update);
    };
    update();
    el.addEventListener("scroll", onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(el);
    return () => {
      cancelAnimationFrame(raf);
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
    };
  }, [scrollerRef, zoom, duration]);

  const timeAt = (clientX: number) => {
    const el = scrollerRef.current;
    if (!el || duration <= 0) return 0;
    const rect = el.getBoundingClientRect();
    const x = clientX - rect.left;
    return Math.max(0, Math.min(duration, ((el.scrollLeft + x) / Math.max(1, el.scrollWidth)) * duration));
  };

  const snapTime = (t: number, ev: { altKey: boolean; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }) => {
    let mode = props.snap;
    if (ev.metaKey || ev.ctrlKey) mode = "off";
    else if (ev.altKey || ev.shiftKey) mode = "beat";
    if (mode === "off" || !grid) return Math.round(t * 1000) / 1000;
    let q = timeToBeat(grid, t);
    q = mode === "bar" ? Math.round(q / bpb) * bpb : Math.round(q);
    const snapped = beatToTime(grid, q);
    return Math.max(0, Math.min(duration, Math.round(snapped * 1000) / 1000));
  };

  const clampEdge = (id: string, t: number) => {
    const sorted = [...song.sections].sort((a, b) => a.start - b.start);
    const i = sorted.findIndex((s) => s.id === id);
    if (i <= 0) return sorted[0]?.start ?? 0;
    const min = sorted[i - 1].start + 0.05;
    const max = (sorted[i + 1]?.start ?? duration) - 0.05;
    if (max <= min) return sorted[i].start;
    return Math.max(min, Math.min(max, t));
  };

  const beginEdge = (id: string, e: ReactPointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    const apply = (ev: PointerEvent) => clampEdge(id, snapTime(timeAt(ev.clientX), ev));
    const move = (ev: PointerEvent) => setEdgeDrag({ id, t: apply(ev) });
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      dragCleanup.current = null;
      const t = apply(ev);
      setEdgeDrag(null);
      actions.onMoveSection(id, t);
    };
    dragCleanup.current = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    setEdgeDrag({ id, t: apply(e.nativeEvent) });
  };

  const beginBeat = (e: ReactPointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    const originX = e.clientX;
    let moved = false;
    const apply = (ev: PointerEvent) => Math.max(0, Math.min(duration, Math.round(timeAt(ev.clientX) * 1000) / 1000));
    const move = (ev: PointerEvent) => {
      if (!moved && Math.abs(ev.clientX - originX) <= 3) return;
      moved = true;
      setBeatDrag(apply(ev));
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      dragCleanup.current = null;
      if (!moved) return;
      const t = apply(ev);
      setBeatDrag(null);
      actions.onMoveBeat(t);
    };
    dragCleanup.current = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const shown = sections.map((s, i) => {
    const start = edgeDrag?.id === s.id ? edgeDrag.t : s.start;
    const next = sections[i + 1];
    const end = next ? (edgeDrag?.id === next.id ? edgeDrag.t : next.start) : duration;
    return { ...s, start, end };
  });

  const pct = (t: number) => `${duration > 0 ? (Math.max(0, t) / duration) * 100 : 0}%`;
  const pxPerBeat = beatSec && duration > 0 ? (beatSec / duration) * view.width * Math.max(1, zoom) : 0;
  const realSections = shown.filter((s) => s.id !== "__song");
  const selected = realSections.find((s) => s.id === props.selectedId) ?? null;
  const separated = (role: TrackRole) => !!song.stems[role as keyof typeof song.stems];
  const stemMuted = (role: TrackRole) => {
    const t = props.tracks.find((x) => x.role === role);
    if (t) return t.muted;
    if (role === "drums") return song.prep.drums !== "keep";
    if (role === "bass") return song.prep.bass === "remove";
    if (role === "vocals") return !song.routing.guideVocalToMonitor;
    return false;
  };

  const seekAt = (e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    actions.onSeek(timeAt(e.clientX));
  };

  const gridLabel = !grid
    ? "No grid"
    : grid.source === "user"
      ? (grid.points.length <= 1 ? "Fixed" : `Tempo map · ${grid.points.length} points`)
      : song.analysis?.tempoMode === "changing" ? "Changing"
      : song.analysis?.tempoMode === "drifting" ? "Drifting"
      : grid.points.length <= 1 ? "Fixed" : `Tempo map · ${grid.points.length} points`;
  const notPrepared = song.status === "imported" || song.status === "error";

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-bg text-fg">
      <header className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-line bg-bg-2 px-3 py-2">
        <IconButton label="Back" onClick={actions.onBack}><ArrowLeft className="size-4" /></IconButton>
        <div className="min-w-0 w-52">
          <input className="w-full truncate bg-transparent text-base font-semibold tracking-tight outline-none" value={song.title}
            onChange={(e) => actions.onTitle(e.target.value)} aria-label="Song title" />
          <div className="flex items-center gap-2">
            <input className="w-full truncate bg-transparent text-xs text-muted outline-none" value={song.artist}
              onChange={(e) => actions.onArtist(e.target.value)} aria-label="Artist" />
          </div>
        </div>
        <SongStatusBadge song={song} />
        <div className="flex items-center gap-1.5">
          <button type="button" aria-label={props.rolling ? "Pause" : "Play"} onClick={actions.onPlay}
            className="grid size-9 place-items-center rounded-full bg-accent text-accent-ink hover:bg-accent-2">
            {props.rolling ? <Pause className="size-4" fill="currentColor" /> : <Play className="size-4 translate-x-px" fill="currentColor" />}
          </button>
          <IconButton label="Stop" onClick={actions.onStop} disabled={!props.loaded}><Square className="size-3.5" fill="currentColor" /></IconButton>
          <button type="button" onClick={actions.onLoop} aria-pressed={props.looping}
            className={cn("flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-xs font-semibold", props.looping ? "border-accent bg-accent/15 text-accent" : "border-line text-fg-2 hover:border-line-2")}>
            <Repeat className="size-3.5" /> Loop
          </button>
        </div>
        <div className="min-w-[5.5rem]">
          <div className="tnum text-xl font-bold leading-none">{grid ? compactFromGrid(grid, props.playhead) : "—"}</div>
          <div className="tnum text-[0.7rem] text-muted">{fmtTime(props.loaded ? props.playhead : 0, true)}</div>
        </div>
        <Pill className="hidden sm:flex" label="Grid" value={gridLabel} />
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" variant="ghost" icon={<Undo2 className="size-3.5" />} disabled={!props.canUndo} onClick={actions.onUndo}>Undo</Button>
          <Button size="sm" variant="ghost" disabled={!props.canRedo} onClick={actions.onRedo}>Redo</Button>
          <SnapMenu snap={props.snap} grid={!!grid} onSnap={actions.onSnap} onSnapAll={actions.onSnapAll} />
          <EditorShortcutsMenu />
          <Button size="sm" variant="outline" icon={<SlidersHorizontal className="size-3.5" />} onClick={actions.onMix}>Mix</Button>
          {props.dirty && <Button size="sm" variant="ghost" icon={<RotateCcw className="size-3.5" />} onClick={actions.onRevert}>Revert</Button>}
          <Button size="sm" variant={props.dirty ? "primary" : "secondary"} icon={<Save className="size-3.5" />} loading={props.saving} disabled={!props.dirty} onClick={actions.onSave}>Save</Button>
          <Button size="sm" variant={notPrepared ? "primary" : "outline"}
            icon={props.preparing ? <Loader2 className="size-3.5 animate-spin" /> : <Sparkles className="size-3.5" />}
            disabled={props.preparing} onClick={actions.onPrepare}>
            {notPrepared ? "Prepare" : props.preparing ? "Preparing…" : "Prepare"}
          </Button>
          <Button size="sm" variant="primary" icon={<Check className="size-3.5" />} onClick={actions.onDone}>Done</Button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 overflow-y-auto">
          <div className="sticky left-0 z-10 w-40 shrink-0 border-r border-line bg-[#111215]">
            <Head h={H.ruler}>
              <span className="text-xs font-semibold text-muted">Time</span>
            </Head>
            <Head h={H.tempo}>
              <span className="min-w-0 flex-1 text-sm font-semibold">Tempo</span>
              <button type="button" disabled={!grid} className="rounded border border-[#e2b340]/50 px-1.5 py-0.5 text-[10px] font-bold text-[#e2b340] disabled:opacity-40"
                onClick={actions.onToggleTempoCurve} title="Step holds the tempo. Ramp glides to the next point.">
                {grid ? tempoCurveLabel(grid, props.tempoPoint, props.playhead) : "Step"}
              </button>
            </Head>
            <Head h={H.arr}><span className="text-sm font-semibold">Arrangement</span></Head>
            <Head h={H.cues}>
              <span className="min-w-0 flex-1 truncate text-sm font-semibold">Cues</span>
              <Ms muted={!props.cueOn} onMute={() => actions.onCueToggle(!props.cueOn)} />
            </Head>
            <Head h={H.click}>
              <span className="min-w-0 flex-1 truncate text-sm font-semibold">Click</span>
              <Ms muted={!props.clickOn} onMute={() => actions.onClickToggle(!props.clickOn)} />
            </Head>
            {STEMS.map((s) => {
              const ok = separated(s.role);
              const muted = stemMuted(s.role);
              return (
                <Head key={s.role} h={H.stem}>
                  <span className="size-2 shrink-0 rounded-full" style={{ background: s.color }} />
                  <span className={cn("min-w-0 flex-1 truncate text-sm font-semibold", !ok && "text-muted")} title={ok ? "Peaks are the full mix. Mute and solo control this stem while rehearsing." : "Prepare the song to separate this stem"}>
                    {s.name}
                  </span>
                  <Ms muted={muted} solo={props.solo === s.role} disabled={!ok}
                    onMute={() => actions.onMute(s.role, !muted)}
                    onSolo={() => actions.onSolo(props.solo === s.role ? null : s.role)} />
                </Head>
              );
            })}
          </div>

          <div ref={scrollerRef} className="min-w-0 flex-1 overflow-x-auto bg-[#0c0d0f]">
            <div className="relative" style={{ width: `${zoom * 100}%`, minWidth: "100%" }}>
              <div className="relative border-b border-line" style={{ height: H.ruler }} onPointerDown={seekAt}>
                {grid && duration > 0 && <MapRuler grid={grid} duration={duration} pxPerBeat={pxPerBeat} />}
                {props.looping && props.loopRegion && (
                  <div className="pointer-events-none absolute top-0 bottom-0 z-[3] border-x-2 border-accent bg-accent/20" style={{ left: pct(props.loopRegion.start), width: pct(props.loopRegion.end - props.loopRegion.start) }} />
                )}
              </div>

              {grid ? (
                <TempoLane
                  grid={grid}
                  duration={duration}
                  selectedId={props.tempoPoint}
                  draw={props.drawTempo}
                  snap={props.snap}
                  timeAt={timeAt}
                  onSelect={actions.onTempoPoint}
                  onStart={actions.onTempoStart}
                  onChange={actions.onTempo}
                  onEnd={actions.onTempoEnd}
                  onRejectDelete={() => {}}
                />
              ) : (
                <div className="relative z-[35] border-b border-line/80 bg-[#141208]" style={{ height: H.tempo }}>
                  <div className="pointer-events-none absolute inset-0 grid place-items-center px-3 text-center text-xs text-faint">
                    Set tempo in the Beat grid panel →
                  </div>
                </div>
              )}

              <div className="relative border-b border-line/80" style={{ height: H.arr }}
                onPointerDown={seekAt}
                onDoubleClick={(e) => { e.stopPropagation(); actions.onAddSection(timeAt(e.clientX)); }}>
                {realSections.length === 0 && (
                  <div className="pointer-events-none absolute inset-0 grid place-items-center text-xs text-faint">Double-click to add a section</div>
                )}
                {realSections.map((s) => {
                  const color = sectionColor(s.kind);
                  const sel = s.id === props.selectedId;
                  const active = s.id === props.activeSectionId;
                  const clipPx = duration > 0 ? ((s.end - s.start) / duration) * view.width * Math.max(1, zoom) : 0;
                  const bars = clipPx >= 72 && grid ? mapSpan(grid, s.start, s.end) : null;
                  const title = clipPx < 64 ? sectionShort(s.label) : s.label;
                  return (
                    <div key={s.id} role="button" tabIndex={0} aria-label={s.label}
                      className="absolute top-1.5 bottom-1.5 overflow-hidden rounded-md border px-2 py-1"
                      style={{
                        left: pct(s.start),
                        width: pct(Math.max(0.05, s.end - s.start)),
                        borderColor: color,
                        background: `color-mix(in oklab, ${color} ${sel || active ? 28 : 16}%, transparent)`,
                        color,
                        boxShadow: sel ? "inset 0 0 0 2px #fff" : undefined,
                      }}
                      onPointerDown={(e) => { e.stopPropagation(); actions.onTempoPoint(null); actions.onSelect(s.id); }}
                    >
                      <div className="truncate text-xs font-bold leading-tight">{title}</div>
                      {bars && <div className="truncate text-[10px] font-medium opacity-70">{bars}</div>}
                    </div>
                  );
                })}
                {realSections.slice(1).map((s) => (
                  <div key={`edge-${s.id}`} role="separator" aria-label={`Move start of ${s.label}`}
                    className="absolute top-1.5 bottom-1.5 z-20 w-3 -translate-x-1/2 cursor-ew-resize"
                    style={{ left: pct(edgeDrag?.id === s.id ? edgeDrag.t : s.start) }}
                    title="Drag the boundary · Alt snaps to the beat · ⌘ drags freely"
                    onPointerDown={(e) => beginEdge(s.id, e)}>
                    <div className="mx-auto h-full w-[2px] bg-white/80" />
                  </div>
                ))}
              </div>

              <div className="relative border-b border-line/80" style={{ height: H.cues }} onPointerDown={seekAt}>
                {realSections.map((s) => {
                  if (!grid || s.start <= 0.05) return null;
                  const t = beatToTime(grid, timeToBeat(grid, s.start) - song.cueLeadBeats);
                  if (t <= 0.05) return null;
                  return (
                    <div key={s.id} className="pointer-events-none absolute top-2 h-6 -translate-x-full whitespace-nowrap rounded-full border border-[#2f5b88] bg-[#1d3550] px-2.5 text-[11px] font-bold leading-6 text-[#8cc4ff]" style={{ left: pct(s.start) }}>
                      {sectionShort(s.label)} · {song.cueLeadBeats}
                    </div>
                  );
                })}
              </div>

              <div className="relative border-b border-line/80" style={{ height: H.click }} onPointerDown={seekAt}>
                {grid && duration > 0 ? (
                  <ClickTicks grid={grid} duration={duration} every={pxPerBeat >= 8 ? 1 : bpb} />
                ) : (
                  <div className="grid h-full place-items-center text-[11px] text-faint">Set a tempo to hear the click</div>
                )}
              </div>

              {STEMS.map((s) => {
                const ok = separated(s.role);
                return (
                  <div key={s.role} className="relative border-b border-line/80" style={{ height: H.stem }} onPointerDown={seekAt}>
                    <PeakLane wave={wave} duration={duration} view={view} height={H.stem} color={s.color} role={s.role} dim={stemMuted(s.role) || !ok} />
                    {!ok && (
                      <div className="pointer-events-none absolute inset-0 grid place-items-center bg-bg/40 text-[11px] font-medium text-muted">
                        Prepare to separate
                      </div>
                    )}
                  </div>
                );
              })}

              {offset > 0.15 && duration > 0 && (
                <div className="pointer-events-none absolute top-0 bottom-0 z-[2] border-r border-dashed border-white/30 bg-[repeating-linear-gradient(45deg,rgba(255,255,255,0.035)_0_8px,transparent_8px_16px)]" style={{ left: 0, width: pct(offset) }}>
                  <span className="absolute top-1 left-1 text-[10px] font-semibold text-faint">pre-roll</span>
                </div>
              )}
              {duration > 0 && (
                <div className="pointer-events-none absolute top-0 bottom-0 z-30 w-3 -translate-x-1/2" style={{ left: pct(offset) }}>
                  <div className="pointer-events-auto absolute inset-x-0 bottom-0 cursor-ew-resize" style={{ top: H.ruler + H.tempo }} onPointerDown={beginBeat} title="Drag Beat 1" />
                  <div className="pointer-events-none absolute inset-y-0 left-1/2 w-[2px] -translate-x-1/2 bg-accent" />
                  <div className="pointer-events-none absolute bottom-2 left-2 whitespace-nowrap rounded bg-accent px-1.5 py-0.5 text-[10px] font-bold text-accent-ink">Beat 1</div>
                </div>
              )}
              {props.tempoPoint && grid && pointTime(grid, props.tempoPoint) != null && (
                <div className="pointer-events-none absolute top-0 bottom-0 z-20 border-l border-dashed border-[#e2b340]/80" style={{ left: pct(pointTime(grid, props.tempoPoint) ?? 0) }} />
              )}
              {props.loaded && duration > 0 && (
                <div className="pointer-events-none absolute top-0 bottom-0 z-40 w-[2px] -translate-x-1/2 bg-white shadow-[0_0_10px_rgba(255,255,255,0.65)]" style={{ left: pct(props.playhead) }} />
              )}
              {edgeDrag && (
                <div className="tnum pointer-events-none absolute top-9 z-50 -translate-x-1/2 rounded bg-white px-1.5 text-[0.7rem] font-semibold text-black" style={{ left: pct(edgeDrag.t) }}>
                  {fmtTime(edgeDrag.t, true)}
                </div>
              )}
            </div>
          </div>
        </div>

        <Inspector
          song={song}
          selected={selected}
          grid={grid}
          offset={offset}
          beatSec={beatSec}
          barSec={barSec}
          tempoPoint={props.tempoPoint}
          playhead={props.playhead}
          onTempoPoint={actions.onTempoPoint}
          onToggleTempoCurve={actions.onToggleTempoCurve}
          onRename={(label) => selected && actions.onRename(selected.id, label)}
          onRenameFocus={() => selected && actions.onRenameFocus(selected.id)}
          onKind={(kind) => selected && actions.onKind(selected.id, kind)}
          onDelete={() => selected && actions.onDelete(selected.id)}
          onBpm={actions.onBpm}
          onKey={actions.onKey}
          onTimeSignature={actions.onTimeSignature}
          onMoveBeat={actions.onMoveBeat}
          loaded={props.loaded}
        />
      </div>

      <footer className="flex h-9 shrink-0 items-center gap-4 border-t border-line bg-bg-2 px-3 text-xs text-muted">
        <button type="button" className="hover:text-fg" onClick={() => actions.onCountIn([0, 1, 2, 4][([0, 1, 2, 4].indexOf(song.countInBars) + 1) % 4])}>
          Count-in {song.countInBars === 0 ? "off" : song.countInBars === 1 ? "1 bar" : `${song.countInBars} bars`}
        </button>
        <span>Snap {props.snap === "off" ? "off" : props.snap === "bar" ? "bar" : "beat"}</span>
        <span className="hidden md:inline">Pinch or slide two fingers up/down to zoom · sideways to move</span>
        <span className="ml-auto flex items-center gap-1">
          <span className="tnum mr-1 font-semibold text-fg">{formatZoom(zoom)}</span>
          {[1, 4, 16, 32].map((z) => (
            <button key={z} type="button" onClick={() => actions.onZoom(props.playhead, z)}
              className={cn("h-6 rounded px-1.5 font-semibold", Math.abs(zoom - z) < 0.05 ? "bg-white/10 text-fg" : "hover:text-fg")}>{z}×</button>
          ))}
          <button type="button" className="h-6 rounded px-1.5 font-semibold hover:text-fg" onClick={() => {
            const twoBars = Math.max(0.2, (beatSec ?? 0.5) * bpb * 2);
            actions.onZoom(offset, clampTimelineZoom(Math.max(zoom, duration / twoBars)));
          }}>Beat 1</button>
        </span>
      </footer>
    </div>
  );
}

function MeterButtons({ value, onChange, compact }: { value: string; onChange: (timeSignature: string) => void; compact?: boolean }) {
  return (
    <div className="grid w-full grid-cols-4 gap-0.5" role="group" aria-label="Time signature">
      {meterChoices(value).map((m) => (
        <button key={m} type="button" aria-pressed={m === value} onClick={() => onChange(m)}
          className={cn("rounded border font-semibold tabular-nums", compact ? "h-5 text-[10px]" : "h-7 text-[11px]", m === value ? "border-accent bg-accent/20 text-accent" : "border-line text-muted hover:text-fg")}>
          {m}
        </button>
      ))}
    </div>
  );
}

function Head({ h, children }: { h: number; children: ReactNode }) {
  return <div className="flex items-center gap-1.5 border-b border-line/80 px-2" style={{ height: h }}>{children}</div>;
}

function Ms({ muted, solo, disabled, onMute, onSolo }: { muted: boolean; solo?: boolean; disabled?: boolean; onMute: () => void; onSolo?: () => void }) {
  return (
    <span className="flex shrink-0 gap-0.5">
      <button type="button" disabled={disabled} aria-pressed={muted} aria-label={muted ? "Unmute" : "Mute"} onClick={(e) => { e.stopPropagation(); onMute(); }}
        className={cn("grid h-5 w-5 place-items-center rounded border text-[10px] font-bold", muted ? "border-accent bg-accent text-accent-ink" : "border-line-2 text-muted hover:text-fg", disabled && "opacity-30")}>M</button>
      {onSolo && (
        <button type="button" disabled={disabled} aria-pressed={!!solo} aria-label={solo ? "Unsolo" : "Solo"} onClick={(e) => { e.stopPropagation(); onSolo(); }}
          className={cn("grid h-5 w-5 place-items-center rounded border text-[10px] font-bold", solo ? "border-[#4cc2ff] bg-[#4cc2ff] text-[#06121f]" : "border-line-2 text-muted hover:text-fg", disabled && "opacity-30")}>S</button>
      )}
    </span>
  );
}

function Pill({ label, value, hint, ok, className }: { label: string; value: string; hint?: string; ok?: boolean; className?: string }) {
  return (
    <div className={cn("flex items-center gap-2 rounded-lg border border-line bg-panel px-2.5 py-1", className)}>
      <span className="text-[10px] font-semibold tracking-wide text-muted uppercase">{label}</span>
      <b className="tnum text-sm">{value}</b>
      {hint && <span className={cn("text-[10px] font-semibold", ok ? "text-ok" : "text-warn")}>{hint}</span>}
    </div>
  );
}

function MapRuler({ grid, duration, pxPerBeat }: { grid: Grid; duration: number; pxPerBeat: number }) {
  const pxPerBar = pxPerBeat * grid.beatsPerBar;
  let every = 1;
  while (pxPerBar * every < 42 && every < 128) every *= 2;
  const drawAll = pxPerBar >= 10;
  const ticks = beatsInRange(grid, 0, duration).filter((b) => b.beat === 1);
  return (
    <>
      {ticks.map((b) => {
        const index = b.bar - 1;
        const show = index % every === 0;
        if (!drawAll && !show) return null;
        const major = index % 4 === 0;
        return (
          <div key={b.q} className={cn("absolute top-0 bottom-0 border-l", major ? "border-line-2 text-fg-2" : "border-white/10 text-faint")} style={{ left: `${(b.t / duration) * 100}%` }}>
            {show && <span className="pl-1 text-[11px] tabular-nums">{b.bar}</span>}
          </div>
        );
      })}
    </>
  );
}

function ClickTicks({ grid, duration, every }: { grid: Grid; duration: number; every: number }) {
  const ticks = useMemo(() => beatsInRange(grid, 0, duration).filter((b) => every === 1 || b.accent), [grid, duration, every]);
  return (
    <>
      {ticks.map((tk) => {
        const down = tk.beat === 1;
        const mid = grid.beatsPerBar === 6 && tk.beat === 4;
        return (
          <div key={tk.q} className={cn("absolute bottom-0 w-px", down || mid ? "bg-accent" : "bg-white/25", mid && "opacity-60")} style={{ left: `${(tk.t / duration) * 100}%`, height: down ? "70%" : mid ? "55%" : "40%" }} />
        );
      })}
    </>
  );
}

function PeakLane({ wave, duration, view, height, color, role, dim }: {
  wave: WaveformData | null; duration: number; view: { start: number; span: number; width: number }; height: number; color: string; role: TrackRole; dim: boolean;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const width = view.width;
    const span = view.span > 0 ? view.span : duration;
    const start = view.span > 0 ? view.start : 0;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.floor(width * dpr);
    c.height = Math.floor(height * dpr);
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, width, height);
    const mid = height / 2;
    g.globalAlpha = dim ? 0.28 : 0.9;
    if (!wave || !wave.peaks.length || span <= 0) {
      g.fillStyle = "rgba(255,255,255,0.08)";
      g.fillRect(0, mid - 0.5, width, 1);
      return;
    }
    const n = wave.peaks.length;
    const pps = wave.peaksPerSecond > 0 ? wave.peaksPerSecond : n / Math.max(duration, 0.001);
    const bars = Math.max(1, Math.floor(width / 2));
    g.fillStyle = color;
    for (let i = 0; i < bars; i++) {
      const t0 = start + (i / bars) * span;
      const t1 = start + ((i + 1) / bars) * span;
      const a = Math.max(0, Math.floor(t0 * pps));
      const b = Math.min(n, Math.max(a + 1, Math.ceil(t1 * pps)));
      let pk = 0;
      let rms = 0;
      for (let j = a; j < b; j++) {
        if (wave.peaks[j] > pk) pk = wave.peaks[j];
        if (wave.rms[j] > rms) rms = wave.rms[j];
      }
      const shaped = role === "drums" ? Math.pow(pk / 255, 0.5) : role === "bass" ? (rms / 255) * 0.9 : role === "vocals" ? Math.min(1, (pk / 255) * 0.72) : pk / 255;
      const ph = Math.max(1, shaped * (height * 0.42));
      g.fillRect(i * 2, mid - ph, 1.5, ph * 2);
    }
  }, [wave, view, height, duration, color, role, dim]);
  return (
    <canvas ref={canvas} className="pointer-events-none absolute top-0 block" style={{ left: duration > 0 ? `${(view.start / duration) * 100}%` : 0, width: view.width, height }} />
  );
}

function Inspector({ song, selected, grid, offset, beatSec, barSec, tempoPoint, playhead, loaded, onTempoPoint, onToggleTempoCurve, onRename, onRenameFocus, onKind, onDelete, onBpm, onKey, onTimeSignature, onMoveBeat }: {
  song: Song;
  selected: ResolvedSection | null;
  grid: Grid | null;
  offset: number;
  beatSec: number | null;
  barSec: number | null;
  tempoPoint: string | null;
  playhead: number;
  loaded: boolean;
  onTempoPoint: (id: string | null) => void;
  onToggleTempoCurve: () => void;
  onRename: (label: string) => void;
  onRenameFocus: () => void;
  onKind: (kind: string) => void;
  onDelete: () => void;
  onBpm: (bpm: number) => void;
  onKey: (key: string | null) => void;
  onTimeSignature: (timeSignature: string) => void;
  onMoveBeat: (t: number) => void;
}) {
  const conf = song.analysis?.bpmConfidence;
  const beatConf = song.analysis?.downbeatConfidence;
  const keyConf = song.analysis?.keyConfidence;
  const alt = song.analysis?.detected?.bpmAlternatives ?? [];
  const musicalStart = song.analysis?.musicalStart;
  const hi = song.duration > 0 ? song.duration : offset;
  const nudgeBeat = (d: number) => onMoveBeat(Math.max(0, Math.min(hi, Math.round((offset + d) * 1000) / 1000)));
  const first = selected ? song.sections.slice().sort((a, b) => a.start - b.start)[0]?.id === selected.id : false;
  const startBar = selected && grid ? barBeatOf(Math.round(timeToBeat(grid, selected.start)), grid.beatsPerBar).bar : selected && barSec ? Math.round((selected.start - offset) / barSec) + 1 : null;
  const lenBars = selected && grid ? Math.max(1, Math.round((timeToBeat(grid, selected.end) - timeToBeat(grid, selected.start)) / grid.beatsPerBar)) : selected && barSec ? Math.max(1, Math.round((selected.end - selected.start) / barSec)) : null;
  const picked = grid?.points.find((p) => p.id === tempoPoint) ?? null;
  return (
    <aside className="flex w-[17.5rem] shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-[#111215] p-3">
      <div className="rounded-xl border border-line bg-panel p-3">
        <h4 className="mb-2 text-[10px] font-semibold tracking-wider text-muted uppercase">Section</h4>
        {selected ? (
          <div className="space-y-2.5">
            <input className="field h-8 text-sm" aria-label="Section name" value={selected.label}
              onFocus={onRenameFocus} onChange={(e) => onRename(e.target.value)} />
            <div className="flex flex-wrap gap-1">
              {KIND_CHIPS.map((k) => (
                <button key={k.kind} type="button" onClick={() => onKind(k.kind)}
                  className={cn("rounded-md border px-1.5 py-0.5 text-[11px] font-semibold", selected.kind === k.kind ? "border-white/40 bg-white/10 text-fg" : "border-line text-muted hover:text-fg")}>
                  {k.short}
                </button>
              ))}
            </div>
            <Row label="Starts" value={startBar != null ? `Bar ${startBar}` : fmtTime(selected.start, true)} />
            <Row label="Length" value={lenBars != null ? `${lenBars} ${lenBars === 1 ? "bar" : "bars"}` : fmtTime(selected.end - selected.start)} />
            <p className="text-[11px] leading-snug text-faint">Cue speaks “{selected.label}” {song.cueLeadBeats} beats before this section.</p>
            <button type="button" className="text-xs font-semibold text-err hover:underline" onClick={onDelete} disabled={first && song.sections.length === 1}>Delete section</button>
          </div>
        ) : (
          <p className="text-xs text-muted">Click a section clip. Double-click the arrangement lane, or press M, to split at the playhead.</p>
        )}
      </div>

      <div className="rounded-xl border border-line bg-panel p-3">
        <h4 className="mb-2 text-[10px] font-semibold tracking-wider text-muted uppercase">Beat grid</h4>
        <p className="mb-2 text-[11px] leading-snug text-faint">Tempo, Beat 1, and meter. Edits update the tempo map and click.</p>
        <div className="mb-2">
          <div className="mb-1 text-[10px] font-semibold tracking-wide text-muted uppercase">Key</div>
          <select className="field h-8 text-sm" value={song.key ?? ""} onChange={(e) => onKey(e.target.value || null)} aria-label="Key">
            <option value="">—</option>
            <optgroup label="Major">{KEYS.map((k) => <option key={k} value={k}>{k}</option>)}</optgroup>
            <optgroup label="Minor">{MINOR_KEYS.map((k) => <option key={k} value={k}>{k}</option>)}</optgroup>
          </select>
          {keyConf != null && keyConf < 0.3 && <p className="mt-1 text-[10px] text-warn">Low confidence — verify</p>}
        </div>
        <div className="mb-1 text-[10px] font-semibold tracking-wide text-muted uppercase">Meter</div>
        <MeterButtons value={song.timeSignature} onChange={onTimeSignature} />
        <p className="mt-1 mb-2 text-[11px] leading-snug text-faint">{meterHint(song.timeSignature)} The tempo number stays the speed of each click.</p>
        <div className="mb-1 text-[10px] font-semibold tracking-wide text-muted uppercase">Tempo (BPM)</div>
        <div className="mb-2 flex items-center gap-1">
          <BpmField bpm={song.bpm} onBpm={onBpm} />
          <TapTempo onBpm={onBpm} />
        </div>
        {conf != null && conf < 0.6 && <p className="mb-1 text-[10px] text-warn">Low BPM confidence — verify</p>}
        {conf != null && <Conf label="BPM confidence" value={conf} />}
        <div className="mt-2 grid grid-cols-2 gap-1">
          <Mini onClick={() => song.bpm && onBpm(Math.max(30, song.bpm / 2))} disabled={!song.bpm}>÷2</Mini>
          <Mini onClick={() => song.bpm && onBpm(Math.min(300, song.bpm * 2))} disabled={!song.bpm}>×2</Mini>
        </div>
        {alt.length > 0 && song.bpm && !alt.includes(song.bpm) && (
          <button type="button" className="mt-1.5 text-left text-[10px] text-mon hover:underline" onClick={() => onBpm(alt[0])}>Use detector {alt[0]} BPM</button>
        )}
        <div className="mt-3 mb-1 text-[10px] font-semibold tracking-wide text-muted uppercase">Beat 1</div>
        <BeatField offset={offset} max={hi} onBeat={onMoveBeat} />
        <div className="mt-1 flex gap-1">
          <Tip label={loaded ? `Snap to playhead (${fmtTime(playhead, true)})` : "Load and pause on Beat 1 first"}>
            <Button size="sm" variant="outline" className="h-7 flex-1 px-1" icon={<Crosshair className="size-3.5" />} disabled={!loaded} onClick={() => onMoveBeat(Math.round(playhead * 1000) / 1000)}>Playhead</Button>
          </Tip>
        </div>
        <div className="mt-2 grid grid-cols-4 gap-1">
          <Mini onClick={() => nudgeBeat(-0.01)}>−10 ms</Mini>
          <Mini onClick={() => nudgeBeat(0.01)}>+10 ms</Mini>
          <Mini onClick={() => beatSec && nudgeBeat(-beatSec)} disabled={!beatSec}>◀ beat</Mini>
          <Mini onClick={() => beatSec && nudgeBeat(beatSec)} disabled={!beatSec}>beat ▶</Mini>
        </div>
        {musicalStart != null && musicalStart > 1.5 && (
          <button type="button" className="mt-2 text-left text-[10px] text-mon hover:underline" onClick={() => onMoveBeat(musicalStart)}>
            Musical start at {fmtTime(musicalStart, true)}
          </button>
        )}
        {beatConf != null && <Conf label="Beat 1 confidence" value={beatConf} />}
        {grid && (
          <div className="mt-3 border-t border-line pt-2">
            <div className="mb-1 flex items-center justify-between">
              <span className="text-[10px] font-semibold tracking-wide text-muted uppercase">Tempo map · {grid.points.length} {grid.points.length === 1 ? "point" : "points"}</span>
              <button type="button" className="text-[10px] font-bold text-[#e2b340]" onClick={onToggleTempoCurve}>{picked ? (picked.curve === "linear" ? "Ramp" : "Step") : tempoCurveLabel(grid, null, playhead)}</button>
            </div>
            <div className="max-h-36 overflow-y-auto">
              {grid.points.map((p, i) => (
                <button key={p.id} type="button" onClick={() => onTempoPoint(p.id)}
                  className={cn("flex w-full items-center gap-2 rounded px-1 py-0.5 text-left text-[11px]", p.id === tempoPoint ? "bg-white/10 text-fg" : "text-muted hover:text-fg")}>
                  <span className="w-4 tnum">{i + 1}</span>
                  <span className="tnum w-12">{p.bar}.{p.beat}</span>
                  <span className="tnum flex-1">{p.bpm.toFixed(2)}</span>
                  <span>{p.curve === "linear" ? "Ramp" : "Step"}</span>
                </button>
              ))}
            </div>
            {picked && (
              <p className="mt-1 text-[11px] text-faint">
                {fmtPointSafe(grid, picked)} · drag up or down to change BPM. Point 1 stays on Beat 1.
              </p>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}

const menuContent =
  "z-50 min-w-[12rem] rounded-lg border border-line bg-panel p-1 text-sm shadow-lg";

function SnapMenu({ snap, grid, onSnap, onSnapAll }: { snap: SnapMode; grid: boolean; onSnap: () => void; onSnapAll: () => void }) {
  const label = snap === "bar" ? "Bar" : snap === "beat" ? "Beat" : "Off";
  return (
    <div className="flex items-stretch">
      <Button size="sm" variant="outline" className="rounded-r-none border-r-0 pr-2" onClick={onSnap}>
        Snap: {label}
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button size="sm" variant="outline" className="rounded-l-none px-1.5" aria-label="Snap options">
            <ChevronDown className="size-3.5" />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className={menuContent} align="end" sideOffset={4}>
            <DropdownMenu.Item
              className="cursor-pointer rounded-md px-2 py-1.5 text-xs outline-none hover:bg-white/5 data-[disabled]:opacity-40"
              disabled={!grid}
              onSelect={onSnapAll}
            >
              Snap section markers to bars
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  );
}

function EditorShortcutsMenu() {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <IconButton label="Editor shortcuts" size="sm"><HelpCircle className="size-4" /></IconButton>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={cn(menuContent, "max-w-[16rem] p-3 text-[11px] leading-relaxed text-muted")} align="end" sideOffset={4}>
          <p className="mb-1.5 text-[10px] font-semibold tracking-wider uppercase text-fg-2">Shortcuts</p>
          <p><Kbd>M</Kbd> split at playhead</p>
          <p><Kbd>←</Kbd><Kbd>→</Kbd> move 1 bar · <Kbd>⌥</Kbd> 1 beat · <Kbd>⇧</Kbd> 50 ms</p>
          <p><Kbd>⌫</Kbd> delete section or tempo point · <Kbd>T</Kbd> Beat 1 at playhead</p>
          <p><Kbd>L</Kbd> loop section · <Kbd>Z</Kbd> fit section · <Kbd>⌘Z</Kbd> undo · <Kbd>⇧⌘Z</Kbd> redo</p>
          <p><Kbd>B</Kbd> draw tempo · <Kbd>R</Kbd> step/ramp · double-click tempo lane to add a point</p>
          <p className="mt-1 text-faint">Drag a clip edge to move the boundary. The first section stays at 0:00.</p>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2 py-0.5 text-xs">
      <span className="text-muted">{label}</span>
      <span className="tnum font-semibold">{value}</span>
    </div>
  );
}

function Conf({ label, value }: { label: string; value: number }) {
  return (
    <div className="mb-1.5">
      <div className="mb-0.5 flex justify-between text-[10px] text-faint"><span>{label}</span><span>{Math.round(value * 100)}%</span></div>
      <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
        <div className={cn("h-full rounded-full", value >= 0.65 ? "bg-ok" : value >= 0.35 ? "bg-warn" : "bg-err")} style={{ width: `${Math.round(value * 100)}%` }} />
      </div>
    </div>
  );
}

function Mini({ children, onClick, disabled }: { children: ReactNode; onClick: () => void; disabled?: boolean }) {
  return (
    <button type="button" disabled={disabled} onClick={onClick}
      className="h-7 rounded-md border border-line text-[10px] font-semibold text-muted hover:border-line-2 hover:text-fg disabled:opacity-40">
      {children}
    </button>
  );
}

function BeatField({ offset, max, onBeat }: { offset: number; max: number; onBeat: (t: number) => void }) {
  const [text, setText] = useState(String(Math.round(offset * 1000) / 1000));
  useEffect(() => setText(String(Math.round(offset * 1000) / 1000)), [offset]);
  const commit = () => {
    const n = Number(text.replace(",", "."));
    if (!Number.isFinite(n)) return setText(String(offset));
    onBeat(Math.max(0, Math.min(max, Math.round(n * 1000) / 1000)));
  };
  return (
    <input className="field tnum h-8 w-full text-sm" inputMode="decimal" aria-label="Beat 1 offset" value={text}
      onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && (e.currentTarget.blur())} />
  );
}

function BpmField({ bpm, onBpm }: { bpm: number | null; onBpm: (bpm: number) => void }) {
  const [text, setText] = useState(bpm == null ? "" : String(Math.round(bpm * 1000) / 1000));
  useEffect(() => setText(bpm == null ? "" : String(Math.round(bpm * 1000) / 1000)), [bpm]);
  const commit = () => {
    const n = Number(text.replace(",", "."));
    if (!Number.isFinite(n)) return setText(bpm == null ? "" : String(bpm));
    onBpm(Math.round(Math.min(300, Math.max(30, n)) * 1000) / 1000);
  };
  return (
    <input className="field tnum h-7 w-24 text-right text-sm" inputMode="decimal" aria-label="Tempo" value={text}
      onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && (e.currentTarget.blur())} />
  );
}

function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="mx-0.5 rounded border border-line-2 px-1 font-mono text-[10px] text-fg-2">{children}</kbd>;
}

function mapSpan(grid: Grid, start: number, end: number) {
  const a = barBeatOf(Math.round(timeToBeat(grid, start)), grid.beatsPerBar).bar;
  const b = barBeatOf(Math.round(timeToBeat(grid, end)), grid.beatsPerBar).bar;
  if (b <= a) return `bar ${a}`;
  return `bars ${a}–${b}`;
}

function compactFromGrid(grid: Grid, t: number) {
  const pos = barBeatOf(Math.round(timeToBeat(grid, t)), grid.beatsPerBar);
  return `${pos.bar}.${pos.beat}`;
}

function fmtPointSafe(grid: Grid, p: Grid["points"][number]) {
  const i = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, grid.beatsPerBar) - qOf(b.bar, b.beat, grid.beatsPerBar)).findIndex((x) => x.id === p.id);
  const t = beatToTime(grid, i <= 0 ? 0 : qOf(p.bar, p.beat, grid.beatsPerBar));
  return `Bar ${p.bar}.${p.beat} · ${fmtTime(t, true)} · ${p.bpm.toFixed(2)} BPM`;
}

function formatZoom(z: number) {
  if (z < 10) {
    const n = Math.round(z * 10) / 10;
    return `${Number.isInteger(n) ? n.toFixed(0) : n.toFixed(1)}×`;
  }
  return `${Math.round(z)}×`;
}
