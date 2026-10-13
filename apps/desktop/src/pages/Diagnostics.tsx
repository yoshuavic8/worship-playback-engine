import { listen } from "@tauri-apps/api/event";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle, CheckCircle2, CircleDashed, Copy, Download, ExternalLink, FolderOpen, HardDrive, Loader2, Package, RefreshCw, ShieldCheck, Trash2, XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Badge, Button, Card, Confirm, PageHeader, Progress, Segmented, StatusDot } from "@/components/ui";
import { cn, fmtBytes } from "@/lib/format";
import { errMsg, ipc, isTauri } from "@/lib/ipc";
import type { LogEntry, Readiness, ResourceProgress, ResourceStatus } from "@/lib/types";
import { verdict } from "./Dashboard";

export function Diagnostics() {
  const [ready, setReady] = useState<Readiness | null>(null);
  const [loading, setLoading] = useState(false);
  const [resources, setResources] = useState<ResourceStatus[]>([]);
  const [envProgress, setEnvProgress] = useState<ResourceProgress | null>(null);
  const [remove, setRemove] = useState<ResourceStatus | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [r, res] = await Promise.all([ipc.readiness(), ipc.listResources()]);
      setReady(r);
      setResources(res);
      const liveOk = r.playback === "READY";
      const prepOk = r.preparation === "READY";
      toast.success(liveOk && prepOk ? "All checks passed" : "Re-check complete", {
        description: `Live playback: ${r.playback} · Preparation: ${r.preparation}`,
      });
    } catch (e) {
      toast.error(errMsg(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    if (!isTauri()) return;
    const offs: Promise<() => void>[] = [
      listen<{ id: string; progress: ResourceProgress }>("resource:progress", (e) => {
        if (e.payload.id === "python-env") setEnvProgress(e.payload.progress);
        else setResources((rs) => rs.map((r) => (r.id === e.payload.id ? { ...r, progress: e.payload.progress } : r)));
      }),
      listen<string>("resource:changed", (e) => {
        refresh();
        if (e.payload === "python-env") toast.success("Python environment ready");
      }),
    ];
    return () => offs.forEach((p) => p.then((f) => f()));
  }, [refresh]);

  const checks = ready?.checks ?? [];
  const envCheck = checks.find((c) => c.id === "python");
  const envMissing = envCheck && envCheck.status !== "ok";

  return (
    <div className="pb-10">
      <PageHeader title="Diagnostics" subtitle="Is this Mac ready for Sunday?"
        actions={<Button variant="ghost" icon={<RefreshCw className={cn("size-4", loading && "animate-spin")} />} onClick={refresh}>Re-check</Button>} />
      <div className="grid grid-cols-12 gap-5 px-8 pt-6">
        <div className="col-span-12 grid grid-cols-3 gap-5">
          <Verdict label="Live playback" value={ready?.playback} desc="Needs only an audio device" />
          <Verdict label="Preparation" value={ready?.preparation} desc="FFmpeg + Python + model" />
          <Verdict label="AI assistant" value={ready?.ai} desc="Local Ollama, optional" />
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-7">
          <Card title="System checks">
            {!ready ? <div className="flex items-center gap-2 text-sm text-muted"><Loader2 className="size-4 animate-spin" />Checking…</div> : (
              <div className="divide-y divide-line">
                {checks.map((c) => (
                  <div key={c.id} className="flex items-start gap-3 py-2.5">
                    <CheckIcon status={c.status} />
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium">{c.label}</div>
                      <div className="truncate text-xs text-muted" title={c.detail}>{c.detail}</div>
                      {c.hint && c.status !== "ok" && <div className="mt-1 text-xs text-fg-2">→ {c.hint}</div>}
                    </div>
                    {c.hint?.startsWith("brew ") && c.status !== "ok" && (
                      <Button size="sm" variant="ghost" icon={<Copy className="size-3.5" />} onClick={() => { navigator.clipboard.writeText(c.hint!); toast.success("Copied — paste it in Terminal"); }}>Copy</Button>
                    )}
                    {c.id === "python" && c.status !== "ok" && (
                      <Button size="sm" variant="primary" loading={envProgress?.active} onClick={() => ipc.installPythonEnv().catch((e) => toast.error(errMsg(e)))}>Install</Button>
                    )}
                  </div>
                ))}
              </div>
            )}
            {(envProgress?.active || envProgress?.error) && (
              <div className="mt-3 rounded-xl bg-bg-2/60 p-3">
                <div className="flex items-center gap-2 text-sm font-medium">{envProgress.active && <Loader2 className="size-4 animate-spin" />}Python environment</div>
                <div className={cn("mt-1 truncate font-mono text-xs", envProgress.error ? "text-err" : "text-muted")}>{envProgress.error ?? envProgress.message}</div>
                {envProgress.active && <Progress value={0.5} indeterminate className="mt-2" />}
              </div>
            )}
            {envMissing && !envProgress?.active && (
              <p className="mt-3 text-xs text-muted">
                The Python environment (PyTorch, Demucs, librosa — about 1 GB) is installed once into the app’s own folder using uv. Nothing is installed system-wide.
              </p>
            )}
          </Card>

          <Card title={<span className="flex items-center gap-2"><Package className="size-4 text-muted" />AI models</span>}
            subtitle="Downloaded only when you click — sources, licenses and checksums shown">
            <div className="space-y-3">
              {resources.map((r) => <ModelRow key={r.id} r={r} onRemove={() => setRemove(r)} onChange={refresh} />)}
            </div>
          </Card>
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-5">
          <Card title={<span className="flex items-center gap-2"><HardDrive className="size-4 text-muted" />Storage</span>}>
            {ready && (
              <div className="space-y-2 text-sm">
                {Object.entries(ready.diskUsage).map(([k, v]) => (
                  <div key={k} className="flex justify-between"><span className="text-muted">{{ songs: "Songs & stems", models: "Models", pythonEnv: "Python environment", cues: "Voice cues" }[k] ?? k}</span><span className="tnum text-fg-2">{fmtBytes(v)}</span></div>
                ))}
                <div className="mt-3 border-t border-line pt-3 text-xs text-muted">
                  <div className="truncate" title={ready.dataDir}>Data: {ready.dataDir}</div>
                  <div className="truncate" title={ready.libraryDir}>Library: {ready.libraryDir}</div>
                  <div>Database schema v{ready.schemaVersion} · App {ready.appVersion}</div>
                </div>
                <Button size="sm" variant="outline" className="mt-2" icon={<FolderOpen className="size-3.5" />} onClick={() => revealItemInDir(`${ready.dataDir}/database.sqlite`).catch(() => {})}>Show in Finder</Button>
              </div>
            )}
          </Card>
          <Logs />
        </div>
      </div>

      <Confirm open={!!remove} onOpenChange={(v) => !v && setRemove(null)} title="Remove model?" danger confirmLabel="Remove"
        message={<>Deletes {remove?.name} ({fmtBytes(remove?.sizeBytes)}). Prepared songs keep working; new preparations need a model.</>}
        onConfirm={() => remove && ipc.removeResource(remove.id).then(refresh).catch((e) => toast.error(errMsg(e)))} />
    </div>
  );
}

