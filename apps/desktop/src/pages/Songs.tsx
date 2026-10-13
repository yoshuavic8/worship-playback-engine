import * as Menu from "@radix-ui/react-dropdown-menu";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ArrowDownUp, Copy, FolderOpen, MoreHorizontal, Music2, Pause, PenLine, Play, Search, Sparkles, Trash2, Upload,
} from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { KeyBpm, SongStatusBadge, StemPills, presetName } from "@/components/SongBits";
import { Button, Confirm, Empty, IconButton, PageHeader, Segmented } from "@/components/ui";
import { acceptSongReview, pickAndImport, playSong, prepareSongs } from "@/lib/actions";
import { cn, fmtTime } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { isRolling } from "@/lib/transport";
import type { Song } from "@/lib/types";
import { useApp, usePlayback } from "@/store/app";

type Filter = "all" | "ready" | "todo" | "issues";
type Sort = "title" | "artist" | "updated" | "bpm" | "key";

export function Songs() {
  const songs = useApp((s) => s.songs);
  const navigate = useApp((s) => s.navigate);
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("title");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [del, setDel] = useState<Song[] | null>(null);

  const list = useMemo(() => {
    const needle = q.trim().toLowerCase();
    let l = songs.filter((s) =>
      !needle || s.title.toLowerCase().includes(needle) || s.artist.toLowerCase().includes(needle) || (s.key ?? "").toLowerCase() === needle,
    );
    if (filter === "ready") l = l.filter((s) => s.status === "ready" || s.status === "warning");
    if (filter === "todo") l = l.filter((s) => s.status === "imported" || s.status === "queued" || s.status === "processing");
    if (filter === "issues") l = l.filter((s) => s.status === "error" || s.status === "warning" || !s.bpm);
    const cmp: Record<Sort, (a: Song, b: Song) => number> = {
      title: (a, b) => a.title.localeCompare(b.title),
      artist: (a, b) => a.artist.localeCompare(b.artist) || a.title.localeCompare(b.title),
      updated: (a, b) => b.updatedAt.localeCompare(a.updatedAt),
      bpm: (a, b) => (a.bpm ?? 999) - (b.bpm ?? 999),
      key: (a, b) => (a.key ?? "~").localeCompare(b.key ?? "~"),
    };
    return [...l].sort(cmp[sort]);
  }, [songs, q, filter, sort]);

  const counts = useMemo(() => ({
    ready: songs.filter((s) => s.status === "ready" || s.status === "warning").length,
    todo: songs.filter((s) => s.status === "imported" || s.status === "queued" || s.status === "processing").length,
    issues: songs.filter((s) => s.status === "error" || s.status === "warning" || !s.bpm).length,
  }), [songs]);

  const selected = list.filter((s) => sel.has(s.id));
  const toggle = (id: string) => setSel((p) => {
    const n = new Set(p);
    if (n.has(id)) n.delete(id);
    else n.add(id);
    return n;
  });

  const remove = async (items: Song[]) => {
    for (const s of items) {
      try {
        await ipc.deleteSong(s.id);
      } catch (e) {
        toast.error(`Could not delete ${s.title}`, { description: errMsg(e) });
      }
    }
    setSel(new Set());
    useApp.getState().refreshSongs();
    toast.success(items.length === 1 ? `Deleted “${items[0].title}”` : `Deleted ${items.length} songs`);
  };

  return (
    <div className="pb-10">
      <PageHeader
        title="Songs"
        subtitle={`${songs.length} songs · ${counts.ready} live-ready`}
        actions={<Button variant="primary" icon={<Upload className="size-4" />} onClick={pickAndImport}>Import songs</Button>}
      >
        <div className="flex items-center gap-3">
          <div className="relative w-72">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-faint" />
            <input className="field pl-9" placeholder="Search title, artist or key…" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <Segmented<Filter>
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: "All" },
              { value: "ready", label: `Live-ready ${counts.ready}` },
              { value: "todo", label: `To prepare ${counts.todo}` },
              { value: "issues", label: `Needs attention ${counts.issues}` },
            ]}
          />
          <div className="flex-1" />
          <label className="flex items-center gap-2 text-xs text-muted">
            <ArrowDownUp className="size-3.5" />
            <select className="field h-8 w-36 text-sm" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
              <option value="title">Title</option>
              <option value="artist">Artist</option>
              <option value="updated">Recently updated</option>
              <option value="bpm">BPM</option>
              <option value="key">Key</option>
            </select>
          </label>
        </div>
      </PageHeader>

      {songs.length === 0 ? (
        <Empty icon={<Music2 className="size-6" />} title="No songs yet"
          action={<Button variant="primary" size="lg" icon={<Upload className="size-4" />} onClick={pickAndImport}>Import songs</Button>}>
          Drag audio files onto the window or import MP3, WAV, FLAC, M4A or AIFF. The app separates stems,
          detects BPM, key and sections, and renders backing tracks — all locally.
        </Empty>
      ) : (
        <div className="px-8 pt-4">
          {selected.length > 0 && (
            <div className="mb-3 flex items-center gap-3 rounded-xl border border-accent/30 bg-accent/[0.07] px-4 py-2.5">
              <span className="text-sm font-medium">{selected.length} selected</span>
              <Button size="sm" variant="primary" icon={<Sparkles className="size-3.5" />} onClick={() => { prepareSongs(selected.map((s) => s.id)); setSel(new Set()); }}>Prepare</Button>
              <Button size="sm" variant="outline" onClick={() => { prepareSongs(selected.map((s) => s.id), { separate: false }); setSel(new Set()); }}>Analyze only</Button>
              <Button size="sm" variant="ghost" className="text-err" icon={<Trash2 className="size-3.5" />} onClick={() => setDel(selected)}>Delete</Button>
              <div className="flex-1" />
              <Button size="sm" variant="ghost" onClick={() => setSel(new Set())}>Clear</Button>
            </div>
          )}
          <div className="card overflow-hidden">
            <div className="grid grid-cols-[2rem_2.5rem_minmax(0,1fr)_9rem_4.5rem_5rem_8rem_8.5rem_2.5rem] items-center gap-3 border-b border-line px-3 py-2 text-[0.7rem] font-semibold tracking-wider text-muted uppercase">
              <input type="checkbox" className="accent-[#f4b740]" aria-label="Select all"
                checked={list.length > 0 && list.every((s) => sel.has(s.id))}
                onChange={(e) => setSel(e.target.checked ? new Set(list.map((s) => s.id)) : new Set())} />
              <span />
              <span>Title</span>
              <span>Key · BPM</span>
              <span>Length</span>
              <span>Stems</span>
              <span>Preset</span>
              <span>Status</span>
              <span />
            </div>
            {list.map((s) => (
              <SongRow key={s.id} song={s} selected={sel.has(s.id)} onToggle={() => toggle(s.id)} onOpen={() => navigate("song", s.id)} onDelete={() => setDel([s])} />
            ))}
            {list.length === 0 && <div className="px-4 py-10 text-center text-sm text-muted">No songs match.</div>}
          </div>
        </div>
      )}

      <Confirm
        open={!!del}
        onOpenChange={(v) => !v && setDel(null)}
        title={del && del.length > 1 ? `Delete ${del.length} songs?` : "Delete song?"}
        message={<>This removes {del && del.length > 1 ? "these songs" : <b>{del?.[0]?.title}</b>} and their stems and backing tracks from the library. Setlists that use {del && del.length > 1 ? "them" : "it"} will skip {del && del.length > 1 ? "them" : "it"}.</>}
        confirmLabel="Delete"
        danger
        onConfirm={() => del && remove(del)}
      />
    </div>
  );
}

