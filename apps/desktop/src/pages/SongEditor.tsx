import { X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { AiAssistant } from "@/components/AiAssistant";
import { ArrangementEditor } from "@/components/Arrangement";
import { PresetPicker, StemControls, samePrep } from "@/components/StemSettings";
import { SongTimeline, type SnapMode } from "@/components/SongTimeline";
import { Badge, Button, Card, Slider, Switch } from "@/components/ui";
import { acceptSongReview, loadSong, prepareSongs } from "@/lib/actions";
import { clampTimelineZoom } from "@/lib/editorZoom";
import { cn, fmtDb, fmtTime } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { modeFromKey, pitchClassFromKey } from "@/lib/keyDisplay";
import { beatsPerBar } from "@/lib/meter";
import { visibleReviewWarnings } from "@/lib/reviewWarnings";
import { nextLabel, resolveSections } from "@/lib/sections";
import { barBeatOf, beatToTime, constantGrid, ensureGrid, movePoint, qOf, remapBeatsPerBar, removePoint, resnapSections, setCurve, timeToBeat, type Grid } from "@/lib/tempoMap";
import { isRolling } from "@/lib/transport";
import type { AiProposal, Section, Song, TrackInfo, TrackRole, Waveform as WF } from "@/lib/types";
import { useApp, usePlayback } from "@/store/app";

interface Hist {
  sections: Section[];
  beatOffset: number;
  bpm: number | null;
  arrangement: string[];
  countInBars: number;
  timeSignature: string;
  grid: Grid | null;
}

/** Convert seconds + beat grid → "Bar 3 · Beat 1" string. */
export function fmtBarBeat(t: number, bpm: number | null, offset: number, beatsPerBar: number): string {
  if (!bpm || bpm <= 0) return fmtTime(t, true);
  const beat = 60 / bpm;
  const beatNum = Math.round((t - offset) / beat);
  const bar = Math.floor(beatNum / beatsPerBar) + 1;
  const b = ((beatNum % beatsPerBar) + beatsPerBar) % beatsPerBar + 1;
  return `Bar ${bar} · Beat ${b}`;
}

export function SongEditor({ id }: { id: string }) {
  const stored = useApp((s) => s.songs.find((x) => x.id === id));
  const presets = useApp((s) => s.presets);
  const back = useApp((s) => s.back);
  const loaded = useApp((s) => s.loaded);
  const [draft, setDraft] = useState<Song | null>(stored ?? null);
  const [saving, setSaving] = useState(false);
  const [wave, setWave] = useState<WF | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [snapMode, setSnapMode] = useState<SnapMode>("bar");
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const [tempoPoint, setTempoPoint] = useState<string | null>(null);
  const [drawTempo, setDrawTempo] = useState(false);
  const [mixOpen, setMixOpen] = useState(false);
  const [solo, setSolo] = useState<TrackRole | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const zoomAnchor = useRef<{ t: number; x: number } | null>(null);
  const zoomRef = useRef(1);
  zoomRef.current = zoom;
  const past = useRef<Hist[]>([]);
  const future = useRef<Hist[]>([]);
  const gridSend = useRef<{ id: string; latest: Grid | null; busy: boolean }>({ id: "", latest: null, busy: false });
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const tempoPointRef = useRef(tempoPoint);
  tempoPointRef.current = tempoPoint;
  const soloBackup = useRef<TrackInfo[] | null>(null);
  const renameArmed = useRef<string | null>(null);
  const actionsRef = useRef({
    save: () => {},
    undo: () => {},
    redo: () => {},
    tempo: (_e: KeyboardEvent): boolean => false,
    add: (_t: number) => {},
    nudge: (_dir: number, _fine: "bar" | "beat" | "ms") => {},
    remove: () => {},
    loop: () => {},
    tapBeat: () => {},
    zoomSel: () => {},
    closeMix: () => {},
    mixOpen: false,
  });
  const isLoaded = loaded?.songId === id;
  const status = usePlayback((s) => (s.status?.songId === id ? s.status : null));
  const rolling = isRolling(status?.state);

  // Adopt backend updates (preparation finished, etc.) when there are no local edits.
  const dirty = useMemo(() => !!draft && !!stored && JSON.stringify(editable(draft)) !== JSON.stringify(editable(stored)), [draft, stored]);
  const lastStored = useRef(stored);
  useEffect(() => {
    if (!stored) return;
    if (!draft || (!dirty && lastStored.current?.updatedAt !== stored.updatedAt)) setDraft(stored);
    else if (draft && lastStored.current && (stored.status !== lastStored.current.status || stored.preparedAt !== lastStored.current.preparedAt)) {
      // Keep edits but take over backend-owned fields.
      setDraft({ ...draft, status: stored.status, stems: stored.stems, rendered: stored.rendered, analysis: stored.analysis, preparedAt: stored.preparedAt, lastError: stored.lastError, duration: stored.duration });
    }
    lastStored.current = stored;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stored]);

  useEffect(() => {
    ipc.waveform(id).then(setWave).catch(() => setWave(null));
  }, [id, stored?.preparedAt]);

  const set = useCallback((patch: Partial<Song>) => setDraft((d) => (d ? { ...d, ...patch } : d)), []);

  const shot = (d: Song): Hist => ({
    sections: d.sections, beatOffset: d.beatOffset, bpm: d.bpm, arrangement: d.arrangement,
    countInBars: d.countInBars, timeSignature: d.timeSignature, grid: d.grid ?? null,
  });

  const pushGrid = useCallback((id: string, grid: Grid) => {
    const slot = gridSend.current;
    slot.id = id;
    slot.latest = grid;
    if (slot.busy) return;
    slot.busy = true;
    const pump = async () => {
      while (slot.latest) {
        const songId = slot.id;
        const g = slot.latest;
        slot.latest = null;
        try {
          const saved = await ipc.setGrid(songId, g);
          if (!slot.latest && saved) useApp.getState().upsertSong(saved);
        } catch { /* the next edit sends again */ }
      }
      slot.busy = false;
    };
    void pump();
  }, []);

  const writeGrid = useCallback((grid: Grid, toastIt: boolean, persist = true) => {
    const d = draftRef.current;
    if (!d) return;
    const prev = ensureGrid(d);
    const snapped = resnapSections(d.sections, prev, grid, d.duration);
    const first = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, grid.beatsPerBar) - qOf(b.bar, b.beat, grid.beatsPerBar))[0];
    set({ grid, bpm: first?.bpm ?? d.bpm, beatOffset: grid.beat1Sec, sections: snapped.sections });
    if (toastIt && snapped.moved > 0) toast.message(`${snapped.moved} ${snapped.moved === 1 ? "section" : "sections"} adjusted · Undo`);
    if (persist) pushGrid(d.id, grid);
  }, [pushGrid, set]);

  const remember = useCallback(() => {
    const d = draftRef.current;
    if (!d) return;
    past.current.push(shot(d));
    if (past.current.length > 50) past.current.shift();
    future.current = [];
    setCanUndo(true);
    setCanRedo(false);
  }, []);

  const restore = useCallback((entry: Hist) => {
    const d = draftRef.current;
    if (!d) return;
    set(entry);
    const grid = entry.grid ?? (entry.bpm != null ? constantGrid(entry.bpm, entry.beatOffset, beatsPerBar(entry.timeSignature), "user") : null);
    if (grid) pushGrid(d.id, grid);
    else if (entry.beatOffset !== d.beatOffset) ipc.setDownbeat(d.id, entry.beatOffset).catch(() => {});
  }, [pushGrid, set]);

  const undo = useCallback(() => {
    const prev = past.current.pop();
    setCanUndo(past.current.length > 0);
    const d = draftRef.current;
    if (!prev || !d) return;
    future.current.push(shot(d));
    setCanRedo(true);
    restore(prev);
  }, [restore]);

  const redo = useCallback(() => {
    const next = future.current.pop();
    setCanRedo(future.current.length > 0);
    const d = draftRef.current;
    if (!next || !d) return;
    past.current.push(shot(d));
    setCanUndo(true);
    restore(next);
  }, [restore]);

  const edit = useCallback((patch: Partial<Song>) => {
    remember();
    set(patch);
  }, [remember, set]);

  const save = useCallback(async (d = draft): Promise<Song | null> => {
    if (!d) return null;
    setSaving(true);
    try {
      const s = await ipc.saveSong(d);
      useApp.getState().upsertSong(s);
      lastStored.current = s;
      setDraft(s);
      // Re-load into the player so section / arrangement edits are live (only when not rolling).
      const l = useApp.getState().loaded;
      if (l?.songId === s.id && !isRolling(usePlayback.getState().status?.state)) await loadSong(s.id, l.context);
      return s;
    } catch (e) {
      toast.error("Could not save", { description: errMsg(e) });
      return null;
    } finally {
      setSaving(false);
    }
  }, [draft]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      const inInput = el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
      const a = actionsRef.current;
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        a.save();
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "z" && e.shiftKey && !inInput) {
        e.preventDefault();
        a.redo();
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "z" && !e.shiftKey && !inInput) {
        e.preventDefault();
        a.undo();
        return;
      }
      if (e.key === "Escape" && a.mixOpen) {
        a.closeMix();
        return;
      }
      if (inInput || e.metaKey || e.ctrlKey || e.repeat) return;
      if (a.tempo(e)) return;
      if (e.key === "m") {
        e.preventDefault();
        const pos = usePlayback.getState().status?.position ?? 0;
        const loadedHere = useApp.getState().loaded?.songId === draftRef.current?.id;
        if (!loadedHere && pos < 0.05) {
          toast.message("Play the song, then press M at the playhead");
          return;
        }
        a.add(pos);
      } else if (e.key === "Backspace" || e.key === "Delete") {
        e.preventDefault();
        a.remove();
      } else if (e.key === "l" || e.key === "L") {
        e.preventDefault();
        a.loop();
      } else if (e.key === "t" || e.key === "T") {
        e.preventDefault();
        a.tapBeat();
      } else if (e.key === "z" || e.key === "Z") {
        e.preventDefault();
        a.zoomSel();
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        const fine = e.shiftKey ? "ms" : e.altKey ? "beat" : "bar";
        a.nudge(e.key === "ArrowLeft" ? -1 : 1, fine);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const sections = useMemo(() => (draft ? resolveSections(draft) : []), [draft]);

  const selectSection = useCallback((id: string) => setSelected(id), []);

  const playhead = status?.position ?? 0;

  useLayoutEffect(() => {
    const el = scroller.current;
    const duration = draft?.duration ?? 0;
    if (!el || duration <= 0) return;
    if (zoomAnchor.current) {
      const { t, x } = zoomAnchor.current;
      zoomAnchor.current = null;
      el.scrollLeft = Math.max(0, (t / duration) * el.scrollWidth - x);
      return;
    }
    if (rolling && zoom > 1) {
      el.scrollLeft = Math.max(0, (playhead / duration) * el.scrollWidth - el.clientWidth * 0.32);
    }
  }, [zoom, playhead, rolling, draft?.duration]);

  useEffect(() => {
    const el = scroller.current;
    const duration = draft?.duration ?? 0;
    if (!el) return;
    const zoomAt = (clientX: number, nextZoom: number) => {
      const rect = el.getBoundingClientRect();
      const x = Math.max(0, clientX - rect.left);
      const t = duration > 0 && el.scrollWidth > 0 ? ((el.scrollLeft + x) / el.scrollWidth) * duration : 0;
      const next = clampTimelineZoom(nextZoom);
      if (Math.abs(next - zoomRef.current) < 0.01) return;
      zoomAnchor.current = { t, x };
      zoomRef.current = next;
      setZoom(next);
    };
    // Pinch (WebKit gesture events, and ctrl+wheel elsewhere). A vertical
    // two-finger slide zooms too. A sideways slide is left alone so it pans.
    let pinching = false;
    let gestureBase = 1;
    const onWheel = (e: WheelEvent) => {
      if (pinching && e.ctrlKey) {
        e.preventDefault();
        return;
      }
      const pinch = e.ctrlKey || e.metaKey;
      const vertical = Math.abs(e.deltaY) > Math.abs(e.deltaX);
      if (!pinch && !vertical) return;
      e.preventDefault();
      let dy = e.deltaY;
      if (e.deltaMode === WheelEvent.DOM_DELTA_LINE) dy *= 16;
      else if (e.deltaMode === WheelEvent.DOM_DELTA_PAGE) dy *= el.clientHeight;
      const gain = pinch ? 0.01 : 0.0018;
      zoomAt(e.clientX, zoomRef.current * Math.exp(-dy * gain));
    };
    const startGesture = (e: Event) => {
      e.preventDefault();
      pinching = true;
      gestureBase = zoomRef.current;
    };
    const changeGesture = (e: Event) => {
      e.preventDefault();
      const g = e as Event & { scale: number; clientX: number };
      if (!(g.scale > 0)) return;
      zoomAt(g.clientX, gestureBase * g.scale);
    };
    const endGesture = () => {
      pinching = false;
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", startGesture, { passive: false });
    el.addEventListener("gesturechange", changeGesture, { passive: false });
    el.addEventListener("gestureend", endGesture);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", startGesture);
      el.removeEventListener("gesturechange", changeGesture);
      el.removeEventListener("gestureend", endGesture);
    };
  }, [draft?.duration]);

  if (!draft) return <div className="p-10 text-muted">Song not found.</div>;

  const ensureLoaded = async () => (isLoaded ? loaded : await loadSong(id, "rehearsal"));
  const playPause = async () => {
    if (dirty) await save();
    if (!(await ensureLoaded())) return;
    if (rolling) ipc.pause();
    else ipc.play(false).catch((e) => toast.error(errMsg(e)));
  };
  const seek = async (t: number) => {
    if (!(await ensureLoaded())) return;
    ipc.seek(t);
  };

  const focusTime = (t: number, nextZoom = zoom) => {
    const el = scroller.current;
    if (!el || draft.duration <= 0) return;
    const x = el.clientWidth * 0.32;
    const z = clampTimelineZoom(nextZoom);
    if (z !== zoom) {
      zoomAnchor.current = { t, x };
      setZoom(z);
      return;
    }
    el.scrollLeft = Math.max(0, (t / draft.duration) * el.scrollWidth - x);
  };

  const needsRender = !!draft.stems.other && !!draft.analysis?.renderedWith && !samePrep(draft.analysis.renderedWith, draft.prep);
  const hasStems = !!draft.stems.other && !!draft.stems.bass && !!draft.stems.drums;
  const applyProposal = async (p: AiProposal) => {
    const t = p.timeline;
    const patched: Song = {
      ...draft,
      prep: p.prep ?? draft.prep,
      routing: p.routing ?? draft.routing,
      arrangement: p.arrangement ?? draft.arrangement,
      ...(t?.countInBars != null ? { countInBars: t.countInBars } : {}),
      ...(t?.cueLeadBeats != null ? { cueLeadBeats: t.cueLeadBeats } : {}),
      ...(t?.key ? { key: t.key } : {}),
    };
    const next = withTempo(patched, { bpm: t?.bpm, beatOffset: t?.beatOffset, timeSignature: t?.timeSignature });
    setDraft(next);
    const saved = await save(next);
    if (!saved) return;
    let current = saved;
    if (t?.beatOffset != null && !next.grid) {
      try {
        current = await ipc.setDownbeat(saved.id, t.beatOffset);
        useApp.getState().upsertSong(current);
        lastStored.current = current;
        setDraft(current);
      } catch { /* save already stored the offset */ }
    }
    if (p.requiresRerender) {
      if (hasStems) await rerender(current);
      else prepareSongs([current.id]);
    }
    toast.success("Applied");
  };

  const rerender = async (s: Song = draft) => {
    const id = toast.loading("Rendering backing track…");
    try {
      if (dirty) await save();
      const r = await ipc.rerenderBacking(s.id);
      useApp.getState().upsertSong(r);
      lastStored.current = r;
      setDraft(r);
      toast.success("Backing track updated", { id });
    } catch (e) {
      toast.error("Render failed", { id, description: errMsg(e) });
    }
  };

  const addSectionAt = (t: number) => {
    let start = Math.max(0, Math.min(draft.duration || t, t));
    const g = ensureGrid(draft);
    if (g && snapMode !== "off") {
      let q = timeToBeat(g, start);
      q = snapMode === "bar" ? Math.round(q / g.beatsPerBar) * g.beatsPerBar : Math.round(q);
      start = beatToTime(g, q);
    }
    start = Math.max(0, Math.min(draft.duration || start, Math.round(start * 1000) / 1000));
    if (draft.sections.some((s) => Math.abs(s.start - start) < 0.02)) start = Math.min(draft.duration, start + 0.05);
    const kind = guessNextKind(draft.sections);
    const s: Section = { id: crypto.randomUUID(), label: nextLabel(draft.sections, kind), kind, start };
    remember();
    set({ sections: [...draft.sections, s].sort((a, b) => a.start - b.start) });
    setSelected(s.id);
  };

  const snapToBars = () => {
    const g = ensureGrid(draft);
    if (!g) return toast.warning("Set a BPM first");
    remember();
    set({
      sections: draft.sections.map((s, i) => {
        if (i === 0) return s;
        const q = Math.round(timeToBeat(g, s.start) / g.beatsPerBar) * g.beatsPerBar;
        return { ...s, start: Math.max(0, Math.round(beatToTime(g, q) * 1000) / 1000) };
      }),
    });
    toast.success("Section markers snapped to bars");
  };

  const moveSectionStart = (sectionId: string, t: number) => {
    const start = Math.round(t * 1000) / 1000;
    const cur = draft.sections.find((s) => s.id === sectionId);
    if (!cur || Math.abs(cur.start - start) < 0.0005) return;
    remember();
    set({ sections: draft.sections.map((s) => (s.id === sectionId ? { ...s, start } : s)).sort((a, b) => a.start - b.start) });
  };

  const commitBeat = (t: number) => {
    const hi = draft.duration > 0 ? draft.duration : t;
    const v = Math.max(0, Math.min(hi, Math.round(t * 1000) / 1000));
    if (Math.abs(v - draft.beatOffset) < 0.0005) return;
    const g = ensureGrid(draft);
    remember();
    if (g) writeGrid({ ...g, beat1Sec: v, source: "user" }, true);
    else {
      set({ beatOffset: v });
      ipc.setDownbeat(draft.id, v).catch(() => {});
    }
  };

  const commitBpm = (bpm: number) => {
    const v = Math.round(Math.min(300, Math.max(30, bpm)) * 1000) / 1000;
    const g = ensureGrid(draft.bpm == null ? { ...draft, bpm: v } : draft);
    if (!g) return;
    const first = [...g.points].sort((a, b) => qOf(a.bar, a.beat, g.beatsPerBar) - qOf(b.bar, b.beat, g.beatsPerBar))[0];
    if (!first || Math.abs(first.bpm - v) < 0.0005) return;
    remember();
    writeGrid({ ...g, source: "user", points: g.points.map((p) => (p.id === first.id ? { ...p, bpm: v } : p)) }, true);
  };

  const applyMeter = (timeSignature: string) => {
    const d = draftRef.current;
    if (!d || d.timeSignature === timeSignature) return;
    const g = ensureGrid(d);
    remember();
    if (!g) {
      set({ timeSignature });
      return;
    }
    const grid = remapBeatsPerBar(g, beatsPerBar(timeSignature));
    const snapped = resnapSections(d.sections, g, grid, d.duration);
    const first = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, grid.beatsPerBar) - qOf(b.bar, b.beat, grid.beatsPerBar))[0];
    set({ timeSignature, grid, bpm: first?.bpm ?? d.bpm, beatOffset: grid.beat1Sec, sections: snapped.sections });
    if (snapped.moved > 0) toast.message(`${snapped.moved} ${snapped.moved === 1 ? "section" : "sections"} adjusted · Undo`);
    pushGrid(d.id, grid);
  };

  const removeSelected = () => {
    if (!selected) return;
    remember();
    set({ sections: draft.sections.filter((s) => s.id !== selected), arrangement: draft.arrangement.filter((a) => a !== selected) });
    setSelected(null);
  };

  const nudgeSelected = (dir: number, fine: "bar" | "beat" | "ms") => {
    if (!selected) return;
    const sorted = [...draft.sections].sort((a, b) => a.start - b.start);
    const idx = sorted.findIndex((s) => s.id === selected);
    if (idx <= 0) return;
    const g = ensureGrid(draft);
    let step = 0.05;
    if (fine !== "ms" && g) {
      const q = timeToBeat(g, sorted[idx].start);
      const next = q + dir * (fine === "beat" ? 1 : g.beatsPerBar);
      step = Math.abs(beatToTime(g, next) - sorted[idx].start);
    } else if (fine !== "ms" && draft.bpm) {
      const beat = 60 / draft.bpm;
      step = fine === "beat" ? beat : beat * beatsPerBar(draft.timeSignature);
    }
    const min = sorted[idx - 1].start + 0.05;
    const max = (sorted[idx + 1]?.start ?? draft.duration) - 0.05;
    const start = Math.max(min, Math.min(max, Math.round((sorted[idx].start + dir * step) * 1000) / 1000));
    if (Math.abs(start - sorted[idx].start) < 0.0005) return;
    remember();
    set({ sections: draft.sections.map((s) => (s.id === selected ? { ...s, start } : s)).sort((a, b) => a.start - b.start) });
  };

  const muteRole = async (role: TrackRole, muted: boolean) => {
    const info = await ensureLoaded();
    if (!info) return;
    const t = info.tracks.find((x) => x.role === role);
    if (!t) return toast.message("Prepare the song to separate this stem");
    const tracks = info.tracks.map((x) => (x.role === role ? { ...x, muted } : x));
    useApp.getState().setLoaded({ ...info, tracks });
    ipc.setTrack(role, t.gainDb, muted).catch((e) => toast.error(errMsg(e)));
  };

  const soloRole = async (role: TrackRole | null) => {
    const info = await ensureLoaded();
    if (!info) return;
    const musical: TrackRole[] = ["drums", "bass", "other", "vocals"];
    if (role == null) {
      const prev = soloBackup.current;
      soloBackup.current = null;
      setSolo(null);
      if (!prev) return;
      const tracks = info.tracks.map((t) => {
        const old = prev.find((p) => p.role === t.role);
        return old ? { ...t, muted: old.muted } : t;
      });
      useApp.getState().setLoaded({ ...info, tracks });
      await Promise.all(tracks.filter((t) => musical.includes(t.role)).map((t) => ipc.setTrack(t.role, t.gainDb, t.muted)));
      return;
    }
    if (!info.tracks.some((t) => t.role === role)) return toast.message("Prepare the song to separate this stem");
    if (!soloBackup.current) soloBackup.current = info.tracks.map((t) => ({ ...t }));
    const tracks = info.tracks.map((t) => (musical.includes(t.role) ? { ...t, muted: t.role !== role } : t));
    setSolo(role);
    useApp.getState().setLoaded({ ...info, tracks });
    await Promise.all(tracks.filter((t) => musical.includes(t.role)).map((t) => ipc.setTrack(t.role, t.gainDb, t.muted)));
  };

  const setClickOn = async (on: boolean) => {
    const info = await ensureLoaded();
    if (!info) return;
    if (on && !info.clickAvailable) return toast.warning("Click needs a BPM");
    useApp.getState().setLoaded({ ...info, clickEnabled: on });
    ipc.setClick(on).catch((e) => toast.error(errMsg(e)));
  };

  const setCueOn = async (on: boolean) => {
    const info = await ensureLoaded();
    if (!info) return;
    useApp.getState().setLoaded({ ...info, cueEnabled: on });
    ipc.setCue(on).catch((e) => toast.error(errMsg(e)));
  };

  actionsRef.current = {
    save: () => { void save(); },
    undo,
    redo,
    tempo: (e) => {
      const d = draftRef.current;
      const g = d ? ensureGrid(d) : null;
      if (e.key === "b" || e.key === "B") {
        e.preventDefault();
        setDrawTempo((v) => !v);
        return true;
      }
      const id = tempoPointRef.current;
      if (!g || !id) return false;
      const points = [...g.points].sort((a, b) => qOf(a.bar, a.beat, g.beatsPerBar) - qOf(b.bar, b.beat, g.beatsPerBar));
      const i = points.findIndex((p) => p.id === id);
      const p = points[i];
      if (!p) return false;
      if (e.key === "Escape") {
        setTempoPoint(null);
        return true;
      }
      if (e.key === "r" || e.key === "R") {
        e.preventDefault();
        remember();
        writeGrid(setCurve(g, id, p.curve === "linear" ? "step" : "linear"), true);
        return true;
      }
      if (e.key === "Backspace" || e.key === "Delete") {
        e.preventDefault();
        const next = removePoint(g, id);
        if (!next) toast.message("Beat 1 keeps its tempo. Delete a later point.");
        else {
          remember();
          writeGrid(next, true);
          setTempoPoint(null);
        }
        return true;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        const n = points[(i + (e.shiftKey ? points.length - 1 : 1)) % points.length];
        setTempoPoint(n.id);
        return true;
      }
      if (e.key === "ArrowUp" || e.key === "ArrowDown") {
        e.preventDefault();
        const step = e.shiftKey ? 0.01 : 0.1;
        remember();
        writeGrid(movePoint(g, id, { bpm: p.bpm + (e.key === "ArrowUp" ? step : -step) }), true);
        return true;
      }
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        if (i === 0) return true;
        const dir = e.key === "ArrowLeft" ? -1 : 1;
        let q = qOf(p.bar, p.beat, g.beatsPerBar);
        if (e.altKey) q += dir * g.beatsPerBar;
        else if (e.shiftKey) q += dir / 16;
        else q += dir;
        const pos = barBeatOf(q, g.beatsPerBar);
        remember();
        writeGrid(movePoint(g, id, { bar: pos.bar, beat: pos.beat }), true);
        return true;
      }
      return false;
    },
    add: addSectionAt,
    nudge: nudgeSelected,
    remove: removeSelected,
    loop: () => { void (async () => { if (!(await ensureLoaded())) return; ipc.loop(!(usePlayback.getState().status?.looping)); })(); },
    tapBeat: () => { void (async () => { if (!(await ensureLoaded())) return; commitBeat(usePlayback.getState().status?.position ?? 0); })(); },
    zoomSel: () => {
      const sec = sections.find((s) => s.id === selected);
      if (!sec || draft.duration <= 0) return;
      const span = Math.max(0.5, sec.end - sec.start);
      focusTime(sec.start, clampTimelineZoom(Math.max(1, (draft.duration / span) * 0.85)));
    },
    closeMix: () => setMixOpen(false),
    mixOpen,
  };

  const activeSectionId = isLoaded && status ? loaded?.segments[status.segmentIndex]?.sectionId ?? null : null;
  const review = useMemo(() => visibleReviewWarnings(draft), [draft]);
  const loopRegion = status?.looping && isLoaded && loaded
    ? (() => {
        const seg = loaded.segments[status.segmentIndex];
        return seg ? { start: seg.start, end: seg.end } : null;
      })()
    : null;

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      {draft.lastError && <div className="shrink-0 border-b border-err/30 bg-err/10 px-4 py-2 text-xs text-err">{draft.lastError}</div>}
      {draft.status === "warning" && (review.active.length > 0 || review.stale.length > 0) && (
        <div className="flex shrink-0 flex-wrap items-start justify-between gap-3 border-b border-warn/30 bg-warn/10 px-4 py-2.5">
          <div className="min-w-0 text-xs">
            <div className="font-semibold text-fg">Needs review</div>
            {review.active.length > 0 && <div className="mt-0.5 text-warn">{review.active.join(" · ")}</div>}
            {review.stale.length > 0 && (
              <p className="mt-1 text-faint">Superseded after your tempo map edits: {review.stale.join(" · ")}</p>
            )}
            <p className="mt-1 max-w-xl text-fg-2">The song can still play. Listen with the click on. If the tempo already sits with the band, confirm it — Re-analyze only updates this when a new tempo is detected.</p>
          </div>
          <Button size="sm" variant="outline" className="shrink-0" onClick={async () => {
            try {
              const s = await acceptSongReview(draft.id);
              lastStored.current = s;
              setDraft(s);
              toast.success(s.status === "ready" ? "Marked live-ready" : "Saved — something else still needs a look");
            } catch (e) {
              toast.error("Could not update status", { description: errMsg(e) });
            }
          }}>{review.active.length === 0 || review.active.every((w) => /bpm|tempo|uncertain|confidence|verify/i.test(w)) ? "Tempo is correct" : "I've checked this"}</Button>
        </div>
      )}
      <SongTimeline
        song={draft}
        sections={sections}
        wave={wave}
        playhead={playhead}
        rolling={rolling}
        loaded={isLoaded}
        looping={!!status?.looping}
        loopRegion={loopRegion}
        zoom={zoom}
        scrollerRef={scroller}
        selectedId={selected}
        canUndo={canUndo}
        canRedo={canRedo}
        tempoPoint={tempoPoint}
        drawTempo={drawTempo}
        snap={snapMode}
        saving={saving}
        dirty={dirty}
        tracks={isLoaded && loaded ? loaded.tracks : []}
        clickOn={isLoaded ? !!loaded?.clickEnabled : draft.routing.clickEnabled}
        cueOn={isLoaded ? !!loaded?.cueEnabled : draft.routing.cueEnabled}
        preparing={draft.status === "processing" || draft.status === "queued"}
        activeSectionId={activeSectionId}
        solo={solo}
        actions={{
          onBack: () => {
            if (!dirty) { back(); return; }
            if (window.confirm("Save changes before leaving?")) {
              void save().then((s) => { if (s) back(); });
            }
          },
          onTitle: (title) => set({ title }),
          onArtist: (artist) => set({ artist }),
          onPlay: () => { void playPause(); },
          onStop: () => { void ipc.stop(); },
          onLoop: () => { void (async () => { if (!(await ensureLoaded())) return; ipc.loop(!usePlayback.getState().status?.looping); })(); },
          onSeek: (t) => { void seek(t); },
          onSelect: selectSection,
          onAddSection: addSectionAt,
          onMoveSection: moveSectionStart,
          onRename: (id, label) => set({ sections: draft.sections.map((s) => (s.id === id ? { ...s, label } : s)) }),
          onRenameFocus: (id) => {
            if (renameArmed.current === id) return;
            renameArmed.current = id;
            remember();
          },
          onKind: (id, kind) => {
            if (renameArmed.current !== id) remember();
            set({ sections: draft.sections.map((s) => (s.id === id ? { ...s, kind } : s)) });
          },
          onDelete: removeSelected,
          onMoveBeat: commitBeat,
          onBpm: commitBpm,
          onKey: (key) => edit({
            key,
            keyPitchClass: pitchClassFromKey(key),
            keyMode: modeFromKey(key),
          }),
          onTimeSignature: applyMeter,
          onCountIn: (countInBars) => edit({ countInBars }),
          onUndo: undo,
          onRedo: redo,
          onTempoStart: remember,
          onTempo: (grid) => writeGrid(grid, false, false),
          onTempoEnd: (grid) => writeGrid(grid, true),
          onTempoPoint: (id) => { setTempoPoint(id); if (id) setSelected(null); },
          onToggleTempoCurve: () => {
            const d = draftRef.current;
            const g = d ? ensureGrid(d) : null;
            if (!g) return;
            const id = tempoPointRef.current ?? g.points[0]?.id;
            const p = g.points.find((x) => x.id === id);
            if (!id || !p) return;
            remember();
            writeGrid(setCurve(g, id, p.curve === "linear" ? "step" : "linear"), true);
          },
          onSnap: () => setSnapMode((m) => (m === "bar" ? "beat" : m === "beat" ? "off" : "bar")),
          onSnapAll: snapToBars,
          onSave: () => { void save(); },
          onRevert: () => {
            if (!stored) return;
            setDraft(stored);
            past.current = [];
            future.current = [];
            setCanUndo(false);
            setCanRedo(false);
            const g = stored.grid ?? (stored.bpm != null ? constantGrid(stored.bpm, stored.beatOffset, beatsPerBar(stored.timeSignature), "user") : null);
            if (g) pushGrid(stored.id, g);
          },
          onDone: () => { void (async () => { if (dirty) await save(); back(); })(); },
          onPrepare: () => { void (async () => { if (dirty) await save(); prepareSongs([draft.id]); })(); },
          onMix: () => setMixOpen(true),
          onMute: (role, muted) => { void muteRole(role, muted); },
          onSolo: (role) => { void soloRole(role); },
          onClickToggle: (on) => { void setClickOn(on); },
          onCueToggle: (on) => { void setCueOn(on); },
          onZoom: focusTime,
        }}
      />
      {mixOpen && (
        <div className="absolute inset-0 z-40 flex justify-end bg-black/45" onClick={() => setMixOpen(false)}>
          <div className="flex h-full w-[min(40rem,100%)] flex-col border-l border-line bg-bg shadow-2xl" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center gap-3 border-b border-line px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold">Song settings</div>
                <div className="text-xs text-muted">Count-in, backing, monitor, arrangement, and the assistant</div>
              </div>
              <Button size="sm" variant="ghost" icon={<X className="size-4" />} onClick={() => setMixOpen(false)}>Close</Button>
            </div>
            <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-4">
              <Card title="Notes for the band">
                <textarea className="field min-h-24 text-sm" placeholder="e.g. Count-in 2 bars. Bridge soft, then build."
                  value={draft.notes} onChange={(e) => set({ notes: e.target.value })} />
              </Card>
              <CountInSettings draft={draft} set={edit} />
              <Card title="Live arrangement" subtitle="Order the band will play. Repeats and skips jump sample-accurately — no re-render needed.">
                <ArrangementEditor sections={sections} rawSections={draft.sections} value={draft.arrangement} onChange={(arrangement) => edit({ arrangement })} />
              </Card>
              <Card title="Backing track" subtitle="What plays in FOH"
                actions={needsRender ? <Badge tone="warn">Needs render</Badge> : draft.rendered.backing ? <Badge tone="ok">Rendered</Badge> : null}>
                <PresetPicker value={draft.prep} presets={presets}
                  onPick={(p) => set({ prep: { ...p.settings, presetId: p.id }, routing: { ...draft.routing, clickEnabled: p.settings.click, cueEnabled: p.settings.cue } })} />
                <div className="my-4 h-px bg-line" />
                <StemControls value={draft.prep} onChange={(prep) => set({ prep, routing: { ...draft.routing, clickEnabled: prep.click, cueEnabled: prep.cue } })} />
                {needsRender && (
                  <div className="mt-4 flex items-center justify-between gap-3 rounded-xl border border-warn/30 bg-warn/[0.07] px-3 py-2.5">
                    <span className="text-xs text-fg-2">Stem settings changed. Re-render takes a few seconds.</span>
                    <Button size="sm" variant="primary" onClick={() => rerender()}>Re-render</Button>
                  </div>
                )}
                {!hasStems && draft.status !== "processing" && (
                  <p className="mt-3 text-xs text-muted">Stems are created when the song is prepared. Until then, live playback uses the original file.</p>
                )}
              </Card>
              <Card title="Monitor" subtitle="Click, cues and guide vocal never reach FOH">
                <div className="space-y-3">
                  <ToggleRow label="Guide vocal in monitor" hint="Separated vocals for the singers" checked={draft.routing.guideVocalToMonitor} disabled={!draft.stems.vocals}
                    onChange={(v) => set({ routing: { ...draft.routing, guideVocalToMonitor: v } })} tone="mon" />
                  <ToggleRow label="Backing track to FOH" hint="Turn off for monitor-only songs" checked={draft.routing.backingToFoh}
                    onChange={(v) => set({ routing: { ...draft.routing, backingToFoh: v } })} />
                  <ToggleRow label="Backing track in monitor" hint="Band hears the track in their in-ears" checked={draft.routing.backingToMonitor}
                    onChange={(v) => set({ routing: { ...draft.routing, backingToMonitor: v } })} tone="mon" />
                </div>
              </Card>
              {isLoaded && loaded && <RehearsalMixer tracks={loaded.tracks} />}
              <Card title="Ask the assistant (optional)" subtitle="Ollama when available · otherwise built-in rules · you always confirm">
                <AiAssistant
                  warmUp
                  placeholder="e.g. “count-in 2 bar” or “geser downbeat ke awal musik”"
                  examples={["Drummer tidak main, bassist main", "Count-in 2 bar", "Geser downbeat ke awal musik", "Pakai tempo yang lebih cepat"]}
                  run={async (text) => {
                    if (dirty) await save();
                    return ipc.aiSong(draft.id, text);
                  }}
                  onApply={applyProposal}
                />
              </Card>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function withTempo(song: Song, patch: { bpm?: number | null; beatOffset?: number; timeSignature?: string }): Song {
  const touchBpm = patch.bpm != null;
  const touchBeat = patch.beatOffset != null;
  const touchMeter = !!patch.timeSignature && patch.timeSignature !== song.timeSignature;
  if (!touchBpm && !touchBeat && !touchMeter) return song;
  const base = touchMeter ? { ...song, timeSignature: patch.timeSignature! } : song;
  const seeded = touchBpm ? { ...base, bpm: patch.bpm! } : base;
  const g0 = ensureGrid(seeded);
  if (!g0) {
    return {
      ...base,
      ...(touchBpm ? { bpm: patch.bpm! } : {}),
      ...(touchBeat ? { beatOffset: patch.beatOffset! } : {}),
    };
  }
  let grid = touchMeter ? remapBeatsPerBar(g0, beatsPerBar(base.timeSignature)) : { ...g0, source: "user" as const };
  if (touchBeat) grid = { ...grid, beat1Sec: Math.max(0, patch.beatOffset!), source: "user" };
  if (touchBpm) {
    const v = Math.round(Math.min(300, Math.max(30, patch.bpm!)) * 1000) / 1000;
    const first = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, grid.beatsPerBar) - qOf(b.bar, b.beat, grid.beatsPerBar))[0];
    if (first) grid = { ...grid, source: "user", points: grid.points.map((p) => (p.id === first.id ? { ...p, bpm: v } : p)) };
  }
  const first = [...grid.points].sort((a, b) => qOf(a.bar, a.beat, grid.beatsPerBar) - qOf(b.bar, b.beat, grid.beatsPerBar))[0];
  return { ...base, grid, bpm: first?.bpm ?? base.bpm, beatOffset: grid.beat1Sec };
}

