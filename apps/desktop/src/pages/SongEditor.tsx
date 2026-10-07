import {
  ArrowLeft, Crosshair, Headphones, Loader2, Magnet, Pause, Play, Plus, RotateCcw, Save, Sparkles, Square, Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { AiAssistant } from "@/components/AiAssistant";
import { ArrangementEditor } from "@/components/Arrangement";
import { PresetPicker, StemControls, samePrep } from "@/components/StemSettings";
import { SongStatusBadge } from "@/components/SongBits";
import { Waveform } from "@/components/Waveform";
import { Badge, Button, Card, IconButton, Slider, Switch, Tip } from "@/components/ui";
import { loadSong, prepareSongs } from "@/lib/actions";
import { cn, fmtDb, fmtTime } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { SECTION_KINDS, nextLabel, resolveSections, sectionColor } from "@/lib/sections";
import { isRolling } from "@/lib/transport";
import type { AiProposal, Section, Song, TrackInfo, Waveform as WF } from "@/lib/types";
import { useApp, usePlayback } from "@/store/app";

export const KEYS = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
export const MINOR_KEYS = ["Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm"];
const TIME_SIGS = ["4/4", "3/4", "6/8", "2/4", "12/8", "5/4"];

export function SongEditor({ id }: { id: string }) {
  const stored = useApp((s) => s.songs.find((x) => x.id === id));
  const presets = useApp((s) => s.presets);
  const back = useApp((s) => s.back);
  const loaded = useApp((s) => s.loaded);
  const [draft, setDraft] = useState<Song | null>(stored ?? null);
  const [saving, setSaving] = useState(false);
  const [wave, setWave] = useState<WF | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
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
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  const sections = useMemo(() => (draft ? resolveSections(draft) : []), [draft]);
  if (!draft) return <div className="p-10 text-muted">Song not found.</div>;

  const playhead = status?.position ?? 0;
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

  const needsRender = !!draft.stems.other && !!draft.analysis?.renderedWith && !samePrep(draft.analysis.renderedWith, draft.prep);
  const hasStems = !!draft.stems.other && !!draft.stems.bass && !!draft.stems.drums;
  const notPrepared = draft.status === "imported" || draft.status === "error";

  const applyProposal = async (p: AiProposal) => {
    const next: Song = { ...draft, prep: p.prep ?? draft.prep, routing: p.routing ?? draft.routing, arrangement: p.arrangement ?? draft.arrangement };
    setDraft(next);
    const saved = await save(next);
    if (saved && p.requiresRerender) {
      if (hasStems) await rerender(saved);
      else prepareSongs([saved.id]);
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
    const kind = guessNextKind(draft.sections);
    const s: Section = { id: crypto.randomUUID(), label: nextLabel(draft.sections, kind), kind, start: Math.round(t * 100) / 100 };
    set({ sections: [...draft.sections, s].sort((a, b) => a.start - b.start) });
    setSelected(s.id);
  };

  const snapToBars = () => {
    if (!draft.bpm) return toast.warning("Set a BPM first");
    const beats = Number(draft.timeSignature.split("/")[0]) || 4;
    const bar = (60 / draft.bpm) * beats;
    set({ sections: draft.sections.map((s, i) => (i === 0 ? s : { ...s, start: Math.max(0, Math.round((s.start - draft.beatOffset) / bar) * bar + draft.beatOffset) })) });
    toast.success("Section markers snapped to bars");
  };

  return (
    <div className="pb-12">
      {/* header */}
      <div className="drag sticky top-0 z-20 border-b border-line/70 bg-bg/90 px-8 pt-6 pb-4 backdrop-blur-md">
        <div className="no-drag flex items-center gap-4">
          <IconButton label="Back" onClick={back}><ArrowLeft className="size-4" /></IconButton>
          <div className="min-w-0 flex-1">
            <input className="w-full truncate bg-transparent text-[1.5rem] font-semibold tracking-tight outline-none placeholder:text-faint" value={draft.title}
              onChange={(e) => set({ title: e.target.value })} placeholder="Song title" />
            <div className="flex items-center gap-3">
              <input className="w-64 bg-transparent text-sm text-muted outline-none placeholder:text-faint" value={draft.artist}
                onChange={(e) => set({ artist: e.target.value })} placeholder="Artist" />
              <SongStatusBadge song={draft} />
              {draft.analysis?.separator && (
                <span className="text-xs text-faint">Prepared with {draft.analysis.separator.engine} on {draft.analysis.separator.device} · {Math.round(draft.analysis.separator.seconds)} s</span>
              )}
            </div>
          </div>
          {dirty && (
            <>
              <span className="text-xs text-warn">Unsaved changes</span>
              <Button variant="ghost" icon={<RotateCcw className="size-4" />} onClick={() => stored && setDraft(stored)}>Revert</Button>
            </>
          )}
          <Button variant={dirty ? "primary" : "secondary"} icon={<Save className="size-4" />} loading={saving} disabled={!dirty} onClick={() => save()}>Save</Button>
          <Button
            variant={notPrepared ? "primary" : "outline"}
            icon={draft.status === "processing" || draft.status === "queued" ? <Loader2 className="size-4 animate-spin" /> : <Sparkles className="size-4" />}
            disabled={draft.status === "processing" || draft.status === "queued"}
            onClick={async () => { if (dirty) await save(); prepareSongs([draft.id]); }}
          >
            {notPrepared ? "Prepare song" : draft.status === "processing" || draft.status === "queued" ? "Preparing…" : "Prepare again"}
          </Button>
        </div>
        {draft.lastError && <div className="no-drag mt-3 rounded-lg border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">{draft.lastError}</div>}
        {(draft.analysis?.warnings?.length ?? 0) > 0 && draft.status === "warning" && (
          <div className="no-drag mt-3 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">{draft.analysis!.warnings!.join(" · ")}</div>
        )}
      </div>

      <div className="grid grid-cols-12 gap-5 px-8 pt-5">
        <div className="col-span-12 space-y-5 2xl:col-span-8 xl:col-span-7">
          {/* waveform + transport */}
          <Card className="overflow-hidden">
            <div className="mb-3 flex items-center gap-2">
              <IconButton label={rolling ? "Pause" : "Rehearse from playhead"} size="lg" variant="secondary" className={cn("rounded-full", rolling && "bg-accent text-accent-ink hover:bg-accent-2")} onClick={playPause}>
                {rolling ? <Pause className="size-5" fill="currentColor" /> : <Play className="size-5 translate-x-px" fill="currentColor" />}
              </IconButton>
              <IconButton label="Stop" onClick={() => ipc.stop()} disabled={!isLoaded}><Square className="size-4" fill="currentColor" /></IconButton>
              <span className="tnum ml-1 text-lg font-semibold">{fmtTime(playhead, true)}</span>
              <span className="tnum text-sm text-muted">/ {fmtTime(draft.duration)}</span>
              {isLoaded && <Badge tone="mon" className="ml-2"><Headphones className="size-3" />{loaded?.source}</Badge>}
              <div className="flex-1" />
              <Tip label="Add a section marker at the playhead">
                <Button size="sm" variant="outline" icon={<Plus className="size-3.5" />} onClick={() => addSectionAt(playhead)}>Marker at playhead</Button>
              </Tip>
              <Tip label="Move every marker to the nearest bar line">
                <Button size="sm" variant="ghost" icon={<Magnet className="size-3.5" />} onClick={snapToBars} disabled={!draft.bpm || draft.sections.length < 2}>Snap to bars</Button>
              </Tip>
            </div>
            <Waveform
              data={wave}
              duration={draft.duration}
              sections={draft.sections.length ? sections : []}
              position={isLoaded ? playhead : undefined}
              height={150}
              onSeek={seek}
              onMoveMarker={(sid, t) => set({ sections: draft.sections.map((s) => (s.id === sid ? { ...s, start: Math.round(t * 100) / 100 } : s)).sort((a, b) => a.start - b.start) })}
              onSelectSection={setSelected}
              selectedId={selected}
              activeSectionId={isLoaded && status ? loaded?.segments[status.segmentIndex]?.sectionId : null}
              beatGrid={draft.bpm ? { bpm: draft.bpm, offset: draft.beatOffset, beatsPerBar: Number(draft.timeSignature.split("/")[0]) || 4 } : null}
            />
            <p className="mt-2 text-xs text-faint">Click to seek · drag boundaries to adjust · bar lines follow BPM and downbeat offset</p>
          </Card>

          {/* sections */}
          <Card title="Sections" subtitle="Used for cues, the live timeline and jumps"
            actions={<Button size="sm" variant="ghost" icon={<Plus className="size-3.5" />} onClick={() => addSectionAt(draft.sections.length ? Math.min(draft.duration - 1, playhead || (draft.sections.at(-1)!.start + 10)) : 0)}>Add</Button>}>
            {draft.sections.length === 0 ? (
              <div className="rounded-xl border border-dashed border-line-2 px-4 py-6 text-center text-sm text-muted">
                No sections yet. Prepare the song to detect them, or add markers while listening.
              </div>
            ) : (
              <div className="space-y-1">
                {sections.map((s, i) => (
                  <div key={s.id} onClick={() => setSelected(s.id)}
                    className={cn("grid grid-cols-[0.5rem_minmax(0,1fr)_9rem_6.5rem_2rem_2rem] items-center gap-2.5 rounded-lg px-2 py-1.5", selected === s.id ? "bg-white/[0.05]" : "hover:bg-white/[0.025]")}>
                    <span className="h-6 w-1.5 rounded-full" style={{ background: sectionColor(s.kind) }} />
                    <input className="field h-8 text-sm" value={s.label} onChange={(e) => set({ sections: draft.sections.map((x) => (x.id === s.id ? { ...x, label: e.target.value } : x)) })} />
                    <select className="field h-8 text-sm" value={s.kind} onChange={(e) => set({ sections: draft.sections.map((x) => (x.id === s.id ? { ...x, kind: e.target.value } : x)) })}>
                      {SECTION_KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.label}</option>)}
                    </select>
                    <TimeInput value={s.start} disabled={i === 0} onChange={(t) => set({ sections: draft.sections.map((x) => (x.id === s.id ? { ...x, start: t } : x)).sort((a, b) => a.start - b.start) })} />
                    <IconButton size="sm" label="Set to playhead" disabled={i === 0 || !isLoaded}
                      onClick={(e) => { e.stopPropagation(); set({ sections: draft.sections.map((x) => (x.id === s.id ? { ...x, start: Math.round(playhead * 100) / 100 } : x)).sort((a, b) => a.start - b.start) }); }}>
                      <Crosshair className="size-3.5" />
                    </IconButton>
                    <IconButton size="sm" label="Delete section"
                      onClick={(e) => { e.stopPropagation(); set({ sections: draft.sections.filter((x) => x.id !== s.id), arrangement: draft.arrangement.filter((a) => a !== s.id) }); }}>
                      <Trash2 className="size-3.5" />
                    </IconButton>
                  </div>
                ))}
              </div>
            )}
          </Card>

          {/* arrangement */}
          <Card title="Live arrangement" subtitle="Order the band will play. Repeats and skips jump sample-accurately — no re-render needed.">
            <ArrangementEditor sections={sections} value={draft.arrangement} onChange={(arrangement) => set({ arrangement })} />
          </Card>

          <Card title="Notes for the band">
            <textarea className="field min-h-24 text-sm" placeholder="e.g. Bridge 2× soft, then build. Key change after last chorus is played live."
              value={draft.notes} onChange={(e) => set({ notes: e.target.value })} />
          </Card>
        </div>

        <div className="col-span-12 space-y-5 2xl:col-span-4 xl:col-span-5">
          <SongInfo draft={draft} set={set} playhead={isLoaded ? playhead : null} />

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

          <Card title="Ask the assistant" subtitle="Local Ollama · suggestions only, you confirm">
            <AiAssistant
              placeholder="e.g. “drummer tidak main, bassist main” or “repeat the chorus twice at the end”"
              examples={["Drummer tidak main, bassist main", "Full track tanpa vokal", "Bass lebih pelan", "Tambah chorus di akhir"]}
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
  );
}