function SongRow({ song, selected, onToggle, onOpen, onDelete }: {
  song: Song; selected: boolean; onToggle: () => void; onOpen: () => void; onDelete: () => void;
}) {
  const loadedId = useApp((s) => s.loaded?.songId);
  const playing = usePlayback((s) => s.status?.songId === song.id && isRolling(s.status?.state));
  const isLoaded = loadedId === song.id;
  return (
    <div
      onDoubleClick={onOpen}
      className={cn(
        "group grid grid-cols-[2rem_2.5rem_minmax(0,1fr)_9rem_4.5rem_5rem_8rem_8.5rem_2.5rem] items-center gap-3 border-b border-line/60 px-3 py-2 last:border-0 hover:bg-white/[0.025]",
        selected && "bg-accent/[0.05]",
      )}
    >
      <input type="checkbox" className="accent-[#f4b740]" checked={selected} onChange={onToggle} aria-label={`Select ${song.title}`} />
      <IconButton label={playing ? "Pause" : "Rehearse"} size="sm" variant={isLoaded ? "secondary" : "ghost"}
        className={cn("rounded-full", playing && "text-accent")}
        onClick={() => (playing ? ipc.pause() : playSong(song.id))}>
        {playing ? <Pause className="size-3.5" fill="currentColor" /> : <Play className="size-3.5" fill="currentColor" />}
      </IconButton>
      <button className="min-w-0 text-left" onClick={onOpen}>
        <div className="truncate text-[0.92rem] font-medium group-hover:text-accent-2">{song.title}</div>
        <div className="truncate text-xs text-muted">{song.artist || "Unknown artist"}{song.sections.length ? ` · ${song.sections.length} sections` : ""}</div>
      </button>
      <KeyBpm song={song} className="text-sm" />
      <span className="tnum text-sm text-fg-2">{fmtTime(song.duration)}</span>
      <StemPills song={song} />
      <span className="truncate text-xs text-fg-2">{presetName(song.prep.presetId)}</span>
      <span title={song.lastError ?? undefined}><SongStatusBadge song={song} /></span>
      <Menu.Root>
        <Menu.Trigger asChild>
          <button className="grid size-8 place-items-center rounded-md text-muted opacity-60 hover:bg-white/5 hover:text-fg group-hover:opacity-100" aria-label="More">
            <MoreHorizontal className="size-4" />
          </button>
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Content align="end" sideOffset={4} className="z-50 min-w-48 rounded-xl border border-line-2 bg-panel-2 p-1 shadow-pop">
            <MenuItem icon={<PenLine className="size-4" />} onSelect={onOpen}>Edit song</MenuItem>
            <MenuItem icon={<Sparkles className="size-4" />} onSelect={() => prepareSongs([song.id])}>
              {song.status === "imported" ? "Prepare" : "Prepare again"}
            </MenuItem>
            <MenuItem icon={<Sparkles className="size-4 opacity-0" />} onSelect={() => prepareSongs([song.id], { separate: false })}>Re-analyze only</MenuItem>
            {song.status === "warning" && (
              <MenuItem icon={<Sparkles className="size-4 opacity-0" />} onSelect={() => acceptSongReview(song.id)
                .then((s) => toast.success(s.status === "ready" ? `“${s.title}” is live-ready` : "Still needs a look"))
                .catch((e) => toast.error(errMsg(e)))}>
                Tempo is correct
              </MenuItem>
            )}
            <MenuItem icon={<Copy className="size-4" />} onSelect={() => ipc.duplicateSong(song.id).then(() => toast.success("Duplicated")).catch((e) => toast.error(errMsg(e)))}>Duplicate</MenuItem>
            <MenuItem icon={<FolderOpen className="size-4" />} onSelect={() => revealItemInDir(`${song.dir}/song.json`).catch(() => {})}>Show in Finder</MenuItem>
            <Menu.Separator className="my-1 h-px bg-line" />
            <MenuItem icon={<Trash2 className="size-4" />} danger onSelect={onDelete}>Delete…</MenuItem>
          </Menu.Content>
        </Menu.Portal>
      </Menu.Root>
    </div>
  );
}

export function MenuItem({ children, icon, onSelect, danger, disabled }: {
  children: React.ReactNode; icon?: React.ReactNode; onSelect: () => void; danger?: boolean; disabled?: boolean;
}) {
  return (
    <Menu.Item
      disabled={disabled}
      onSelect={onSelect}
      className={cn(
        "flex h-8 cursor-default items-center gap-2.5 rounded-lg px-2.5 text-sm outline-none data-[disabled]:opacity-40 data-[highlighted]:bg-white/[0.07]",
        danger ? "text-err" : "text-fg-2 data-[highlighted]:text-fg",
      )}
    >
      <span className="text-muted">{icon}</span>
      {children}
    </Menu.Item>
  );
}
