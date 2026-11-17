import {
  ArrowRight, Bot, CalendarDays, CheckCircle2, Cpu, ListPlus, Music2, Radio, ShieldAlert, Sparkles, Upload, Volume2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, PageHeader, Progress, StatusDot } from "@/components/ui";
import { KeyBpm, SongStatusBadge } from "@/components/SongBits";
import { pickAndImport, prepareSongs } from "@/lib/actions";
import { fmtDate, fmtDuration, relTime } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import type { Readiness, Setlist } from "@/lib/types";
import { useApp } from "@/store/app";
import { newSetlist } from "./Setlists";

export function Dashboard() {
  const settings = useApp((s) => s.settings);
  const songs = useApp((s) => s.songs);
  const setlists = useApp((s) => s.setlists);
  const jobs = useApp((s) => s.jobs);
  const navigate = useApp((s) => s.navigate);
  const [ready, setReady] = useState<Readiness | null>(null);

  useEffect(() => {
    ipc.readiness().then(setReady).catch(() => {});
  }, []);

  const stats = useMemo(() => {
    const live = songs.filter((s) => s.status === "ready" || s.status === "warning").length;
    const unprepared = songs.filter((s) => s.status === "imported" || s.status === "error");
    return { live, unprepared, total: songs.length };
  }, [songs]);

  const next = useMemo(() => upcoming(setlists), [setlists]);
  const running = Object.values(jobs).find((j) => j.status === "running");
  const recent = [...songs].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0, 6);
  const hour = new Date().getHours();
  const greet = hour < 11 ? "Good morning" : hour < 15 ? "Good afternoon" : hour < 19 ? "Good evening" : "Good evening";

  return (
    <div className="pb-10">
      <PageHeader
        title={`${greet}${settings?.userName ? `, ${capitalize(settings.userName)}` : ""}`}
        subtitle="Prepare during the week. Play with confidence on Sunday."
        actions={
          <>
            <Button icon={<Upload className="size-4" />} onClick={pickAndImport}>Import songs</Button>
            <Button variant="primary" icon={<ListPlus className="size-4" />} onClick={async () => { const s = await newSetlist(); if (s) navigate("setlist", s.id); }}>
              New setlist
            </Button>
          </>
        }
      />
      <div className="grid grid-cols-12 gap-5 px-8 pt-6">
        {/* Next service */}
        <Card className="col-span-12 xl:col-span-7 overflow-hidden">
          {next ? <NextService setlist={next} /> : (
            <div className="flex items-center gap-5 py-4">
              <div className="grid size-14 place-items-center rounded-2xl bg-accent/10 text-accent"><CalendarDays className="size-6" /></div>
              <div className="flex-1">
                <div className="font-semibold">No setlist yet</div>
                <div className="text-sm text-muted">Create a setlist for the next service — songs, order, presets and transitions.</div>
              </div>
              <Button variant="primary" onClick={async () => { const s = await newSetlist(); if (s) navigate("setlist", s.id); }}>Create setlist</Button>
            </div>
          )}
        </Card>

        {/* System readiness */}
        <Card className="col-span-12 xl:col-span-5" title="System" actions={<Button size="sm" variant="ghost" onClick={() => navigate("diagnostics")}>Details</Button>}>
          <div className="space-y-2.5">
            <ReadyRow icon={<Volume2 className="size-4" />} label="Live playback" value={ready?.playback} hint="Native audio engine, offline" />
            <ReadyRow icon={<Cpu className="size-4" />} label="Stem preparation" value={ready?.preparation} hint={ready?.compat?.separationBackend ?? "Demucs · same models on every Mac"} onFix={() => navigate("diagnostics")} />
            <ReadyRow icon={<Bot className="size-4" />} label="AI assistant (optional)" value={ready?.ai} hint="Ollama or built-in rules" onFix={() => navigate("settings")} />
            {ready?.compat && (
              <div className="rounded-lg border border-line/80 bg-bg-2/40 px-3 py-2 text-xs text-muted">
                <span className="font-medium text-fg-2">{ready.compat.profile === "full" ? "Full profile" : "Lite profile"}</span>
                {" · "}
                {ready.compat.reason}
                {ready.compat.prepEtaWarning && (
                  <div className="mt-1 text-warn">{ready.compat.prepEtaWarning}</div>
                )}
              </div>
            )}
          </div>
        </Card>

        {/* Library stats */}
        <div className="col-span-12 grid grid-cols-3 gap-5">
          <Stat icon={<Music2 className="size-4" />} label="Songs in library" value={stats.total} onClick={() => navigate("songs")} />
          <Stat icon={<CheckCircle2 className="size-4" />} label="Live-ready" value={stats.live} tone="ok" onClick={() => navigate("songs")} />
          <Stat icon={<Sparkles className="size-4" />} label="Need preparation" value={stats.unprepared.length} tone={stats.unprepared.length ? "warn" : undefined}
            action={stats.unprepared.length > 0 && <Button size="sm" variant="outline" onClick={() => prepareSongs(stats.unprepared.map((s) => s.id))}>Prepare all</Button>} />
        </div>

        {/* Preparation activity */}
        {running && (
          <Card className="col-span-12" title="Preparing now" actions={<Button size="sm" variant="ghost" onClick={() => navigate("prep")}>Open queue</Button>}>
            <div className="flex items-center gap-4">
              <div className="min-w-0 flex-1">
                <div className="flex justify-between text-sm"><span className="truncate font-medium">{running.songTitle}</span><span className="tnum text-muted">{Math.round(running.progress * 100)}%</span></div>
                <div className="mt-0.5 truncate text-xs text-muted">{running.message || running.stage}</div>
                <Progress value={running.progress} className="mt-2" />
              </div>
            </div>
          </Card>
        )}

        {/* Recent songs */}
        <Card className="col-span-12" title="Recently updated" actions={<Button size="sm" variant="ghost" onClick={() => navigate("songs")}>All songs <ArrowRight className="size-3.5" /></Button>}>
          {recent.length ? (
            <div className="grid grid-cols-2 gap-x-6 2xl:grid-cols-3">
              {recent.map((s) => (
                <button key={s.id} onClick={() => navigate("song", s.id)} className="flex items-center gap-3 rounded-lg border-b border-line/60 px-2 py-2.5 text-left hover:bg-white/[0.03]">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{s.title}</div>
                    <div className="truncate text-xs text-muted">{s.artist || "Unknown artist"} · {relTime(s.updatedAt)}</div>
                  </div>
                  <KeyBpm song={s} className="text-xs" />
                  <SongStatusBadge song={s} />
                </button>
              ))}
            </div>
          ) : (
            <div className="flex items-center justify-between rounded-xl border border-dashed border-line-2 px-5 py-6">
              <div>
                <div className="font-medium">Your library is empty</div>
                <div className="text-sm text-muted">Drag MP3/WAV files onto this window, or import them. Songs stay on this Mac.</div>
              </div>
              <Button variant="primary" icon={<Upload className="size-4" />} onClick={pickAndImport}>Import songs</Button>
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}

function NextService({ setlist }: { setlist: Setlist }) {
  const songs = useApp((s) => s.songs);
  const navigate = useApp((s) => s.navigate);
  const items = setlist.items.map((i) => songs.find((s) => s.id === i.songId)).filter(Boolean);
  const dur = items.reduce((a, s) => a + (s?.duration ?? 0), 0);
  const notReady = items.filter((s) => s && s.status !== "ready" && s.status !== "warning").length;
  return (
    <div className="flex gap-6">
      <div className="min-w-0 flex-1">
        <div className="label text-accent">Next service</div>
        <div className="mt-1 truncate text-xl font-semibold">{setlist.name}</div>
        <div className="mt-0.5 text-sm text-muted">{setlist.date ? fmtDate(setlist.date) : "No date"} · {items.length} songs · {fmtDuration(dur)}</div>
        <ol className="mt-4 space-y-1.5">
          {items.slice(0, 5).map((s, i) => s && (
            <li key={`${s.id}-${i}`} className="flex items-center gap-3 text-sm">
              <span className="tnum w-5 text-right text-faint">{i + 1}</span>
              <span className="min-w-0 flex-1 truncate">{s.title}</span>
              <KeyBpm song={s} className="text-xs" />
            </li>
          ))}
          {items.length > 5 && <li className="pl-8 text-xs text-muted">+ {items.length - 5} more</li>}
        </ol>
      </div>
      <div className="flex w-44 shrink-0 flex-col justify-between gap-3">
        {notReady > 0 ? (
          <Badge tone="warn" className="self-end"><ShieldAlert className="size-3" />{notReady} not prepared</Badge>
        ) : items.length ? (
          <Badge tone="ok" className="self-end"><CheckCircle2 className="size-3" />All songs ready</Badge>
        ) : <span />}
        <div className="flex flex-col gap-2">
          <Button variant="outline" onClick={() => navigate("setlist", setlist.id)}>Edit setlist</Button>
          <Button variant="primary" size="lg" icon={<Radio className="size-4" />} disabled={!items.length}
            onClick={() => navigate("live", setlist.id)}>Go live</Button>
        </div>
      </div>
    </div>
  );
}

function ReadyRow({ icon, label, value, hint, onFix }: { icon: React.ReactNode; label: string; value?: string; hint: string; onFix?: () => void }) {
  const v = value ?? "…";
  const [tone, text] = verdict(v);
  return (
    <div className="flex items-center gap-3 rounded-lg bg-bg-2/60 px-3 py-2.5">
      <span className="text-muted">{icon}</span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{label}</div>
        <div className="text-xs text-muted">{hint}</div>
      </div>
      <span className="flex items-center gap-2 text-xs font-medium text-fg-2"><StatusDot tone={tone as "ok"} />{text}</span>
      {onFix && tone !== "ok" && tone !== "idle" && <Button size="sm" variant="ghost" onClick={onFix}>Fix</Button>}
    </div>
  );
}

function Stat({ icon, label, value, tone, onClick, action }: {
  icon: React.ReactNode; label: string; value: number; tone?: "ok" | "warn"; onClick?: () => void; action?: React.ReactNode;
}) {
  const color = tone === "ok" ? "text-ok" : tone === "warn" ? "text-warn" : "text-fg";
  return (
    <div className="card flex items-center gap-4 px-5 py-4">
      <div className="grid size-10 place-items-center rounded-xl bg-white/[0.05] text-muted">{icon}</div>
      <button className="min-w-0 flex-1 text-left" onClick={onClick} disabled={!onClick}>
        <div className={`tnum text-2xl font-semibold ${color}`}>{value}</div>
        <div className="text-xs text-muted">{label}</div>
      </button>
      {action}
    </div>
  );
}

export function verdict(v: string): ["ok" | "warn" | "err" | "idle", string] {
  switch (v) {
    case "READY": return ["ok", "Ready"];
    case "ANALYSIS ONLY": return ["warn", "Analysis only — no model"];
    case "SETUP NEEDED": return ["err", "Setup needed"];
    case "NOT READY": return ["err", "Not ready"];
    case "OPTIONAL": return ["idle", "Not connected (optional)"];
    case "…": return ["idle", "Checking…"];
    default: return ["warn", v];
  }
}

function upcoming(setlists: Setlist[]): Setlist | null {
  const today = new Date().toISOString().slice(0, 10);
  const future = setlists.filter((s) => s.date && s.date >= today).sort((a, b) => (a.date! < b.date! ? -1 : 1));
  if (future.length) return future[0];
  return [...setlists].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))[0] ?? null;
}

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
