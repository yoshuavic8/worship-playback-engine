import { Cable, Keyboard, Trash2, X } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { cn } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { isContinuous, isSwitch, sourceLabel, targetLabel, useMidi } from "@/lib/midi";
import type { ButtonMode, EncoderMode, MidiMapping } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Button, Confirm, Segmented, Slider } from "./ui";

/** Connected controllers + the last message received (helps when a pad "does nothing"). */
export function MidiDevices({ compact }: { compact?: boolean }) {
  const status = useMidi((s) => s.status);
  const act = useMidi((s) => s.activity);
  const devices = status?.devices ?? [];
  return (
    <div className="space-y-2">
      {status && !status.available ? (
        <div className="text-sm text-err">MIDI is unavailable: {status.error ?? "unknown error"}</div>
      ) : devices.length === 0 ? (
        <div className="flex items-center gap-2 text-sm text-muted">
          <Cable className="size-4" /> No MIDI controller connected. Plug one in; it is picked up automatically.
        </div>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {devices.map((d) => <Badge key={d} tone="ok" dot>{d}</Badge>)}
        </div>
      )}
      {!compact && (
        <div className="text-xs text-faint">
          {act ? (
            <>Last: <span className="font-mono text-fg-2">{sourceLabel({ ...act, key: act.key ?? undefined })}</span>
              {act.kind !== "key" && <> = {act.value}</>}
              {act.device && <> · {act.device}</>}
              {!act.mapped && <> · not mapped</>}</>
          ) : "Press a pad or turn a knob to see what WPE receives."}
        </div>
      )}
    </div>
  );
}

function MappingRow({ m, songTitle }: { m: MidiMapping; songTitle?: string }) {
  const [min, setMin] = useState(Math.round(m.min * 100));
  const [max, setMax] = useState(Math.round(m.max * 100));
  const save = (patch: Partial<MidiMapping>) =>
    ipc.midiSaveMapping({ ...m, ...patch }).catch((e) => toast.error("Could not save mapping", { description: String(e) }));
  const cont = isContinuous(m.target);
  const btnMode = !cont && m.source.kind !== "pc";
  return (
    <div className="rounded-lg border border-line bg-bg-2/50 px-3 py-2">
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{targetLabel(m.target)}</div>
          <div className="flex items-center gap-1.5 text-xs text-muted">
            {m.source.kind === "key" ? <Keyboard className="size-3" /> : <Cable className="size-3" />}
            <span className="font-mono">{sourceLabel(m.source)}</span>
            {m.songId ? <Badge tone="accent" className="h-4 px-1 text-[0.62rem]">{songTitle ?? "This song"}</Badge>
              : <Badge className="h-4 px-1 text-[0.62rem]">All songs</Badge>}
          </div>
        </div>
        <button
          className="rounded-md p-1.5 text-faint hover:bg-white/5 hover:text-err"
          aria-label="Delete mapping"
          onClick={() => ipc.midiDeleteMapping(m.id).catch((e) => toast.error(String(e)))}
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>
      {btnMode && (
        <div className="mt-2 flex items-center gap-2">
          <Segmented<ButtonMode>
            size="sm"
            value={m.mode}
            onChange={(mode) => save({ mode })}
            options={[
              { value: "trigger", label: "Trigger", title: "Fires on press" },
              ...(isSwitch(m.target) ? [
                { value: "toggle" as const, label: "Toggle", title: "Press on, press off" },
                { value: "momentary" as const, label: "Momentary", title: "On while held" },
              ] : []),
            ]}
          />
        </div>
      )}
      {cont && (
        <div className="mt-2 space-y-2">
          {m.source.kind === "cc" && (
            <Segmented<EncoderMode>
              size="sm"
              value={m.encoder}
              onChange={(encoder) => save({ encoder })}
              options={[
                { value: "absolute", label: "Absolute", title: "Normal knob/fader, with pick-up" },
                { value: "relative1", label: "Rel #1", title: "Arturia Relative #1 (64 ± n)" },
                { value: "relative2", label: "Rel #2", title: "Arturia Relative #2 (two's complement)" },
                { value: "relative3", label: "Rel #3", title: "Arturia Relative #3 (16 ± n)" },
              ]}
            />
          )}
          <div className="grid grid-cols-[2.2rem_1fr_2.4rem] items-center gap-2 text-xs text-muted">
            <span>Min</span>
            <Slider value={min} min={0} max={100} onChange={setMin} onCommit={(v) => save({ min: v / 100, max: Math.max(max, v + 1) / 100 })} label="Minimum" />
            <span className="text-right tabular-nums">{min}%</span>
            <span>Max</span>
            <Slider value={max} min={0} max={100} onChange={setMax} onCommit={(v) => save({ max: v / 100, min: Math.min(min, v - 1) / 100 })} label="Maximum" />
            <span className="text-right tabular-nums">{max}%</span>
          </div>
        </div>
      )}
    </div>
  );
}

