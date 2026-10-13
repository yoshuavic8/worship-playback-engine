import * as Menu from "@radix-ui/react-dropdown-menu";
import { CalendarDays, Copy, FileDown, FileUp, ListMusic, MoreHorizontal, Plus, Radio, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { Badge, Button, Confirm, Empty, PageHeader } from "@/components/ui";
import { openDialog, saveDialog } from "@/lib/actions";
import { fmtDate, fmtDateShort, fmtDuration, relTime } from "@/lib/format";
import { t } from "@/lib/locale";
import { errMsg, ipc } from "@/lib/ipc";
import type { Setlist } from "@/lib/types";
import { useApp } from "@/store/app";
import { MenuItem } from "./Songs";

export async function newSetlist(name?: string, songIds: string[] = []) {
  const d = nextSunday();
  try {
    const s = await ipc.saveSetlist({
      id: "", name: name ?? `${t("sundayService")} · ${fmtDateShort(d.toISOString().slice(0, 10))}`,
      date: d.toISOString().slice(0, 10), notes: "", createdAt: "", updatedAt: "",
      items: songIds.map((songId, i) => ({
        id: "", songId, position: i, presetId: null, clickEnabled: null, cueEnabled: null, fohEnabled: null, monitorBacking: null,
        startSectionId: null, arrangement: null, transition: "stop", notes: "",
      })),
    });
    await useApp.getState().refreshSetlists();
    return s;
  } catch (e) {
    toast.error("Could not create setlist", { description: errMsg(e) });
    return null;
  }
}

function nextSunday() {
  const d = new Date();
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7 || 7));
  return d;
}

