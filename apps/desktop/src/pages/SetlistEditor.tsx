import { DndContext, KeyboardSensor, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  AlertTriangle, ArrowLeft, CheckCircle2, ChevronDown, GripVertical, Link2, Loader2, Plus, Radio, Search, ShieldCheck, Sparkles, Trash2, XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { AiAssistant } from "@/components/AiAssistant";
import { KeyBpm, SongStatusBadge, presetName } from "@/components/SongBits";
import { Badge, Button, Card, Dialog, IconButton, Segmented, Tip } from "@/components/ui";
import { prepareSongs } from "@/lib/actions";
import { cn, fmtDuration, fmtTime } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { buildPlan, resolveSections } from "@/lib/sections";
import type { PreflightReport, Setlist, SetlistItem, Song } from "@/lib/types";
import { useApp } from "@/store/app";

const emptyItem = (songId: string): SetlistItem => ({
  id: crypto.randomUUID(), songId, position: 0, presetId: null, clickEnabled: null, cueEnabled: null, fohEnabled: null,
  monitorBacking: null, startSectionId: null, arrangement: null, transition: "stop", notes: "",
});

export function SetlistEditor({ id }: { id: string }) {
  const stored = useApp((s) => s.setlists.find((x) => x.id === id));
  const songs = useApp((s) => s.songs);
  const navigate = useApp((s) => s.navigate);
  const back = useApp((s) => s.back);
  const [draft, setDraft] = useState<Setlist | null>(stored ?? null);
  const [picker, setPicker] = useState(false);
  const [open, setOpen] = useState<string | null>(null);
  const [report, setReport] = useState<PreflightReport | null>(null);
  const [checking, setChecking] = useState(false);
  const saveTimer = useRef<number | undefined>(undefined);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));

  useEffect(() => {
    if (!stored) {
      ipc.getSetlist(id).then(setDraft).catch(() => {});
    } else if (!draft) setDraft(stored);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const runPreflight = useCallback(async (quiet = false) => {
    setChecking(true);
    try {
      const next = await ipc.preflight(id);
      setReport(next);
      if (!quiet) toast.success(`Pre-service check: ${next.verdict}`);
    } catch (e) {
      if (!quiet) toast.error("Pre-service check failed", { description: errMsg(e) });
    } finally {
      setChecking(false);
    }
  }, [id]);

  // Autosave (debounced) — setlists are small, saving is cheap, and nothing gets lost before a service.
  const update = (patch: Partial<Setlist>) => {
    setDraft((d) => {
      if (!d) return d;
      const next = { ...d, ...patch };
      window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(async () => {
        try {
          await ipc.saveSetlist(next);
          runPreflight(true);
        } catch (e) {
          toast.error("Could not save setlist", { description: errMsg(e) });
        }
      }, 450);
      return next;
    });
  };

  useEffect(() => {
    runPreflight(true);
  }, [runPreflight, songs]);

  const songOf = (sid: string) => songs.find((s) => s.id === sid);
  const items = draft?.items ?? [];
  const total = useMemo(() => items.reduce((a, it) => {
    const s = songOf(it.songId);
    if (!s) return a;
    return a + buildPlan(s, it.arrangement).reduce((x, p) => x + p.end - p.start, 0);
  }, 0), [items, songs]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!draft) return <div className="p-10 text-muted">Loading…</div>;

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    const from = items.findIndex((x) => x.id === e.active.id);
    const to = items.findIndex((x) => x.id === e.over!.id);
    update({ items: arrayMove(items, from, to) });
  };

  const setItem = (iid: string, patch: Partial<SetlistItem>) => update({ items: items.map((x) => (x.id === iid ? { ...x, ...patch } : x)) });
  const unprepared = items.map((it) => songOf(it.songId)).filter((s): s is Song => !!s && (s.status === "imported" || s.status === "error"));

  return (
    <div className="pb-12">
      <div className="drag sticky top-0 z-20 border-b border-line/70 bg-bg/90 px-8 pt-6 pb-4 backdrop-blur-md">
        <div className="no-drag flex items-center gap-4">
          <IconButton label="Back" onClick={back}><ArrowLeft className="size-4" /></IconButton>
          <div className="min-w-0 flex-1">
            <input className="w-full bg-transparent text-[1.5rem] font-semibold tracking-tight outline-none" value={draft.name} onChange={(e) => update({ name: e.target.value })} />
            <div className="mt-0.5 flex items-center gap-3 text-sm text-muted">
              <input type="date" className="bg-transparent text-sm text-muted outline-none [color-scheme:dark]" value={draft.date ?? ""} onChange={(e) => update({ date: e.target.value || null })} />
              <span>· {items.length} songs · {fmtDuration(total)}</span>
            </div>
          </div>
          <Button icon={<Plus className="size-4" />} onClick={() => setPicker(true)}>Add songs</Button>
          <Button variant="primary" size="lg" icon={<Radio className="size-4" />} disabled={!items.length}
            onClick={async () => {
              window.clearTimeout(saveTimer.current);
              await ipc.saveSetlist(draft).catch(() => {});
              navigate("live", draft.id);
            }}>
            Go live
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-12 gap-5 px-8 pt-5">
        <div className="col-span-12 xl:col-span-8">
          {items.length === 0 ? (
            <div className="card flex flex-col items-center px-6 py-14 text-center">
              <div className="text-base font-semibold">This setlist is empty</div>
              <p className="mt-1 text-sm text-muted">Add songs from your library. Drag to reorder.</p>
              <Button variant="primary" className="mt-5" icon={<Plus className="size-4" />} onClick={() => setPicker(true)}>Add songs</Button>
            </div>
          ) : (
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
              <SortableContext items={items.map((x) => x.id)} strategy={verticalListSortingStrategy}>
                <div className="space-y-2">
                  {items.map((it, i) => (
                    <ItemRow key={it.id} index={i} item={it} song={songOf(it.songId)} open={open === it.id} onToggle={() => setOpen(open === it.id ? null : it.id)}
                      onChange={(p) => setItem(it.id, p)} onRemove={() => update({ items: items.filter((x) => x.id !== it.id) })}
                      issues={report?.songs.find((s) => s.songId === it.songId)} isLast={i === items.length - 1} />
                  ))}
                </div>
              </SortableContext>
            </DndContext>
          )}
          <Card className="mt-5" title="Service notes">
            <textarea className="field min-h-20 text-sm" placeholder="Notes for the team (shown in Live mode)" value={draft.notes} onChange={(e) => update({ notes: e.target.value })} />
          </Card>
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-4">
          <Preflight report={report} checking={checking} onRun={() => runPreflight(false)}
            extra={unprepared.length > 0 && (
              <Button size="sm" variant="primary" icon={<Sparkles className="size-3.5" />} onClick={() => prepareSongs(unprepared.map((s) => s.id))}>
                Prepare {unprepared.length} song{unprepared.length > 1 ? "s" : ""}
              </Button>
            )} />
          <Card title="Suggest with AI" subtitle="Local Ollama builds a draft — you decide">
            <AiAssistant
              placeholder="e.g. “4 lagu: 2 fast opener, 2 slow worship, key berdekatan”"
              examples={["2 lagu cepat lalu 2 lagu lambat", "Worship set in G", "Short set, 3 songs"]}
              run={(text) => ipc.aiSetlist(text)}
              onApply={(p) => {
                const add = p.setlistSongIds.map((sid) => emptyItem(sid));
                update({ items: [...items, ...add], name: draft.items.length === 0 && p.setlistName ? p.setlistName : draft.name });
                toast.success(`Added ${add.length} songs`);
              }}
            />
          </Card>
        </div>
      </div>

      <SongPicker open={picker} onOpenChange={setPicker} existing={items.map((x) => x.songId)}
        onAdd={(ids) => update({ items: [...items, ...ids.map(emptyItem)] })} />
    </div>
  );
}

