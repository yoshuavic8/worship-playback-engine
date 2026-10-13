import * as TooltipP from "@radix-ui/react-tooltip";
import { Download, Loader2 } from "lucide-react";
import { useEffect } from "react";
import { Toaster } from "sonner";
import { MiniTransport } from "./components/Transport";
import { Sidebar, Logo } from "./components/Sidebar";
import { connectEvents } from "./lib/events";
import { isTauri } from "./lib/ipc";
import { useShortcuts } from "./lib/useShortcuts";
import { Dashboard } from "./pages/Dashboard";
import { Devices } from "./pages/Devices";
import { Diagnostics } from "./pages/Diagnostics";
import { Live } from "./pages/Live";
import { Preparation } from "./pages/Preparation";
import { SetlistEditor } from "./pages/SetlistEditor";
import { Setlists } from "./pages/Setlists";
import { Settings } from "./pages/Settings";
import { SongEditor } from "./pages/SongEditor";
import { Songs } from "./pages/Songs";
import { useApp } from "./store/app";

export default function App() {
  const ready = useApp((s) => s.ready);
  const bootError = useApp((s) => s.bootError);
  const route = useApp((s) => s.route);
  const importing = useApp((s) => s.importing);
  useShortcuts();

  useEffect(() => {
    if (!useApp.getState().ready) useApp.getState().init();
    let off: (() => void) | undefined;
    let cancelled = false;
    connectEvents().then((f) => (cancelled ? f() : (off = f)));
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  if (!ready) return <Boot error={bootError} />;

  const live = route.page === "live";
  return (
    <TooltipP.Provider>
      <div className="app-bg flex h-full">
        {!live && <Sidebar />}
        <div className="flex min-w-0 flex-1 flex-col">
          <main key={`${route.page}:${route.id ?? ""}`} className={`min-h-0 flex-1 ${route.page === "song" ? "overflow-hidden" : "overflow-y-auto"}`}>
            <PageView />
          </main>
          {!live && <MiniTransport />}
        </div>
      </div>
      {importing && (
        <div className="pointer-events-none fixed inset-0 z-50 grid place-items-center bg-bg/80 backdrop-blur-sm">
          <div className="flex flex-col items-center gap-3 rounded-3xl border-2 border-dashed border-accent/60 px-16 py-12">
            <Download className="size-10 text-accent" />
            <div className="text-lg font-semibold">Drop audio files to import</div>
            <div className="text-sm text-muted">MP3 · WAV · FLAC · M4A · AIFF</div>
          </div>
        </div>
      )}
      <Toaster theme="dark" position="bottom-right" offset={84} richColors closeButton
        toastOptions={{ style: { background: "#1c1f24", border: "1px solid #2a2e35", color: "#eceef1" } }} />
    </TooltipP.Provider>
  );
}

function PageView() {
  const route = useApp((s) => s.route);
  switch (route.page) {
    case "dashboard": return <Dashboard />;
    case "songs": return <Songs />;
    case "song": return route.id ? <SongEditor id={route.id} /> : <Songs />;
    case "setlists": return <Setlists />;
    case "setlist": return route.id ? <SetlistEditor id={route.id} /> : <Setlists />;
    case "live": return <Live />;
    case "prep": return <Preparation />;
    case "devices": return <Devices />;
    case "settings": return <Settings />;
    case "diagnostics": return <Diagnostics />;
  }
}

function Boot({ error }: { error: string | null }) {
  return (
    <div className="app-bg drag grid h-full place-items-center">
      <div className="flex flex-col items-center gap-5 text-center">
        <Logo size={64} />
        {error ? (
          <div className="no-drag max-w-md">
            <div className="text-lg font-semibold">Could not start</div>
            <p className="mt-2 text-sm text-muted">{isTauri() ? error : "This UI runs inside the Worship Playback Engine desktop app. Start it with “npm run tauri dev”."}</p>
          </div>
        ) : (
          <div className="flex items-center gap-2 text-sm text-muted"><Loader2 className="size-4 animate-spin" /> Starting audio engine…</div>
        )}
      </div>
    </div>
  );
}
