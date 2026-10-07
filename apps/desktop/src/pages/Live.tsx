import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AlertTriangle, ArrowLeft, Drum, Headphones, ListMusic, Loader2, Maximize2, Mic, Minimize2, Pause, Play, Radio, Repeat,
  SkipBack, SkipForward, Square, Volume1,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Metronome } from "@/components/icons";
import { KeyBpm } from "@/components/SongBits";
import { PanicButton, StateBadge, planLen, planPos, segOffset } from "@/components/Transport";
import { Badge, Button, Meter, StatusDot, Tip } from "@/components/ui";
import { cn, fmtTime } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { sectionColor } from "@/lib/sections";
import { DEFAULT_SHORTCUTS, keyLabel } from "@/lib/shortcuts";
import {
  fadeOut, gotoSong, isRolling, jumpTo, stepSong, stop, toggleClick, toggleCue, toggleLoop, togglePlay,
} from "@/lib/transport";
import type { LiveInfo } from "@/lib/types";
import { useApp, usePlayback } from "@/store/app";

export function Live() {
  const route = useApp((s) => s.route);
  const live = useApp((s) => s.live);
  const setlists = useApp((s) => s.setlists);
  const navigate = useApp((s) => s.navigate);
  const [opening, setOpening] = useState(false);

  const wanted = route.id ?? live?.setlist.id;
  const stored = setlists.find((s) => s.id === wanted);

  useEffect(() => {
    if (!wanted) return;
    const stale = !live || live.setlist.id !== wanted || (stored && stored.updatedAt !== live.setlist.updatedAt);
    if (!stale || isRolling(usePlayback.getState().status?.state)) return;
    setOpening(true);
    ipc.liveOpen(wanted)
      .then(async (info) => {
        useApp.getState().setLive(info);
        useApp.getState().setLoaded(await ipc.loaded());
      })
      .catch((e) => {
        toast.error("Cannot start live mode", { description: errMsg(e) });
        navigate("setlist", wanted);
      })
      .finally(() => setOpening(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted, stored?.updatedAt]);

  if (!wanted) return <ChooseSetlist />;
  if (!live || opening) {
    return (
      <div className="grid h-full place-items-center bg-[#08090a] text-muted">
        <div className="flex items-center gap-3"><Loader2 className="size-5 animate-spin" />Loading setlist & first song…</div>
      </div>
    );
  }
  return <LiveView live={live} />;
}

function LiveView({ live }: { live: LiveInfo }) {
  const loaded = useApp((s) => s.loaded);
  const navigate = useApp((s) => s.navigate);
  const audio = useApp((s) => s.audioStatus);
  const custom = useApp((s) => s.settings?.shortcuts);
  const shortcuts = useMemo(() => ({ ...DEFAULT_SHORTCUTS, ...(custom ?? {}) }), [custom]);
  const status = usePlayback((s) => s.status);
  const [full, setFull] = useState(false);
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 1000);
    getCurrentWindow().isFullscreen().then(setFull).catch(() => {});
    return () => clearInterval(t);
  }, []);

  const rolling = isRolling(status?.state);
  const index = loaded?.liveIndex ?? live.index;
  const song = live.songs[index];
  const item = live.setlist.items[index];
  const next = live.songs[index + 1];
  const segs = loaded?.segments ?? [];
  const segIdx = status?.segmentIndex ?? 0;
  const seg = segs[segIdx];
  const queuedIdx = status?.queuedIndex ?? null;
  const upcoming = queuedIdx != null ? segs[queuedIdx] : status?.looping ? seg : segs[segIdx + 1];
  const pos = status?.songId === loaded?.songId ? status?.position ?? 0 : 0;
  const total = planLen(segs);
  const planned = planPos(segs, segIdx, pos);
  const remaining = Math.max(0, total - planned);
  const segRemaining = seg ? Math.max(0, seg.end - pos) : 0;
  const beat = loaded?.bpm ? 60 / loaded.bpm : 0.5;
  const countIn = status?.state === "COUNT_IN" ? Math.max(1, Math.ceil((status.countInRemaining ?? 0) / beat)) : null;
  const optionalDrums = loaded?.tracks.find((t) => t.role === "drums");
  const guide = loaded?.tracks.find((t) => t.role === "vocals");

  const toggleFull = async () => {
    const w = getCurrentWindow();
    const f = !(await w.isFullscreen());
    await w.setFullscreen(f);
    setFull(f);
  };

  const fallback = loaded && loaded.source !== "backing";

  return (
    <div className="flex h-full flex-col bg-[#08090a]">
      {/* top bar */}
      <div className="drag flex h-14 shrink-0 items-center gap-4 border-b border-line/70 pr-4 pl-20">
        <Button size="sm" variant="ghost" className="no-drag" icon={<ArrowLeft className="size-4" />} onClick={() => navigate("setlist", live.setlist.id)}>Setlist</Button>
        <div className="flex items-center gap-2">
          <span className="flex items-center gap-1.5 rounded-md bg-live px-2 py-0.5 text-[0.7rem] font-black tracking-[0.14em] text-white"><Radio className="size-3" />LIVE</span>
          <span className="truncate font-semibold">{live.setlist.name}</span>
        </div>
        <div className="flex-1" />
        <div className="no-drag flex items-center gap-2 rounded-lg border border-line px-3 py-1.5 text-xs">
          <StatusDot tone={audio?.running ? "ok" : "err"} />
          <span className="text-fg-2">{audio?.running?.deviceName ?? "No device"}</span>
          <span className="text-faint">·</span>
          <span className="text-foh">FOH {describe(audio?.settings.routing.foh)}</span>
          <span className="text-mon">MON {describe(audio?.settings.routing.monitor)}</span>
        </div>
        <span className="tnum text-lg font-semibold text-fg-2">{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
        <Tip label={full ? "Exit full screen" : "Full screen"}>
          <button className="no-drag grid size-9 place-items-center rounded-lg text-muted hover:bg-white/5 hover:text-fg" onClick={toggleFull}>
            {full ? <Minimize2 className="size-4" /> : <Maximize2 className="size-4" />}
          </button>
        </Tip>
        <div className="no-drag"><PanicButton /></div>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* setlist */}
        <aside className="flex w-[19rem] shrink-0 flex-col border-r border-line/70">
          <div className="label flex items-center gap-2 px-5 pt-4 pb-2"><ListMusic className="size-3.5" />Running order</div>
          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto px-3 pb-3">
            {live.songs.map((s, i) => {
              const cur = i === index;
              const it = live.setlist.items[i];
              return (
                <button key={`${s.id}-${i}`} onClick={() => i !== index && gotoSong(i)} disabled={rolling && !cur}
                  className={cn("flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors disabled:cursor-not-allowed",
                    cur ? "bg-accent/12 ring-1 ring-accent/40" : i < index ? "opacity-45 hover:opacity-80" : "hover:bg-white/[0.04]")}>
                  <span className={cn("tnum w-5 text-center text-sm font-bold", cur ? "text-accent" : "text-faint")}>{i + 1}</span>
                  <div className="min-w-0 flex-1">
                    <div className={cn("truncate text-[0.95rem] font-semibold", cur && "text-accent-2")}>{s.title}</div>
                    <KeyBpm song={s} className="text-xs" />
                  </div>
                  {it?.transition === "auto" && i < live.songs.length - 1 && <Tip label="Next song starts automatically"><SkipForward className="size-3.5 text-mon" /></Tip>}
                  {s.status !== "ready" && s.status !== "warning" && <Tip label="Not prepared — original file will play"><AlertTriangle className="size-3.5 text-warn" /></Tip>}
                </button>
              );
            })}
          </div>
          {(live.setlist.notes || item?.notes) && (
            <div className="border-t border-line/70 px-5 py-3 text-xs text-fg-2">
              {item?.notes && <div className="mb-1.5"><span className="font-semibold text-accent">Song: </span>{item.notes}</div>}
              {live.setlist.notes && <div className="whitespace-pre-line text-muted">{live.setlist.notes}</div>}
            </div>
          )}
        </aside>

        {/* stage */}
        <main className="flex min-w-0 flex-1 flex-col px-10 py-8">
          <div className="flex items-start justify-between gap-6">
            <div className="min-w-0">
              <div className="flex items-center gap-3">
                <StateBadge state={status?.state} fading={status?.fading} />
                <span className="text-xs text-muted">Song {index + 1} of {live.songs.length}</span>
                {fallback && <Badge tone="warn"><AlertTriangle className="size-3" />Playing {loaded?.source}</Badge>}
              </div>
              <h1 className="mt-1 truncate text-[2.6rem] leading-tight font-bold tracking-tight">{song?.title}</h1>
              <div className="text-base text-muted">{song?.artist}</div>
            </div>
            <div className="flex shrink-0 gap-3">
              <BigStat label="Key" value={loaded?.key ?? song?.key ?? "—"} />
              <BigStat label="BPM" value={loaded?.bpm ? String(Math.round(loaded.bpm * 10) / 10) : "—"} />
              <BigStat label="Time" value={loaded?.timeSignature ?? "4/4"} />
            </div>
          </div>

          {/* now / next */}
          <div className="mt-8 grid grid-cols-[1fr_auto_1fr] items-stretch gap-4">
            <div className="rounded-2xl border border-line bg-panel/60 px-6 py-5">
              <div className="label">Now</div>
              <div className="mt-1 truncate text-[2rem] font-bold" style={{ color: seg ? sectionColor(seg.kind) : undefined }}>{seg?.label ?? "—"}</div>
              <div className="tnum mt-1 text-sm text-muted">{fmtTime(segRemaining)} left in section</div>
            </div>
            <div className="grid w-52 place-items-center rounded-2xl border border-line bg-panel/60">
              {countIn != null ? (
                <div className="text-center">
                  <div className="label text-accent">Count-in</div>
                  <div className="tnum text-[4rem] leading-none font-black text-accent">{countIn}</div>
                </div>
              ) : (
                <div className="text-center">
                  <div className="tnum text-[2.6rem] leading-none font-bold">{fmtTime(planned)}</div>
                  <div className="tnum mt-1.5 text-sm text-muted">−{fmtTime(remaining)}</div>
                </div>
              )}
            </div>
            <div className={cn("rounded-2xl border px-6 py-5", queuedIdx != null ? "border-mon/50 bg-mon/[0.07]" : "border-line bg-panel/60")}>
              <div className="label flex items-center gap-2">
                {status?.looping ? <><Repeat className="size-3.5 text-accent" /><span className="text-accent">Looping</span></> : queuedIdx != null ? <span className="text-mon">Jump queued</span> : "Next"}
              </div>
              <div className="mt-1 truncate text-[2rem] font-bold" style={{ color: upcoming ? sectionColor(upcoming.kind) : undefined }}>
                {upcoming?.label ?? (next ? "End of song" : "End of service")}
              </div>
              <div className="mt-1 truncate text-sm text-muted">
                {upcoming ? (queuedIdx != null ? "At the end of this section" : "In order") : next ? `Then: ${next.title}${item?.transition === "auto" ? " (auto)" : ""}` : ""}
              </div>
            </div>
          </div>

          {/* section timeline */}
          <div className="mt-8">
            <div className="mb-2 flex items-center justify-between text-xs text-muted">
              <span>Click a section to jump there at the end of the current one · <span className="text-fg-2">Shift-click</span> jumps now</span>
              {queuedIdx != null && <button className="text-mon hover:underline" onClick={() => ipc.cancelJump()}>Cancel jump</button>}
            </div>
            <div className="flex h-24 gap-1.5">
              {segs.map((s, i) => {
                const w = ((s.end - s.start) / total) * 100;
                const cur = i === segIdx;
                const q = i === queuedIdx;
                const done = i < segIdx;
                const color = sectionColor(s.kind);
                const prog = cur ? Math.min(1, Math.max(0, (pos - s.start) / (s.end - s.start))) : done ? 1 : 0;
                return (
                  <button key={i} onClick={(e) => jumpTo(i, e.shiftKey)} title={`${s.label} · ${fmtTime(s.end - s.start)}`}
                    style={{ flexGrow: w, flexBasis: 0, borderColor: cur || q ? color : "transparent" }}
                    className={cn("relative min-w-[3.2rem] overflow-hidden rounded-xl border-2 bg-panel text-left transition-colors hover:bg-panel-2", q && "animate-pulse-soft")}>
                    <div className="absolute inset-y-0 left-0" style={{ width: `${prog * 100}%`, background: color, opacity: cur ? 0.28 : 0.1 }} />
                    <div className="absolute inset-x-0 top-0 h-1" style={{ background: color, opacity: cur ? 1 : 0.5 }} />
                    <div className="relative px-2.5 pt-3">
                      <div className={cn("truncate text-sm font-bold", !cur && "text-fg-2")} style={cur ? { color } : undefined}>{s.label}</div>
                      <div className="tnum truncate text-[0.7rem] text-muted">{fmtTime(segOffset(segs, i))}</div>
                    </div>
                    {q && <div className="absolute right-2 bottom-2 rounded bg-mon px-1.5 text-[0.62rem] font-black text-black">NEXT</div>}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="flex-1" />

          {/* transport */}
          <div className="mt-8 flex items-center justify-center gap-4">
            <LiveBtn label={`Previous song (${keyLabel(shortcuts.prevSong)})`} onClick={() => stepSong(-1)} disabled={index === 0 || rolling}><SkipBack className="size-6" fill="currentColor" /></LiveBtn>
            <LiveBtn label={`Stop (${keyLabel(shortcuts.stop)})`} onClick={stop}><Square className="size-6" fill="currentColor" /></LiveBtn>
            <button onClick={togglePlay} aria-label="Play / Pause"
              className={cn("grid size-24 place-items-center rounded-full shadow-pop transition-transform active:scale-95",
                rolling ? "bg-white text-black" : "bg-accent text-accent-ink hover:bg-accent-2")}>
              {rolling ? <Pause className="size-10" fill="currentColor" /> : <Play className="size-10 translate-x-1" fill="currentColor" />}
            </button>
            <LiveBtn label={`Fade out (${keyLabel(shortcuts.fadeOut)})`} onClick={() => fadeOut(4)} disabled={!rolling}><Volume1 className="size-6" /></LiveBtn>
            <LiveBtn label={`Next song (${keyLabel(shortcuts.nextSong)})`} onClick={() => stepSong(1)} disabled={!next || rolling}><SkipForward className="size-6" fill="currentColor" /></LiveBtn>
          </div>
          <div className="mt-5 flex flex-wrap justify-center gap-x-5 gap-y-1 text-xs text-faint">
            {([["playPause", "Play/Pause"], ["prevSection", "Prev section"], ["nextSection", "Next section"], ["loopSection", "Loop"], ["toggleClick", "Click"], ["panic", "PANIC"]] as const).map(([k, l]) => (
              <span key={k} className="flex items-center gap-1.5"><span className="kbd">{keyLabel(shortcuts[k])}</span>{l}</span>
            ))}
          </div>
        </main>

        {/* monitor panel */}
        <aside className="flex w-[17rem] shrink-0 flex-col gap-4 border-l border-line/70 px-4 py-5">
          <div>
            <div className="label mb-2">Monitor (in-ears)</div>
            <div className="space-y-2">
              <MonToggle icon={<Metronome className="size-5" />} label="Click" on={!!loaded?.clickEnabled} disabled={!loaded?.clickAvailable} onClick={toggleClick}
                hint={loaded?.clickAvailable ? undefined : "No BPM set"} />
              <MonToggle icon={<Mic className="size-5" />} label="Voice cues" on={!!loaded?.cueEnabled} disabled={!loaded?.cueAvailable} onClick={toggleCue}
                hint={loaded?.cueAvailable ? undefined : "No sections"} />
              {guide && <MonToggle icon={<Headphones className="size-5" />} label="Guide vocal" on={!guide.muted} onClick={() => setTrack(guide.role, guide.gainDb, !guide.muted)} />}
            </div>
          </div>
          {optionalDrums && (
            <div>
              <div className="label mb-2">Backing</div>
              <MonToggle icon={<Drum className="size-5" />} label="Drums in track" on={!optionalDrums.muted} onClick={() => setTrack("drums", optionalDrums.gainDb, !optionalDrums.muted)} tone="foh" />
            </div>
          )}
          <div>
            <div className="label mb-2">Section</div>
            <MonToggle icon={<Repeat className="size-5" />} label="Loop section" on={!!status?.looping} onClick={toggleLoop} tone="foh" />
          </div>
          <div className="flex-1" />
          <div className="rounded-2xl border border-line bg-panel/60 p-4">
            <div className="flex h-40 justify-center gap-6">
              <MeterPair label="FOH" l={status?.meters[0] ?? 0} r={status?.meters[1] ?? 0} color="text-foh" />
              <MeterPair label="MON" l={status?.meters[2] ?? 0} r={status?.meters[3] ?? 0} color="text-mon" />
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

function setTrack(role: Parameters<typeof ipc.setTrack>[0], gainDb: number, muted: boolean) {
  const l = useApp.getState().loaded;
  if (l) useApp.getState().setLoaded({ ...l, tracks: l.tracks.map((t) => (t.role === role ? { ...t, muted } : t)) });
  ipc.setTrack(role, gainDb, muted);
}

function describe(t?: { kind: string; channel?: number; left?: number; right?: number }) {
  if (!t || t.kind === "none") return "off";
  if (t.kind === "mono") return `${(t.channel ?? 0) + 1}`;
  return `${(t.left ?? 0) + 1}-${(t.right ?? 1) + 1}`;
}

function BigStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-[5.5rem] rounded-2xl border border-line bg-panel/60 px-4 py-3 text-center">
      <div className="label">{label}</div>
      <div className="tnum mt-0.5 text-[1.7rem] leading-tight font-bold">{value}</div>
    </div>
  );
}

function LiveBtn({ label, children, onClick, disabled }: { label: string; children: React.ReactNode; onClick: () => void; disabled?: boolean }) {
  return (
    <Tip label={label}>
      <button aria-label={label} onClick={onClick} disabled={disabled}
        className="grid size-16 place-items-center rounded-full border border-line-2 bg-panel text-fg-2 transition-colors hover:bg-panel-2 hover:text-fg disabled:opacity-25">
        {children}
      </button>
    </Tip>
  );
}

function MonToggle({ icon, label, on, onClick, disabled, hint, tone = "mon" }: {
  icon: React.ReactNode; label: string; on: boolean; onClick: () => void; disabled?: boolean; hint?: string; tone?: "mon" | "foh";
}) {
  const onCls = tone === "mon" ? "border-mon/50 bg-mon/12 text-mon" : "border-accent/50 bg-accent/12 text-accent";
  return (
    <button onClick={onClick} disabled={disabled}
      className={cn("flex h-14 w-full items-center gap-3 rounded-xl border px-4 text-left transition-colors disabled:opacity-35", on ? onCls : "border-line bg-panel text-fg-2 hover:border-line-2")}>
      {icon}
      <div className="min-w-0 flex-1">
        <div className="text-sm font-semibold">{label}</div>
        {hint && <div className="text-[0.7rem] text-muted">{hint}</div>}
      </div>
      <span className="text-xs font-black tracking-wider">{on ? "ON" : "OFF"}</span>
    </button>
  );
}

function MeterPair({ label, l, r, color }: { label: string; l: number; r: number; color: string }) {
  return (
    <div className="flex flex-col items-center gap-2">
      <div className="flex flex-1 gap-1">
        <Meter vertical value={l} className="w-3" />
        <Meter vertical value={r} className="w-3" />
      </div>
      <span className={cn("text-[0.68rem] font-black tracking-wider", color)}>{label}</span>
    </div>
  );
}

function ChooseSetlist() {
  const setlists = useApp((s) => s.setlists);
  const songs = useApp((s) => s.songs);
  const navigate = useApp((s) => s.navigate);
  const sorted = useMemo(() => [...setlists].sort((a, b) => (b.date ?? "").localeCompare(a.date ?? "")), [setlists]);
  return (
    <div className="flex h-full flex-col">
      <div className="drag flex h-14 shrink-0 items-center gap-3 border-b border-line/70 pl-20">
        <Button size="sm" variant="ghost" className="no-drag" icon={<ArrowLeft className="size-4" />} onClick={() => navigate("dashboard")}>Back</Button>
      </div>
      <div className="mx-auto w-full max-w-2xl px-6 py-14">
        <h1 className="text-2xl font-semibold">Start live mode</h1>
        <p className="mt-1 text-sm text-muted">Choose the setlist for this service.</p>
        <div className="mt-6 space-y-2">
          {sorted.map((s) => (
            <button key={s.id} onClick={() => navigate("live", s.id)} disabled={!s.items.length}
              className="card flex w-full items-center gap-4 px-5 py-4 text-left hover:border-line-2 disabled:opacity-40">
              <Radio className="size-5 text-live" />
              <div className="min-w-0 flex-1">
                <div className="truncate font-semibold">{s.name}</div>
                <div className="text-xs text-muted">{s.date ?? "No date"} · {s.items.length} songs · {s.items.filter((i) => { const x = songs.find((y) => y.id === i.songId); return x?.status === "ready" || x?.status === "warning"; }).length} ready</div>
              </div>
              <span className="text-sm text-accent">Go live →</span>
            </button>
          ))}
          {!sorted.length && <div className="rounded-xl border border-dashed border-line-2 px-5 py-8 text-center text-sm text-muted">No setlists yet. <button className="text-accent hover:underline" onClick={() => navigate("setlists")}>Create one</button></div>}
        </div>
      </div>
    </div>
  );
}