function CountInSettings({ draft, set }: { draft: Song; set: (p: Partial<Song>) => void }) {
  const beatsPerBarCount = beatsPerBar(draft.timeSignature);
  const beatSec = draft.bpm ? 60 / draft.bpm : null;
  const barSec = beatSec ? beatSec * beatsPerBarCount : null;

  return (
    <Card
      title="Count-in"
      subtitle="Click beats before the intro. Tempo and Beat 1 live in the Beat grid panel on the right."
    >
      <div className="space-y-3">
        <div>
          <div className="label mb-2">Bars before intro</div>
          <div className="grid grid-cols-4 gap-1.5">
            {[0, 1, 2, 4].map((n) => (
              <button
                key={n}
                type="button"
                onClick={() => set({ countInBars: n })}
                className={cn(
                  "flex h-14 flex-col items-center justify-center rounded-xl border text-sm font-semibold transition-colors",
                  draft.countInBars === n
                    ? "border-accent bg-accent/15 text-accent"
                    : "border-line text-muted hover:border-line-2 hover:text-fg",
                )}
              >
                {n === 0 ? (
                  <><span className="text-xl">—</span><span className="mt-0.5 text-[0.65rem] font-normal">No count</span></>
                ) : (
                  <><span className="text-2xl">{n}</span><span className="mt-0.5 text-[0.65rem] font-normal">{n === 1 ? "bar" : "bars"}</span></>
                )}
              </button>
            ))}
          </div>
          {draft.countInBars > 0 && barSec && (
            <p className="mt-2 text-xs text-faint">
              Count-in duration: ~{fmtTime(draft.countInBars * barSec, true)}
              {" "}({draft.countInBars * beatsPerBarCount} beats at {draft.bpm ?? "—"} BPM)
            </p>
          )}
        </div>
        <div>
          <div className="label mb-1.5">Voice cue lead</div>
          <select className="field" value={draft.cueLeadBeats} onChange={(e) => set({ cueLeadBeats: Number(e.target.value) })}>
            {[2, 4, 6, 8].map((n) => <option key={n} value={n}>{n} beats before</option>)}
          </select>
          <p className="mt-1 text-xs text-faint">How many beats before the next section the voice cue fires.</p>
        </div>
      </div>
    </Card>
  );
}

