import { AlertTriangle, CheckCircle2, CircleDashed, Loader2, XCircle } from "lucide-react";
import { useApp } from "@/store/app";
import { visibleReviewWarnings } from "@/lib/reviewWarnings";
import type { Song, SongStatus } from "@/lib/types";
import { Badge } from "./ui";

const STATUS: Record<SongStatus, { label: string; tone: "neutral" | "ok" | "warn" | "err" | "accent" | "mon" }> = {
  imported: { label: "Not prepared", tone: "neutral" },
  queued: { label: "Queued", tone: "mon" },
  processing: { label: "Preparing", tone: "accent" },
  ready: { label: "Live-ready", tone: "ok" },
  warning: { label: "Needs review", tone: "warn" },
  error: { label: "Error", tone: "err" },
};

export function SongStatusBadge({ song }: { song: Song }) {
  const job = useApp((s) =>
    song.status === "processing" ? Object.values(s.jobs).find((j) => j.songId === song.id && j.status === "running") : undefined,
  );
  const st = STATUS[song.status] ?? STATUS.imported;
  const icon = {
    imported: <CircleDashed className="size-3" />,
    queued: <CircleDashed className="size-3" />,
    processing: <Loader2 className="size-3 animate-spin" />,
    ready: <CheckCircle2 className="size-3" />,
    warning: <AlertTriangle className="size-3" />,
    error: <XCircle className="size-3" />,
  }[song.status];
  const { active, stale } = visibleReviewWarnings(song);
  const why = active.join(" · ") || stale.join(" · ");
  return (
    <span title={why || song.lastError || (song.status === "warning" ? "Open the song — something needs a listen before it is live-ready" : undefined)} className="inline-flex">
      <Badge tone={st.tone}>
        {icon}
        {st.label}
        {job && <span className="tnum opacity-80">{Math.round(job.progress * 100)}%</span>}
      </Badge>
    </span>
  );
}

export function KeyBpm({ song, className = "" }: { song: Pick<Song, "key" | "bpm" | "timeSignature">; className?: string }) {
  return (
    <span className={`tnum inline-flex items-center gap-2 text-fg-2 ${className}`}>
      <span className="min-w-[2.2ch] font-semibold text-fg">{song.key ?? "—"}</span>
      <span className="text-faint">·</span>
      <span>{song.bpm ? `${Math.round(song.bpm * 1000) / 1000}` : "—"} <span className="text-muted">BPM</span></span>
      {song.timeSignature !== "4/4" && (
        <>
          <span className="text-faint">·</span>
          <span>{song.timeSignature}</span>
        </>
      )}
    </span>
  );
}

/** Small stem availability pills: V D B O */
export function StemPills({ song }: { song: Song }) {
  const items: [string, string | null, string][] = [
    ["V", song.stems.vocals, "Vocals"],
    ["D", song.stems.drums, "Drums"],
    ["B", song.stems.bass, "Bass"],
    ["O", song.stems.other, "Other / keys / guitars"],
  ];
  return (
    <span className="inline-flex gap-0.5">
      {items.map(([l, p, t]) => (
        <span
          key={l}
          title={`${t}${p ? "" : " (not separated)"}`}
          className={`grid size-[1.15rem] place-items-center rounded text-[0.6rem] font-bold ${p ? "bg-white/10 text-fg-2" : "text-faint ring-1 ring-line ring-inset"}`}
        >
          {l}
        </span>
      ))}
    </span>
  );
}

export function presetName(id: string | null | undefined): string {
  if (!id) return "Custom";
  return useApp.getState().presets.find((p) => p.id === id)?.name ?? "Custom";
}