function ModelRow({ r, onRemove, onChange }: { r: ResourceStatus; onRemove: () => void; onChange: () => void }) {
  const p = r.progress;
  const active = !!p?.active;
  return (
    <div className={cn("rounded-xl border p-3.5", r.installed ? "border-ok/25 bg-ok/[0.04]" : "border-line bg-bg-2/50")}>
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-semibold">{r.name}</span>
            {r.recommended && <Badge tone="accent">Recommended</Badge>}
            {r.installed && <Badge tone="ok"><ShieldCheck className="size-3" />Verified</Badge>}
          </div>
          <div className="mt-0.5 text-xs text-muted">{r.description}</div>
          <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 text-[0.7rem] text-faint">
            <span>{fmtBytes(r.sizeBytes)}</span>
            <span>License: <span className="text-fg-2">{r.license}</span></span>
            <span>Engine: {r.engine}</span>
            <button className="inline-flex items-center gap-1 hover:text-fg-2" onClick={() => openUrl(r.source).catch(() => {})}>Source <ExternalLink className="size-3" /></button>
          </div>
        </div>
        {r.installed ? (
          <Button size="sm" variant="ghost" icon={<Trash2 className="size-3.5" />} onClick={onRemove}>Remove</Button>
        ) : active ? (
          <Button size="sm" variant="outline" onClick={() => ipc.cancelDownload(r.id)}>Cancel</Button>
        ) : (
          <Button size="sm" variant={r.recommended ? "primary" : "outline"} icon={<Download className="size-3.5" />}
            onClick={() => ipc.downloadResource(r.id).then(onChange).catch((e) => toast.error(errMsg(e)))}>Download</Button>
        )}
      </div>
      {(active || p?.error) && (
        <div className="mt-3">
          <div className="mb-1 flex justify-between text-xs">
            <span className={p?.error ? "text-err" : "text-muted"}>{p?.error ?? p?.message}</span>
            {p && p.total > 0 && <span className="tnum text-muted">{fmtBytes(p.downloaded)} / {fmtBytes(p.total)}</span>}
          </div>
          {active && <Progress value={p && p.total ? p.downloaded / p.total : 0} />}
        </div>
      )}
    </div>
  );
}

