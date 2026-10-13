import { listen } from "@tauri-apps/api/event";
import { Loader2, Pause, Play, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Logo } from "@/components/Sidebar";
import { Button, Progress } from "@/components/ui";
import { fmtBytes } from "@/lib/format";
import { errMsg, ipc, isTauri } from "@/lib/ipc";
import type { SetupStatus } from "@/lib/types";

export function SetupWizard({ onDone }: { onDone: () => void }) {
  const [st, setSt] = useState<SetupStatus | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = useCallback(() => ipc.setupStatus().then(setSt).catch((e) => setErr(errMsg(e))), []);

  useEffect(() => {
    refresh();
    if (!isTauri()) return;
    const off = listen<SetupStatus>("setup:progress", (e) => {
      setSt(e.payload);
      if (e.payload.complete) onDone();
    });
    return () => {
      off.then((f) => f());
    };
  }, [onDone, refresh]);

  useEffect(() => {
    if (st?.complete) onDone();
  }, [st?.complete, onDone]);

  useEffect(() => {
    if (isTauri()) ipc.runSetup().catch((e) => setErr(errMsg(e)));
  }, []);

  const totalBytes = st?.steps.reduce((a, s) => a + s.sizeBytes, 0) ?? 0;

  return (
    <div className="app-bg drag grid min-h-full place-items-center px-6 py-12">
      <div className="no-drag w-full max-w-lg">
        <div className="flex flex-col items-center text-center">
          <Logo size={56} />
          <h1 className="mt-6 text-2xl font-semibold tracking-tight">Persiapan Worship Playback Engine</h1>
          <p className="mt-2 text-sm text-muted leading-relaxed">
            Sekali saja: mesin Python untuk pemisahan stem dan model Demucs sesuai profil Mac Anda.
            Setelah selesai, semua berjalan lokal tanpa internet.
          </p>
        </div>

        <div className="mt-8 rounded-2xl border border-line bg-bg-2/50 p-5 shadow-xl">
          <div className="flex items-center justify-between text-sm">
            <span className="font-medium text-fg-2">{st?.message ?? "Memeriksa…"}</span>
            <span className="tnum text-muted">{st ? `${Math.round(st.progress * 100)}%` : "…"}</span>
          </div>
          <Progress value={st?.progress ?? 0} className="mt-3" indeterminate={!st} />
          {totalBytes > 0 && (
            <div className="mt-2 text-xs text-muted">Perkiraan unduhan: {fmtBytes(totalBytes)}</div>
          )}
          {st?.etaSecs != null && st.etaSecs > 0 && (
            <div className="mt-1 text-xs text-muted">Sisa waktu ± {Math.ceil(st.etaSecs / 60)} menit</div>
          )}

          <ul className="mt-5 space-y-2">
            {(st?.steps ?? []).map((s) => (
              <li key={s.id} className="flex items-start gap-3 rounded-xl bg-white/[0.03] px-3 py-2.5 text-left">
                <span className={`mt-0.5 size-2 shrink-0 rounded-full ${s.ready ? "bg-ok" : "bg-warn animate-pulse"}`} />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{s.title}</div>
                  <div className="text-xs text-muted line-clamp-2">{s.detail}</div>
                </div>
                <span className="tnum text-xs text-faint">{fmtBytes(s.sizeBytes)}</span>
              </li>
            ))}
          </ul>

          {err && <p className="mt-3 text-xs text-err">{err}</p>}

          <div className="mt-5 flex flex-wrap gap-2">
            {st?.paused ? (
              <Button size="sm" variant="primary" icon={<Play className="size-3.5" />} onClick={() => ipc.setupResume().then(refresh).catch((e) => setErr(errMsg(e)))}>
                Lanjutkan
              </Button>
            ) : (
              <Button size="sm" variant="outline" icon={<Pause className="size-3.5" />} onClick={() => ipc.setupPause()} disabled={!st || st.complete}>
                Jeda
              </Button>
            )}
            <Button size="sm" variant="ghost" icon={<Sparkles className="size-3.5" />} onClick={() => ipc.runSetup().catch((e) => setErr(errMsg(e)))}>
              Coba lagi
            </Button>
          </div>
        </div>

        {!st && (
          <div className="mt-6 flex justify-center text-sm text-muted">
            <Loader2 className="size-4 animate-spin" />
          </div>
        )}
      </div>
    </div>
  );
}
