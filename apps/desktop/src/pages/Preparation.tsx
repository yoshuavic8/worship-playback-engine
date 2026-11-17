import { CheckCircle2, Clock, Cpu, Loader2, RotateCcw, Sparkles, X, XCircle, Zap } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, Empty, PageHeader, Progress } from "@/components/ui";
import { prepareSongs } from "@/lib/actions";
import { relTime } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import type { PreparationJob, ResourceStatus } from "@/lib/types";
import { useApp } from "@/store/app";

const STAGES: [string, string][] = [
  ["validate", "Check"], ["normalize", "Convert"], ["separate", "Separate stems"], ["render", "Render backing"],
  ["analyze", "BPM · key · sections"], ["waveform", "Waveform"], ["qc", "Quality check"],
];

export function Preparation() {
  const jobs = useApp((s) => s.jobs);
  const songs = useApp((s) => s.songs);
  const settings = useApp((s) => s.settings);
  const navigate = useApp((s) => s.navigate);
  const [models, setModels] = useState<ResourceStatus[]>([]);

  useEffect(() => {
    useApp.getState().refreshJobs();
    ipc.listResources().then(setModels).catch(() => {});
  }, []);

  const list = useMemo(() => Object.values(jobs).sort((a, b) => b.createdAt.localeCompare(a.createdAt)), [jobs]);
  const running = list.filter((j) => j.status === "running");
  const queued = list.filter((j) => j.status === "queued").reverse();
  const done = list.filter((j) => !["running", "queued"].includes(j.status)).slice(0, 40);
  const unprepared = songs.filter((s) => s.status === "imported" || s.status === "error");
  const installed = models.filter((m) => m.installed);
  const avg = useMemo(() => {
    const ok = list.filter((j) => j.status === "completed" && j.processingSeconds);
    return ok.length ? ok.reduce((a, j) => a + (j.processingSeconds ?? 0), 0) / ok.length : null;
  }, [list]);

  return (
    <div className="pb-10">
      <PageHeader title="Preparation" subtitle="Stem separation and analysis run here — one song at a time, fully offline"
        actions={unprepared.length > 0 && <Button variant="primary" icon={<Sparkles className="size-4" />} onClick={() => prepareSongs(unprepared.map((s) => s.id))}>Prepare {unprepared.length} waiting</Button>} />
      <div className="grid grid-cols-12 gap-5 px-8 pt-6">
        <div className="col-span-12 space-y-5 xl:col-span-8">
          {running.length === 0 && queued.length === 0 && (
            <Card>
              <Empty icon={<Cpu className="size-6" />} title="Queue is empty">
                {unprepared.length ? `${unprepared.length} song(s) are waiting for preparation.` : "All songs are prepared. Import new songs to add them here."}
              </Empty>
            </Card>
          )}
          {running.map((j) => <RunningJob key={j.id} job={j} />)}
          {queued.length > 0 && (
            <Card title={`Up next · ${queued.length}`}>
              <div className="space-y-1">
                {queued.map((j, i) => (
                  <div key={j.id} className="flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-white/[0.03]">
                    <span className="tnum w-5 text-center text-sm text-faint">{i + 1}</span>
                    <Clock className="size-4 text-muted" />
                    <span className="min-w-0 flex-1 truncate text-sm">{j.songTitle}</span>
                    {avg && <span className="tnum text-xs text-muted">~{Math.round((avg * (i + 1)) / 60)} min</span>}
                    <Button size="sm" variant="ghost" icon={<X className="size-3.5" />} onClick={() => ipc.cancelJob(j.id)}>Remove</Button>
                  </div>
                ))}
              </div>
            </Card>
          )}
          <Card title="History">
            {done.length === 0 ? <div className="text-sm text-muted">No finished jobs yet.</div> : (
              <div className="space-y-0.5">
                {done.map((j) => (
                  <div key={j.id} className="group flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-white/[0.03]">
                    {j.status === "completed" ? <CheckCircle2 className="size-4 text-ok" /> : j.status === "cancelled" ? <X className="size-4 text-muted" /> : <XCircle className="size-4 text-err" />}
                    <button className="min-w-0 flex-1 text-left" onClick={() => navigate("song", j.songId)}>
                      <div className="truncate text-sm">{j.songTitle}</div>
                      {j.error && <div className="truncate text-xs text-err">{j.error}</div>}
                    </button>
                    {j.device && <Badge>{j.device}</Badge>}
                    {j.processingSeconds != null && <span className="tnum w-14 text-right text-xs text-muted">{Math.round(j.processingSeconds)} s</span>}
                    <span className="w-20 text-right text-xs text-faint">{relTime(j.finishedAt ?? j.createdAt)}</span>
                    {j.status !== "completed" && (
                      <Button size="sm" variant="ghost" icon={<RotateCcw className="size-3.5" />} className="opacity-0 group-hover:opacity-100" onClick={() => prepareSongs([j.songId])}>Retry</Button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </Card>
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-4">
          <Card title="Engine">
            <div className="space-y-3 text-sm">
              <InfoRow label="Profile" value={settings?.separationProfile === "fast" ? "Fast · Demucs only" : "Worship · Inst HQ 3 + Demucs"} />
              <InfoRow label="Instruments" value={settings?.separationEngine === "auto" ? "Auto (MLX → PyTorch MPS → CPU)" : settings?.separationEngine ?? "—"} />
              <InfoRow label="Model" value={settings?.separationModel ?? "—"} />
              <InfoRow label="Installed models" value={installed.length ? installed.map((m) => m.name).join(", ") : "None"} warn={!installed.length} />
              {avg && <InfoRow label="Average time / song" value={`${Math.round(avg)} s`} />}
            </div>
            {!installed.length && (
              <div className="mt-4 rounded-xl border border-warn/30 bg-warn/[0.07] p-3 text-xs text-fg-2">
                Without a model the app still analyzes BPM, key and sections, but can’t remove drums or bass.
                <Button size="sm" variant="primary" className="mt-2.5" onClick={() => navigate("diagnostics")}>Download a model</Button>
              </div>
            )}
          </Card>
          <Card title={<span className="flex items-center gap-2"><Zap className="size-4 text-accent" />Tips</span>}>
            <ul className="space-y-2 text-xs text-muted">
              <li>Prepare songs during the week — never needed on Sunday morning.</li>
              <li>Changing bass level or drums later only re-renders the backing (seconds), no new separation.</li>
              <li>“Analyze only” is fast and enough when you play the original track.</li>
              <li>Live playback never waits for preparation; jobs pause nothing.</li>
            </ul>
          </Card>
        </div>
      </div>
    </div>
  );
}

function RunningJob({ job }: { job: PreparationJob }) {
  const idx = STAGES.findIndex(([s]) => s === job.stage);
  return (
    <Card className="ring-1 ring-accent/25">
      <div className="flex items-start gap-4">
        <div className="grid size-11 place-items-center rounded-xl bg-accent/12 text-accent"><Loader2 className="size-5 animate-spin" /></div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-base font-semibold">{job.songTitle}</span>
            {job.device && <Badge tone="accent">{job.device}</Badge>}
          </div>
          <div className="mt-0.5 truncate text-sm text-muted">{job.message || "Starting…"}</div>
        </div>
        <span className="tnum text-2xl font-semibold">{Math.round(job.progress * 100)}%</span>
        <Button size="sm" variant="outline" onClick={() => ipc.cancelJob(job.id)}>Cancel</Button>
      </div>
      <Progress value={job.progress} className="mt-4 h-2" />
      <div className="mt-3 flex gap-1">
        {STAGES.map(([id, label], i) => (
          <div key={id} className="min-w-0 flex-1">
            <div className={`h-1 rounded-full ${i < idx ? "bg-ok" : i === idx ? "bg-accent" : "bg-line"}`} />
            <div className={`mt-1 truncate text-[0.68rem] ${i === idx ? "text-fg-2" : "text-faint"}`}>{label}</div>
          </div>
        ))}
      </div>
    </Card>
  );
}

function InfoRow({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className="flex justify-between gap-4">
      <span className="text-muted">{label}</span>
      <span className={`text-right ${warn ? "text-warn" : "text-fg-2"}`}>{value}</span>
    </div>
  );
}
