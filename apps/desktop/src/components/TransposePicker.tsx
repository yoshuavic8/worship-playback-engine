import { toast } from "sonner";
import { cn } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { TRANSPOSE_RANGE, jobKey, semitoneLabel, transposedKey, useTranspose } from "@/lib/transpose";
import type { Song } from "@/lib/types";
import { useApp } from "@/store/app";
import { Progress } from "./ui";

/** Choose the playback key (−4…+4). Keys are rendered offline; a dot marks keys that are ready. */
export function TransposePicker({ song, disabled, disabledHint, size = "md" }: {
  song: Song; disabled?: boolean; disabledHint?: string; size?: "sm" | "md";
}) {
  const jobs = useTranspose((s) => s.jobs);
  const current = song.transpose ?? 0;
  const cached = song.transposed ?? [];
  const hasStems = !!song.stems.other && !!song.stems.bass;
  const job = current !== 0 ? jobs[jobKey(song.id, current)] : undefined;
  const busy = job && (job.state === "queued" || job.state === "running");
  const off = disabled || !hasStems;

  const choose = async (s: number) => {
    if (s === current) return;
    try {
      useApp.getState().upsertSong(await ipc.songSetTranspose(song.id, s));
    } catch (e) {
      toast.error("Cannot change the key", { description: errMsg(e) });
    }
  };

  return (
    <div className="space-y-1.5">
      <div className={cn("inline-flex rounded-lg border border-line bg-bg-2 p-0.5", off && "opacity-40")} role="radiogroup" aria-label="Key">
        {TRANSPOSE_RANGE.map((s) => {
          const on = s === current;
          const ready = s === 0 || cached.includes(s);
          return (
            <button
              key={s}
              role="radio"
              aria-checked={on}
              disabled={off}
              title={`${transposedKey(song.key, s)} (${semitoneLabel(s)} semitone${Math.abs(s) === 1 ? "" : "s"})${ready ? "" : " · not rendered yet"}`}
              onClick={() => choose(s)}
              className={cn(
                "relative flex flex-col items-center justify-center rounded-md font-semibold transition-colors",
                size === "md" ? "h-11 w-11" : "h-9 w-9",
                on ? "bg-raised text-fg shadow-sm" : "text-muted hover:text-fg-2",
                s === 0 && !on && "text-fg-2",
              )}
            >
              <span className={size === "md" ? "text-[0.95rem]" : "text-[0.8rem]"}>{transposedKey(song.key, s)}</span>
              <span className="tnum text-[0.6rem] font-medium text-faint">{s === 0 ? "orig" : semitoneLabel(s)}</span>
              {s !== 0 && ready && <span className="absolute top-1 right-1 size-1.5 rounded-full bg-ok" />}
            </button>
          );
        })}
      </div>
      {!hasStems ? (
        <div className="text-xs text-muted">Prepare the song with stem separation to change its key.</div>
      ) : disabled && disabledHint ? (
        <div className="text-xs text-muted">{disabledHint}</div>
      ) : busy ? (
        <div className="flex items-center gap-2 text-xs text-muted">
          <Progress value={job!.progress} className="w-32" indeterminate={job!.state === "queued"} />
          <span>{job!.state === "queued" ? "Waiting…" : `Rendering ${transposedKey(song.key, current)}…`}</span>
        </div>
      ) : job?.state === "failed" ? (
        <div className="text-xs text-err">
          {job.message}{" "}
          <button className="underline" onClick={() => ipc.transposePrepare([song.id]).catch((e) => toast.error(errMsg(e)))}>Try again</button>
        </div>
      ) : current !== 0 && !cached.includes(current) ? (
        <div className="text-xs text-warn">Not rendered yet. The original key plays until it is ready.</div>
      ) : null}
    </div>
  );
}