/** All mappings, global first, then this song's. */
export function MappingList({ songId, emptyHint }: { songId?: string | null; emptyHint?: string }) {
  const mappings = useMidi((s) => s.mappings);
  const songs = useApp((s) => s.songs);
  const titles = useMemo(() => new Map(songs.map((s) => [s.id, s.title])), [songs]);
  const shown = mappings.filter((m) => songId === undefined || m.songId == null || m.songId === songId);
  if (shown.length === 0) {
    return <div className="rounded-lg border border-dashed border-line px-3 py-6 text-center text-sm text-muted">{emptyHint ?? "No mappings yet."}</div>;
  }
  const sorted = [...shown].sort((a, b) => Number(a.songId != null) - Number(b.songId != null));
  return (
    <div className="space-y-1.5">
      {sorted.map((m) => <MappingRow key={`${m.id}:${m.mode}:${m.encoder}:${m.min}:${m.max}`} m={m} songTitle={m.songId ? titles.get(m.songId) : undefined} />)}
    </div>
  );
}

export function PresetActions() {
  const [ask, setAsk] = useState<null | "preset" | "clear">(null);
  return (
    <div className="flex flex-wrap gap-2">
      <Button size="sm" onClick={() => setAsk("preset")}>Load Arturia MiniLab MK2 preset</Button>
      <Button size="sm" variant="ghost" onClick={() => setAsk("clear")}>Clear global mappings</Button>
      <Confirm
        open={ask === "preset"}
        onOpenChange={(v) => !v && setAsk(null)}
        title="Load the MiniLab MK2 preset?"
        confirmLabel="Load preset"
        message={<>Pads 1–8 (bank 1) jump to sections 1–8. Pads 9–16 (bank 2) are play/pause, next song, previous song, vamp, click on/off, guide-vocal mute, transpose −1 and +1. The knobs set FOH master, click, cue and track volumes; the sustain pedal holds a vamp. Global mappings on the same pads/knobs are replaced; your per-song section mappings stay.<br /><br />Set the MiniLab knobs to <b>Relative #1</b> in Arturia MIDI Control Center for smooth knob control.</>}
        onConfirm={() => ipc.midiApplyPreset("minilab-mk2").then((n) => toast.success(`MiniLab MK2 preset loaded (${n} controls)`)).catch((e) => toast.error(String(e)))}
      />
      <Confirm
        open={ask === "clear"}
        onOpenChange={(v) => !v && setAsk(null)}
        title="Clear all global mappings?"
        danger
        confirmLabel="Clear"
        message="Transport, volume, mute/solo and section-slot mappings are removed. Per-song section mappings stay."
        onConfirm={() => ipc.midiClear("global").then((n) => toast.success(`Removed ${n} mappings`)).catch((e) => toast.error(String(e)))}
      />
    </div>
  );
}

/** Side drawer shown on the Live page while Learn is on. */
export function LearnDrawer() {
  const armed = useMidi((s) => s.armed);
  const scope = useMidi((s) => s.sectionScope);
  const loaded = useApp((s) => s.loaded);
  return (
    <aside className="flex w-[22rem] shrink-0 flex-col border-l border-line bg-panel/80">
      <div className="flex items-center justify-between border-b border-line px-4 py-3">
        <div>
          <div className="text-sm font-semibold">MIDI Learn</div>
          <div className="text-xs text-muted">Click a control, then press a pad, turn a knob or press a key.</div>
        </div>
        <button className="rounded-md p-1 text-muted hover:bg-white/5 hover:text-fg" aria-label="Close Learn" onClick={() => useMidi.getState().setLearn(false)}>
          <X className="size-4" />
        </button>
      </div>
      <div className="space-y-4 overflow-y-auto px-4 py-3">
        <div className={cn("rounded-xl border px-3 py-2.5 text-sm", armed ? "border-accent/50 bg-accent/10" : "border-line bg-bg-2/60 text-muted")}>
          {armed ? (
            <div className="flex items-center justify-between gap-2">
              <span>Waiting for <b className="text-fg">{targetLabel(armed.target)}</b>…</span>
              <Button size="sm" variant="ghost" onClick={() => useMidi.getState().disarm()}>Cancel</Button>
            </div>
          ) : "Nothing selected. Highlighted controls can be mapped."}
        </div>
        <div>
          <div className="mb-1.5 text-xs font-medium uppercase tracking-wider text-faint">Section buttons map to</div>
          <Segmented<"song" | "all">
            size="sm"
            value={scope}
            onChange={(v) => useMidi.getState().setScope(v)}
            options={[
              { value: "song", label: "This song only", title: "The pad jumps to this exact section in this song" },
              { value: "all", label: "Slot (every song)", title: "The pad jumps to section N of whatever song is loaded" },
            ]}
          />
        </div>
        <MidiDevices />
        <div>
          <div className="mb-1.5 text-xs font-medium uppercase tracking-wider text-faint">Mappings</div>
          <MappingList songId={loaded?.songId ?? null} emptyHint="Click a highlighted control to start." />
        </div>
        <PresetActions />
      </div>
    </aside>
  );
}