function Logs() {
  const [cat, setCat] = useState("all");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const load = useCallback(() => ipc.logs(300, cat === "all" ? null : cat).then((l) => setLogs(l.reverse())).catch(() => {}), [cat]);
  useEffect(() => {
    load();
    const t = setInterval(load, 3000);
    return () => clearInterval(t);
  }, [load]);
  const text = useMemo(() => logs.map((l) => `${l.time} [${l.level}] ${l.category}: ${l.message}`).join("\n"), [logs]);
  return (
    <Card title="Logs" actions={<Button size="sm" variant="ghost" icon={<Copy className="size-3.5" />} onClick={() => { navigator.clipboard.writeText(text); toast.success("Logs copied"); }}>Copy</Button>}>
      <Segmented<string> size="sm" value={cat} onChange={setCat} className="mb-3 flex-wrap"
        options={["all", "audio", "playback", "preparation", "routing", "ai", "error"].map((c) => ({ value: c, label: c === "all" ? "All" : c[0].toUpperCase() + c.slice(1) }))} />
      <div className="h-80 overflow-y-auto rounded-lg bg-[#0a0b0d] p-2 font-mono text-[0.7rem] leading-relaxed select-text">
        {logs.length === 0 && <div className="p-2 text-faint">No log entries.</div>}
        {logs.map((l, i) => (
          <div key={i} className="flex gap-2">
            <span className="shrink-0 text-faint">{l.time.slice(11, 19)}</span>
            <span className={cn("w-10 shrink-0", l.level === "error" ? "text-err" : l.level === "warn" ? "text-warn" : "text-muted")}>{l.level}</span>
            <span className="w-20 shrink-0 truncate text-mon/80">{l.category}</span>
            <span className="min-w-0 break-words text-fg-2">{l.message}</span>
          </div>
        ))}
      </div>
    </Card>
  );
}

function Verdict({ label, value, desc }: { label: string; value?: string; desc: string }) {
  const [tone, text] = verdict(value ?? "…");
  return (
    <div className={cn("card px-5 py-4", tone === "ok" && "border-ok/25", tone === "err" && "border-err/30", tone === "warn" && "border-warn/30")}>
      <div className="label">{label}</div>
      <div className="mt-1.5 flex items-center gap-2 text-lg font-semibold"><StatusDot tone={tone} />{text}</div>
      <div className="mt-0.5 text-xs text-muted">{desc}</div>
    </div>
  );
}

function CheckIcon({ status }: { status: string }) {
  if (status === "ok") return <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-ok" />;
  if (status === "warning") return <AlertTriangle className="mt-0.5 size-4 shrink-0 text-warn" />;
  if (status === "optional") return <CircleDashed className="mt-0.5 size-4 shrink-0 text-muted" />;
  return <XCircle className="mt-0.5 size-4 shrink-0 text-err" />;
}