function RehearsalMixer({ tracks }: { tracks: TrackInfo[] }) {
  const [state, setState] = useState(tracks);
  useEffect(() => setState(tracks), [tracks]);
  const names: Record<string, string> = { backing: "Backing", backup: "Backup", original: "Original", vocals: "Guide vocal", drums: "Drums", bass: "Bass", other: "Keys / guitars" };
  const update = (role: string, patch: Partial<TrackInfo>) => {
    setState((s) => s.map((t) => (t.role === role ? { ...t, ...patch } : t)));
    const t = state.find((x) => x.role === role)!;
    const next = { ...t, ...patch };
    ipc.setTrack(next.role, next.gainDb, next.muted);
  };
  return (
    <Card title="Rehearsal mixer" subtitle="Temporary — does not change the prepared song">
      <div className="space-y-2.5">
        {state.map((t) => (
          <div key={t.role} className="grid grid-cols-[6.5rem_minmax(0,1fr)_3.5rem_auto] items-center gap-3">
            <div className="truncate text-sm">
              {names[t.role] ?? t.role}
              <div className="flex gap-1 text-[0.6rem] font-bold">
                {t.toFoh && <span className="text-foh">FOH</span>}
                {t.toMon && <span className="text-mon">MON</span>}
              </div>
            </div>
            <Slider value={t.gainDb} min={-30} max={6} step={0.5} onChange={(v) => update(t.role, { gainDb: v })} tone={t.toFoh ? "accent" : "mon"} disabled={t.muted} />
            <span className="tnum text-right text-xs text-muted">{fmtDb(t.gainDb)}</span>
            <button onClick={() => update(t.role, { muted: !t.muted })}
              className={cn("h-7 w-9 rounded-md border text-[0.7rem] font-bold", t.muted ? "border-err/50 bg-err/15 text-err" : "border-line text-muted hover:text-fg")}>M</button>
          </div>
        ))}
      </div>
    </Card>
  );
}

