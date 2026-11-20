import type { CSSProperties, ReactNode } from "react";
import { cn } from "@/lib/format";
import { mappingFor, sourceShort, targetKey, targetLabel, useMidi } from "@/lib/midi";
import type { MidiTarget } from "@/lib/types";
import { useApp } from "@/store/app";

/**
 * Wraps a control that can be mapped to MIDI or a key. In Learn mode the control is covered
 * by a highlight: click it, then press a pad / turn a knob / press a key.
 */
export function Mappable({ target, children, className, perSong, style }: {
  target: MidiTarget; children: ReactNode; className?: string; perSong?: boolean; style?: CSSProperties;
}) {
  const learn = useMidi((s) => s.learn);
  const armed = useMidi((s) => s.armed);
  const mappings = useMidi((s) => s.mappings);
  const songId = useApp((s) => s.loaded?.songId ?? null);
  if (!learn) return <>{children}</>;
  const mapped = mappingFor(mappings, target, songId);
  const isArmed = armed != null && targetKey(armed.target) === targetKey(target);
  return (
    <div className={cn("relative", className)} style={style}>
      {children}
      <button
        type="button"
        aria-label={`Learn: ${targetLabel(target)}`}
        title={mapped ? `${targetLabel(target)} — mapped` : `${targetLabel(target)} — click, then press a control`}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          const st = useMidi.getState();
          if (isArmed) st.disarm();
          else st.arm(target, perSong ? songId : null);
        }}
        className={cn(
          "absolute inset-0 z-10 rounded-[inherit] border-2 transition-colors",
          isArmed
            ? "animate-pulse-soft border-accent bg-accent/25"
            : mapped
              ? "border-mon/70 bg-mon/10 hover:bg-mon/20"
              : "border-dashed border-accent/60 bg-accent/[0.06] hover:bg-accent/15",
        )}
      >
        {mapped && (
          <span className={cn(
            "absolute top-1 right-1 max-w-[85%] truncate rounded px-1 text-[0.6rem] font-black tracking-wide",
            mapped.songId ? "bg-accent text-accent-ink" : "bg-mon text-black",
          )}>
            {sourceShort(mapped.source)}
          </span>
        )}
      </button>
    </div>
  );
}