function SongInfo({ draft, set, playhead }: { draft: Song; set: (p: Partial<Song>) => void; playhead: number | null }) {
  const alt = draft.analysis?.detected?.bpmAlternatives ?? [];
  const conf = draft.analysis?.bpmConfidence;
  const keyConf = draft.analysis?.keyConfidence;
  return (
    <Card title="Tempo & key">
      <div className="grid grid-cols-2 gap-3">
        <div>
          <div className="label mb-1.5 flex items-center gap-1.5">BPM {conf != null && conf < 0.6 && <span className="text-warn normal-case tracking-normal">· verify</span>}</div>
          <div className="flex gap-1">
            <NumInput value={draft.bpm} min={20} max={300} step={0.1} placeholder="—" onChange={(bpm) => set({ bpm })} />
            <TapTempo onBpm={(bpm) => set({ bpm })} />
          </div>
          <div className="mt-1.5 flex gap-1">
            <button className="h-6 flex-1 rounded border border-line text-[0.7rem] text-muted hover:text-fg" disabled={!draft.bpm} onClick={() => draft.bpm && set({ bpm: Math.round(draft.bpm * 50) / 100 })}>÷2</button>
            <button className="h-6 flex-1 rounded border border-line text-[0.7rem] text-muted hover:text-fg" disabled={!draft.bpm} onClick={() => draft.bpm && set({ bpm: Math.min(300, draft.bpm * 2) })}>×2</button>
          </div>
          {alt.length > 0 && draft.bpm && !alt.includes(draft.bpm) && (
            <button className="mt-1.5 text-left text-xs text-mon hover:underline" onClick={() => set({ bpm: alt[0] })}>Might be {alt[0]} BPM — use it</button>
          )}
        </div>
        <div>
          <div className="label mb-1.5 flex items-center gap-1.5">Key {keyConf != null && keyConf < 0.3 && <span className="text-warn normal-case tracking-normal">· verify</span>}</div>
          <select className="field" value={draft.key ?? ""} onChange={(e) => set({ key: e.target.value || null })}>
            <option value="">—</option>
            <optgroup label="Major">{KEYS.map((k) => <option key={k} value={k}>{k}</option>)}</optgroup>
            <optgroup label="Minor">{MINOR_KEYS.map((k) => <option key={k} value={k}>{k}</option>)}</optgroup>
          </select>
        </div>
        <div>
          <div className="label mb-1.5">Time signature</div>
          <select className="field" value={draft.timeSignature} onChange={(e) => set({ timeSignature: e.target.value })}>
            {TIME_SIGS.map((t) => <option key={t}>{t}</option>)}
          </select>
        </div>
        <div>
          <div className="label mb-1.5">First downbeat</div>
          <div className="flex gap-1">
            <NumInput value={draft.beatOffset} min={0} max={30} step={0.01} suffix="s" onChange={(v) => set({ beatOffset: v ?? 0 })} />
            <IconButton label="Set downbeat to playhead" variant="outline" disabled={playhead == null} onClick={() => playhead != null && set({ beatOffset: Math.round(playhead * 1000) / 1000 })}>
              <Crosshair className="size-4" />
            </IconButton>
          </div>
        </div>
        <div>
          <div className="label mb-1.5">Count-in</div>
          <select className="field" value={draft.countInBars} onChange={(e) => set({ countInBars: Number(e.target.value) })}>
            <option value={0}>None</option>
            {[1, 2, 4].map((n) => <option key={n} value={n}>{n} bar{n > 1 ? "s" : ""}</option>)}
          </select>
        </div>
        <div>
          <div className="label mb-1.5">Voice cue lead</div>
          <select className="field" value={draft.cueLeadBeats} onChange={(e) => set({ cueLeadBeats: Number(e.target.value) })}>
            {[2, 4, 6, 8].map((n) => <option key={n} value={n}>{n} beats before</option>)}
          </select>
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

function TimeInput({ value, onChange, disabled }: { value: number; onChange: (t: number) => void; disabled?: boolean }) {
  const [text, setText] = useState(fmtTime(value, true));
  useEffect(() => setText(fmtTime(value, true)), [value]);
  const commit = () => {
    const t = parseTime(text);
    if (t == null) setText(fmtTime(value, true));
    else onChange(t);
  };
  return (
    <input className="field tnum h-8 text-sm" value={text} disabled={disabled} onChange={(e) => setText(e.target.value)} onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
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

export function TapTempo({ onBpm }: { onBpm: (bpm: number) => void }) {
  const taps = useRef<number[]>([]);
  const [count, setCount] = useState(0);
  const tap = () => {
    const now = performance.now();
    const t = taps.current;
    if (t.length && now - t[t.length - 1] > 2000) t.length = 0;
    t.push(now);
    if (t.length > 9) t.shift();
    setCount(t.length);
    const bpm = tapBpm(t);
    if (bpm) onBpm(bpm);
  };
  return (
    <Tip label="Tap along with the beat (4+ taps)">
      <button onClick={tap} className={cn("h-9 shrink-0 rounded-lg border px-3 text-xs font-semibold", count > 1 ? "border-accent/50 text-accent" : "border-line text-fg-2 hover:border-line-2")}>
        TAP
      </button>
    </Tip>
  );
}

export function tapBpm(times: number[]): number | null {
  if (times.length < 4) return null;
  const iv = times.slice(1).map((t, i) => t - times[i]);
  const avg = iv.reduce((a, b) => a + b, 0) / iv.length;
  if (avg <= 0) return null;
  return Math.round((60000 / avg) * 10) / 10;
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
    sections: s.sections, arr: s.arrangement, ci: s.countInBars, cl: s.cueLeadBeats, notes: s.notes,
  };
}