function ToggleRow({ label, hint, checked, onChange, disabled, tone }: {
  label: string; hint: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; tone?: "mon";
}) {
  return (
    <label className={cn("flex items-center justify-between gap-4", disabled && "opacity-50")}>
      <div>
        <div className="text-sm">{label}</div>
        <div className="text-xs text-muted">{hint}</div>
      </div>
      <Switch checked={checked} onChange={onChange} disabled={disabled} tone={tone === "mon" ? "mon" : "accent"} />
    </label>
  );
}

export function NumInput({ value, onChange, min, max, step = 1, placeholder, suffix }: {
  value: number | null; onChange: (v: number | null) => void; min?: number; max?: number; step?: number; placeholder?: string; suffix?: string;
}) {
  const [text, setText] = useState(value == null ? "" : String(value));
  useEffect(() => setText(value == null ? "" : String(Math.round(value * 1000) / 1000)), [value]);
  const commit = () => {
    if (text.trim() === "") return onChange(null);
    const n = Number(text.replace(",", "."));
    if (!Number.isFinite(n)) return setText(value == null ? "" : String(value));
    const c = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, n));
    onChange(Math.round(c / step) * step);
  };
  return (
    <div className="relative min-w-0 flex-1">
      <input className="field tnum pr-7" inputMode="decimal" value={text} placeholder={placeholder}
        onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
      {suffix && <span className="pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-xs text-faint">{suffix}</span>}
    </div>
  );
}

/** "1:23.4" | "83.4" → seconds */
export function parseTime(s: string): number | null {
  const t = s.trim();
  if (!t) return null;
  const m = t.match(/^(\d+):(\d{1,2}(?:[.,]\d+)?)$/);
  if (m) return Number(m[1]) * 60 + Number(m[2].replace(",", "."));
  const n = Number(t.replace(",", "."));
  return Number.isFinite(n) && n >= 0 ? n : null;
}

function guessNextKind(sections: Section[]) {
  const last = sections.at(-1)?.kind;
  const order: Record<string, Section["kind"]> = { intro: "verse", verse: "chorus", prechorus: "chorus", chorus: "verse", bridge: "chorus", instrumental: "verse" };
  return (last ? order[last] ?? "verse" : "intro") as "verse";
}

/** Fields the user can edit (for dirty checking). */
function editable(s: Song) {
  return {
    title: s.title, artist: s.artist, bpm: s.bpm, key: s.key, ts: s.timeSignature, off: s.beatOffset, prep: s.prep, routing: s.routing,
    sections: s.sections, arr: s.arrangement, ci: s.countInBars, cl: s.cueLeadBeats, notes: s.notes, grid: s.grid ?? null,
  };
}