export function Setlists() {
  const setlists = useApp((s) => s.setlists);
  const songs = useApp((s) => s.songs);
  const navigate = useApp((s) => s.navigate);
  const [del, setDel] = useState<Setlist | null>(null);
  const today = new Date().toISOString().slice(0, 10);

  const groups = useMemo(() => {
    const sorted = [...setlists].sort((a, b) => (b.date ?? "").localeCompare(a.date ?? "") || b.updatedAt.localeCompare(a.updatedAt));
    return {
      upcoming: sorted.filter((s) => s.date && s.date >= today).reverse(),
      other: sorted.filter((s) => !s.date || s.date < today),
    };
  }, [setlists, today]);

  const importFile = async () => {
    const p = await openDialog("Setlist", "json");
    if (!p) return;
    try {
      const s = await ipc.importSetlist(p);
      toast.success(`Imported “${s.name}”`);
      navigate("setlist", s.id);
    } catch (e) {
      toast.error("Import failed", { description: errMsg(e) });
    }
  };

  const card = (s: Setlist) => {
    const items = s.items.map((i) => songs.find((x) => x.id === i.songId));
    const dur = items.reduce((a, x) => a + (x?.duration ?? 0), 0);
    const notReady = items.filter((x) => !x || (x.status !== "ready" && x.status !== "warning")).length;
    return (
      <div key={s.id} role="button" tabIndex={0} aria-label={`Open ${s.name}`}
        onClick={() => navigate("setlist", s.id)}
        onKeyDown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); navigate("setlist", s.id); } }}
        className="card group relative cursor-pointer p-5 transition-colors hover:border-line-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-xs text-muted"><CalendarDays className="size-3.5" />{s.date ? fmtDate(s.date) : "No date"}</div>
            <div className="mt-1.5 truncate text-[1.05rem] font-semibold">{s.name}</div>
          </div>
          <div onClick={(e) => e.stopPropagation()}>
            <Menu.Root>
              <Menu.Trigger asChild>
                <button className="grid size-8 place-items-center rounded-md text-muted hover:bg-white/5 hover:text-fg" aria-label="More"><MoreHorizontal className="size-4" /></button>
              </Menu.Trigger>
              <Menu.Portal>
                <Menu.Content align="end" sideOffset={4} className="z-50 min-w-44 rounded-xl border border-line-2 bg-panel-2 p-1 shadow-pop">
                  <MenuItem icon={<Copy className="size-4" />} onSelect={() => ipc.duplicateSetlist(s.id).then((d) => toast.success(`Created “${d.name}”`))}>Duplicate</MenuItem>
                  <MenuItem icon={<FileDown className="size-4" />} onSelect={async () => {
                    const p = await saveDialog(`${s.name}.wpe-setlist.json`, "Setlist", "json");
                    if (p) ipc.exportSetlist(s.id, p).then(() => toast.success("Exported")).catch((e) => toast.error(errMsg(e)));
                  }}>Export…</MenuItem>
                  <Menu.Separator className="my-1 h-px bg-line" />
                  <MenuItem icon={<Trash2 className="size-4" />} danger onSelect={() => setDel(s)}>Delete…</MenuItem>
                </Menu.Content>
              </Menu.Portal>
            </Menu.Root>
          </div>
        </div>
        <ol className="mt-3 space-y-1">
          {items.slice(0, 4).map((x, i) => (
            <li key={i} className="flex gap-2 truncate text-sm text-fg-2"><span className="tnum w-4 text-right text-faint">{i + 1}</span><span className="truncate">{x?.title ?? "Missing song"}</span></li>
          ))}
          {items.length > 4 && <li className="pl-6 text-xs text-muted">+ {items.length - 4} more</li>}
          {items.length === 0 && <li className="text-sm text-muted">Empty</li>}
        </ol>
        <div className="mt-4 flex items-center gap-2 border-t border-line pt-3 text-xs text-muted">
          <span>{items.length} songs · {fmtDuration(dur)}</span>
          <span className="text-faint">· edited {relTime(s.updatedAt)}</span>
          <span className="flex-1" />
          {notReady > 0 ? <Badge tone="warn">{notReady} not ready</Badge> : items.length > 0 && <Badge tone="ok">Ready</Badge>}
          <Button size="sm" variant="secondary" icon={<Radio className="size-3.5" />} disabled={!items.length}
            onClick={(e) => { e.stopPropagation(); navigate("live", s.id); }}>Live</Button>
        </div>
      </div>
    );
  };

  return (
    <div className="pb-10">
      <PageHeader
        title="Setlists"
        subtitle="Build the order of service, then go live"
        actions={
          <>
            <Button variant="ghost" icon={<FileUp className="size-4" />} onClick={importFile}>Import</Button>
            <Button variant="primary" icon={<Plus className="size-4" />} onClick={async () => { const s = await newSetlist(); if (s) navigate("setlist", s.id); }}>New setlist</Button>
          </>
        }
      />
      {setlists.length === 0 ? (
        <Empty icon={<ListMusic className="size-6" />} title="No setlists"
          action={<Button variant="primary" size="lg" icon={<Plus className="size-4" />} onClick={async () => { const s = await newSetlist(); if (s) navigate("setlist", s.id); }}>Create your first setlist</Button>}>
          A setlist is the running order for a service: songs, per-song presets, start sections and transitions.
        </Empty>
      ) : (
        <div className="space-y-8 px-8 pt-6">
          {groups.upcoming.length > 0 && (
            <section>
              <h2 className="label mb-3">Upcoming</h2>
              <div className="grid grid-cols-2 gap-4 2xl:grid-cols-3">{groups.upcoming.map(card)}</div>
            </section>
          )}
          {groups.other.length > 0 && (
            <section>
              <h2 className="label mb-3">{groups.upcoming.length ? "Past & undated" : "All setlists"}</h2>
              <div className="grid grid-cols-2 gap-4 2xl:grid-cols-3">{groups.other.map(card)}</div>
            </section>
          )}
        </div>
      )}
      <Confirm open={!!del} onOpenChange={(v) => !v && setDel(null)} title="Delete setlist?" danger confirmLabel="Delete"
        message={<>“{del?.name}” will be deleted. Songs stay in the library.</>}
        onConfirm={() => del && ipc.deleteSetlist(del.id).then(() => toast.success("Setlist deleted")).catch((e) => toast.error(errMsg(e)))} />
    </div>
  );
}
