import {
  Activity, AudioLines, Cpu, LayoutDashboard, ListMusic, Music2, Radio, Settings2, type LucideIcon,
} from "lucide-react";
import { useApp, type Page } from "@/store/app";
import { cn } from "@/lib/format";
import { StatusDot, Tip } from "./ui";

const NAV: { page: Page; label: string; icon: LucideIcon; match?: Page[] }[] = [
  { page: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { page: "songs", label: "Songs", icon: Music2, match: ["song"] },
  { page: "setlists", label: "Setlists", icon: ListMusic, match: ["setlist"] },
  { page: "live", label: "Live", icon: Radio },
  { page: "prep", label: "Preparation", icon: Cpu },
  { page: "devices", label: "Devices", icon: AudioLines },
  { page: "settings", label: "Settings", icon: Settings2 },
  { page: "diagnostics", label: "Diagnostics", icon: Activity },
];

export function Sidebar() {
  const route = useApp((s) => s.route);
  const navigate = useApp((s) => s.navigate);
  const queue = useApp((s) => s.queue.length);
  const running = useApp((s) => Object.values(s.jobs).some((j) => j.status === "running"));
  const audio = useApp((s) => s.audioStatus);
  const live = useApp((s) => s.live);
  const songsCount = useApp((s) => s.songs.length);

  const deviceTone = audio?.running ? (audio.issues.some((i) => i.level === "error") ? "err" : "ok") : "err";

  return (
    <aside className="drag flex w-[212px] shrink-0 flex-col border-r border-line bg-bg-2/80">
      <div className="h-11 shrink-0" />
      <div className="no-drag flex items-center gap-2.5 px-4 pb-5">
        <Logo />
        <div className="leading-tight">
          <div className="text-[0.92rem] font-semibold tracking-tight">Worship Playback</div>
          <div className="text-[0.68rem] font-medium tracking-[0.14em] text-muted uppercase">Engine</div>
        </div>
      </div>
      <nav className="no-drag flex flex-col gap-0.5 px-2.5">
        {NAV.map((n) => {
          const active = route.page === n.page || n.match?.includes(route.page);
          const Icon = n.icon;
          return (
            <button
              key={n.page}
              onClick={() => navigate(n.page)}
              className={cn(
                "group relative flex h-9 items-center gap-3 rounded-lg px-3 text-[0.9rem] font-medium transition-colors",
                active ? "bg-white/[0.07] text-fg" : "text-fg-2/80 hover:bg-white/[0.04] hover:text-fg",
              )}
            >
              {active && <span className="absolute top-2 bottom-2 left-0 w-[3px] rounded-r bg-accent" />}
              <Icon className={cn("size-[1.05rem]", active ? "text-accent" : "text-muted group-hover:text-fg-2")} strokeWidth={2} />
              <span className="flex-1 text-left">{n.label}</span>
              {n.page === "live" && live && <span className="rounded bg-live px-1.5 text-[0.62rem] font-bold tracking-wider text-white">ON</span>}
              {n.page === "prep" && (queue > 0 || running) && (
                <span className="tnum grid h-[1.15rem] min-w-[1.15rem] place-items-center rounded-full bg-accent px-1 text-[0.65rem] font-bold text-accent-ink">
                  {queue || "•"}
                </span>
              )}
              {n.page === "songs" && songsCount > 0 && <span className="tnum text-xs text-faint">{songsCount}</span>}
            </button>
          );
        })}
      </nav>
      <div className="flex-1" />
      <div className="no-drag space-y-1 border-t border-line px-3 py-3">
        <Tip label={audio?.running ? `${audio.running.channels} outputs · ${audio.running.sampleRate / 1000} kHz` : audio?.error ?? "No audio device"} side="right">
          <button onClick={() => navigate("devices")} className="flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left text-xs text-fg-2 hover:bg-white/5">
            <StatusDot tone={deviceTone} />
            <span className="truncate">{audio?.running?.deviceName ?? "No output device"}</span>
          </button>
        </Tip>
        <button onClick={() => navigate("devices")} className="flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left text-xs text-muted hover:bg-white/5">
          <span className="w-2" />
          <span className="truncate">Routing: {audio?.settings.routing.name ?? "—"}</span>
        </button>
      </div>
    </aside>
  );
}

export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden>
      <rect width="64" height="64" rx="15" fill="#17191d" />
      <rect x="0.5" y="0.5" width="63" height="63" rx="14.5" fill="none" stroke="rgba(255,255,255,0.08)" />
      <g fill="#f4b740">
        <rect x="13" y="26" width="5" height="12" rx="2.5" />
        <rect x="21" y="19" width="5" height="26" rx="2.5" />
        <rect x="29" y="23" width="5" height="18" rx="2.5" opacity="0.85" />
      </g>
      <path d="M39 21.5v21a1.5 1.5 0 0 0 2.3 1.27l16-10.5a1.5 1.5 0 0 0 0-2.54l-16-10.5A1.5 1.5 0 0 0 39 21.5Z" fill="#eceef1" />
    </svg>
  );
}
