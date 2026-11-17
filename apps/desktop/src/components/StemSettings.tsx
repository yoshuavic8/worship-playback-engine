import { Drum, Guitar, Mic } from "lucide-react";
import { cn } from "@/lib/format";
import type { PrepSettings, Preset, StemMode } from "@/lib/types";
import { Metronome } from "./icons";
import { Segmented, Switch } from "./ui";

export const BASS_STEPS = [-12, -9, -6, -3, 0];

export function samePrep(a: PrepSettings | null | undefined, b: PrepSettings | null | undefined) {
  if (!a || !b) return false;
  return a.vocals === b.vocals && a.drums === b.drums && a.bass === b.bass && a.bassDb === b.bassDb && a.otherDb === b.otherDb;
}

/** Which preset (if any) these settings match exactly. */
export function matchPreset(p: PrepSettings, presets: Preset[]): Preset | undefined {
  return presets.find((x) => samePrep(x.settings, p) && x.settings.click === p.click && x.settings.cue === p.cue);
}

export function PresetPicker({ value, presets, onPick }: { value: PrepSettings; presets: Preset[]; onPick: (p: Preset) => void }) {
  const current = matchPreset(value, presets);
  return (
    <div className="grid grid-cols-2 gap-2">
      {presets.map((p) => {
        const active = current?.id === p.id;
        return (
          <button
            key={p.id}
            onClick={() => onPick(p)}
            className={cn(
              "rounded-xl border px-3 py-2.5 text-left transition-colors",
              active ? "border-accent/60 bg-accent/[0.08]" : "border-line bg-bg-2/60 hover:border-line-2",
            )}
          >
            <div className={cn("text-sm font-semibold", active && "text-accent")}>{p.name}</div>
            <div className="mt-0.5 line-clamp-2 text-xs text-muted">{p.description}</div>
          </button>
        );
      })}
    </div>
  );
}

export function StemControls({ value, onChange, compact }: { value: PrepSettings; onChange: (p: PrepSettings) => void; compact?: boolean }) {
  const set = (patch: Partial<PrepSettings>) => onChange({ ...value, ...patch, presetId: null });
  return (
    <div className={cn("space-y-3", compact && "space-y-2")}>
      <StemRow icon={<Drum className="size-4" />} label="Drums" hint={value.drums === "optional" ? "Muted, can be switched on live" : value.drums === "keep" ? "In the backing track" : "Live drummer plays"}>
        <Segmented<StemMode> size="sm" value={value.drums} onChange={(drums) => set({ drums })}
          options={[{ value: "remove", label: "Remove" }, { value: "keep", label: "Keep" }, { value: "optional", label: "Optional" }]} />
      </StemRow>
      <StemRow icon={<Guitar className="size-4" />} label="Bass" hint={value.bass === "remove" ? "Live bassist plays" : `Backing bass at ${value.bassDb} dB`}>
        <Segmented<StemMode> size="sm" value={value.bass} onChange={(bass) => set({ bass })}
          options={[{ value: "remove", label: "Remove" }, { value: "keep", label: "Keep" }]} />
      </StemRow>
      {value.bass === "keep" && (
        <div className="ml-9 flex items-center gap-1">
          {BASS_STEPS.map((db) => (
            <button key={db} onClick={() => set({ bassDb: db })}
              className={cn("tnum h-7 flex-1 rounded-md border text-xs font-semibold", value.bassDb === db ? "border-accent/60 bg-accent/15 text-accent" : "border-line text-muted hover:text-fg-2")}>
              {db === 0 ? "0 dB" : `${db}`}
            </button>
          ))}
        </div>
      )}
      <StemRow icon={<Mic className="size-4" />} label="Lead vocals" hint={value.vocals === "keep" ? "Original vocals in FOH (unusual)" : "Removed — guide vocal can go to monitor"}>
        <Segmented<StemMode> size="sm" value={value.vocals} onChange={(vocals) => set({ vocals })}
          options={[{ value: "remove", label: "Remove" }, { value: "keep", label: "Keep" }]} />
      </StemRow>
      <div className="flex gap-4 pt-1 pl-9">
        <label className="flex items-center gap-2 text-sm text-fg-2"><Switch size="sm" tone="mon" checked={value.click} onChange={(click) => onChange({ ...value, click, presetId: null })} /><Metronome className="size-3.5 text-mon" />Click</label>
        <label className="flex items-center gap-2 text-sm text-fg-2"><Switch size="sm" tone="mon" checked={value.cue} onChange={(cue) => onChange({ ...value, cue, presetId: null })} /><Mic className="size-3.5 text-mon" />Voice cues</label>
      </div>
    </div>
  );
}

function StemRow({ icon, label, hint, children }: { icon: React.ReactNode; label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <span className="grid size-7 shrink-0 place-items-center rounded-lg bg-white/[0.05] text-muted">{icon}</span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{label}</div>
        <div className="truncate text-xs text-muted">{hint}</div>
      </div>
      {children}
    </div>
  );
}