function ItemRow({ index, item, song, open, onToggle, onChange, onRemove, issues, isLast }: {
  index: number; item: SetlistItem; song?: Song; open: boolean; onToggle: () => void; onChange: (p: Partial<SetlistItem>) => void;
  onRemove: () => void; issues?: { level: string; issues: string[] }; isLast: boolean;
}) {
  const presets = useApp((s) => s.presets);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: item.id });
  const sections = song ? resolveSections(song) : [];
  const overrides = [item.presetId, item.clickEnabled, item.cueEnabled, item.fohEnabled, item.monitorBacking, item.startSectionId].filter((x) => x != null).length;
  const plan = song ? buildPlan(song, item.arrangement) : [];
  const len = plan.reduce((a, p) => a + p.end - p.start, 0);

  return (
    <div ref={setNodeRef} style={{ transform: CSS.Transform.toString(transform), transition, zIndex: isDragging ? 20 : undefined }}
      className={cn("card overflow-hidden", isDragging && "shadow-pop ring-1 ring-accent/40")}>
      <div className="flex items-center gap-3 px-3 py-3">
        <button className="cursor-grab text-faint hover:text-fg-2 active:cursor-grabbing" {...attributes} {...listeners} aria-label="Drag to reorder"><GripVertical className="size-4" /></button>
        <span className="tnum w-6 text-center text-lg font-semibold text-muted">{index + 1}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-semibold">{song?.title ?? "Missing song"}</span>
            {issues && issues.level !== "ok" && (
              <Tip label={issues.issues.join(" · ")}>
                <span>{issues.level === "error" ? <XCircle className="size-4 text-err" /> : <AlertTriangle className="size-4 text-warn" />}</span>
              </Tip>
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-xs text-muted">
            {song && <KeyBpm song={song} className="text-xs" />}
            <span className="text-faint">·</span>
            <span className="tnum">{fmtTime(len)}</span>
            <span className="text-faint">·</span>
            <span>{item.presetId ? presetName(item.presetId) : song ? presetName(song.prep.presetId) : ""}</span>
            {item.startSectionId && <><span className="text-faint">·</span><span className="text-mon">starts at {sections.find((s) => s.id === item.startSectionId)?.label}</span></>}
          </div>
        </div>
        {song && <SongStatusBadge song={song} />}
        {overrides > 0 && <Badge tone="accent">{overrides} override{overrides > 1 ? "s" : ""}</Badge>}
        <Tip label={item.transition === "auto" ? "Next song starts automatically" : "Stops after this song"}>
          <button disabled={isLast} onClick={() => onChange({ transition: item.transition === "auto" ? "stop" : "auto" })}
            className={cn("flex h-7 items-center gap-1 rounded-md border px-2 text-xs font-medium disabled:opacity-30", item.transition === "auto" ? "border-mon/40 bg-mon/10 text-mon" : "border-line text-muted")}>
            <Link2 className="size-3.5" />{item.transition === "auto" ? "Auto" : "Stop"}
          </button>
        </Tip>
        <IconButton size="sm" label="Song options" onClick={onToggle}><ChevronDown className={cn("size-4 transition-transform", open && "rotate-180")} /></IconButton>
        <IconButton size="sm" label="Remove from setlist" onClick={onRemove}><Trash2 className="size-3.5" /></IconButton>
      </div>
      {open && song && (
        <div className="grid grid-cols-2 gap-x-6 gap-y-3 border-t border-line bg-bg-2/50 px-12 py-4">
          <label className="block">
            <span className="label mb-1.5 block">Preset for this service</span>
            <select className="field h-8 text-sm" value={item.presetId ?? ""} onChange={(e) => onChange({ presetId: e.target.value || null })}>
              <option value="">Song default ({presetName(song.prep.presetId)})</option>
              {presets.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
            {item.presetId && <span className="mt-1 block text-xs text-muted">Mixed live from stems — no re-render needed{!song.stems.other && " (needs a prepared song)"}.</span>}
          </label>
          <label className="block">
            <span className="label mb-1.5 block">Start at</span>
            <select className="field h-8 text-sm" value={item.startSectionId ?? ""} onChange={(e) => onChange({ startSectionId: e.target.value || null })}>
              <option value="">Beginning</option>
              {sections.filter((s) => s.id !== "__song").map((s) => <option key={s.id} value={s.id}>{s.label} ({fmtTime(s.start)})</option>)}
            </select>
          </label>
          <TriState label="Click" value={item.clickEnabled} songDefault={song.routing.clickEnabled} onChange={(v) => onChange({ clickEnabled: v })} />
          <TriState label="Voice cues" value={item.cueEnabled} songDefault={song.routing.cueEnabled} onChange={(v) => onChange({ cueEnabled: v })} />
          <TriState label="Backing to FOH" value={item.fohEnabled} songDefault={song.routing.backingToFoh} onChange={(v) => onChange({ fohEnabled: v })} />
          <TriState label="Backing in monitor" value={item.monitorBacking} songDefault={song.routing.backingToMonitor} onChange={(v) => onChange({ monitorBacking: v })} />
          <label className="col-span-2 block">
            <span className="label mb-1.5 block">Notes</span>
            <input className="field h-8 text-sm" placeholder="e.g. Worship leader prays over the intro" value={item.notes} onChange={(e) => onChange({ notes: e.target.value })} />
          </label>
        </div>
      )}
    </div>
  );
}

function TriState({ label, value, songDefault, onChange }: { label: string; value: boolean | null; songDefault: boolean; onChange: (v: boolean | null) => void }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-sm text-fg-2">{label}</span>
      <Segmented<string> size="sm" value={value == null ? "d" : value ? "on" : "off"}
        onChange={(v) => onChange(v === "d" ? null : v === "on")}
        options={[{ value: "d", label: `Song (${songDefault ? "on" : "off"})` }, { value: "on", label: "On" }, { value: "off", label: "Off" }]} />
    </div>
  );
}

export function Preflight({ report, checking, onRun, extra }: { report: PreflightReport | null; checking: boolean; onRun: () => void; extra?: React.ReactNode }) {
  const tone = report?.verdict === "READY" ? "ok" : report?.verdict === "WARNING" ? "warn" : "err";
  return (
    <Card title={<span className="flex items-center gap-2"><ShieldCheck className="size-4 text-muted" />Pre-service check</span>}
      actions={<Button size="sm" variant="ghost" onClick={onRun} loading={checking}>Run</Button>}>
      {!report ? (
        <div className="flex items-center gap-2 text-sm text-muted"><Loader2 className="size-4 animate-spin" />Checking…</div>
      ) : (
        <>
          <div className={cn("flex items-center gap-3 rounded-xl px-3 py-3",
            tone === "ok" ? "bg-ok/10 text-ok" : tone === "warn" ? "bg-warn/10 text-warn" : "bg-err/10 text-err")}>
            {tone === "ok" ? <CheckCircle2 className="size-5" /> : tone === "warn" ? <AlertTriangle className="size-5" /> : <XCircle className="size-5" />}
            <div className="text-sm font-bold tracking-wide">{report.verdict}</div>
          </div>
          <ul className="mt-3 space-y-2">
            {report.checks.map((c) => (
              <li key={c.id} className="flex gap-2.5 text-sm">
                {c.level === "ok" ? <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-ok" /> : c.level === "warning" ? <AlertTriangle className="mt-0.5 size-4 shrink-0 text-warn" /> : <XCircle className="mt-0.5 size-4 shrink-0 text-err" />}
                <div className="min-w-0">
                  <div className="text-fg-2">{c.label}</div>
                  {c.detail && <div className="text-xs text-muted">{c.detail}</div>}
                </div>
              </li>
            ))}
          </ul>
          {extra && <div className="mt-3">{extra}</div>}
        </>
      )}
    </Card>
  );
}

function SongPicker({ open, onOpenChange, existing, onAdd }: { open: boolean; onOpenChange: (v: boolean) => void; existing: string[]; onAdd: (ids: string[]) => void }) {
  const songs = useApp((s) => s.songs);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState<string[]>([]);
  useEffect(() => {
    if (open) { setSel([]); setQ(""); }
  }, [open]);
  const list = songs
    .filter((s) => !q || `${s.title} ${s.artist} ${s.key ?? ""}`.toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => a.title.localeCompare(b.title));
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title="Add songs" description="Select in the order you want them added." width={600}
      footer={<>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>Cancel</Button>
        <Button variant="primary" disabled={!sel.length} onClick={() => { onAdd(sel); onOpenChange(false); }}>Add {sel.length || ""} song{sel.length === 1 ? "" : "s"}</Button>
      </>}>
      <div className="relative mb-3">
        <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-faint" />
        <input autoFocus className="field pl-9" placeholder="Search…" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      <div className="space-y-0.5">
        {list.map((s) => {
          const n = sel.indexOf(s.id);
          return (
            <button key={s.id} onClick={() => setSel(n >= 0 ? sel.filter((x) => x !== s.id) : [...sel, s.id])}
              className={cn("flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left", n >= 0 ? "bg-accent/10" : "hover:bg-white/[0.04]")}>
              <span className={cn("tnum grid size-5 place-items-center rounded-md border text-[0.68rem] font-bold", n >= 0 ? "border-accent bg-accent text-accent-ink" : "border-line-2")}>{n >= 0 ? n + 1 : ""}</span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">{s.title}</div>
                <div className="truncate text-xs text-muted">{s.artist || "Unknown artist"}{existing.includes(s.id) && " · already in setlist"}</div>
              </div>
              <KeyBpm song={s} className="text-xs" />
              <SongStatusBadge song={s} />
            </button>
          );
        })}
        {list.length === 0 && <div className="py-8 text-center text-sm text-muted">No songs found.</div>}
      </div>
    </Dialog>
  );
}
