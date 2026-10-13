import { Bot, Check, FolderOpen, Keyboard, Loader2, Play, RotateCcw, Upload, Download, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Badge, Button, Card, PageHeader, Row, Segmented, Switch } from "@/components/ui";
import { openDialog, pickFolder, saveDialog } from "@/lib/actions";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errMsg, ipc } from "@/lib/ipc";
import { DEFAULT_SHORTCUTS, SHORTCUT_ACTIONS, keyLabel } from "@/lib/shortcuts";
import type { AiStatus, AppSettings, Voice } from "@/lib/types";
import { useApp } from "@/store/app";

export function Settings() {
  const settings = useApp((s) => s.settings);
  const dataDir = useApp((s) => s.dataDir);
  const version = useApp((s) => s.version);
  const [voices, setVoices] = useState<Voice[]>([]);
  const [ai, setAi] = useState<AiStatus | null>(null);
  const [checkingAi, setCheckingAi] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    ipc.listVoices().then(setVoices).catch(() => {});
    checkAi();
  }, []);

  const checkAi = async () => {
    setCheckingAi(true);
    try {
      const status = await ipc.aiStatus();
      setAi(status);
      if (status.available) {
        toast.success(status.model ? `Connected · ${status.model}` : `Connected · ${status.provider}`);
      } else if (settings?.ollamaEnabled) {
        toast.message("Ollama not connected", { description: "Optional — built-in rules and manual controls still work." });
      }
    } catch {
      setAi(null);
      toast.error("Could not check AI connection");
    } finally {
      setCheckingAi(false);
    }
  };

  if (!settings) return null;

  const save = (patch: Partial<AppSettings>, debounce = 0) => {
    const next = { ...settings, ...patch };
    useApp.getState().setSettings(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      try {
        const s = await ipc.saveSettings(next);
        useApp.getState().setSettings(s);
        if ("ollamaEnabled" in patch || "ollamaUrl" in patch || "ollamaModel" in patch) checkAi();
      } catch (e) {
        toast.error("Could not save settings", { description: errMsg(e) });
      }
    }, debounce);
  };

  const shortcuts = { ...DEFAULT_SHORTCUTS, ...settings.shortcuts };
  const english = voices.filter((v) => v.locale.startsWith("en"));
  const indo = voices.filter((v) => v.locale.startsWith("id"));
  const others = voices.filter((v) => !v.locale.startsWith("en") && !v.locale.startsWith("id"));

  return (
    <div className="pb-10">
      <PageHeader title="Settings" subtitle={`Version ${version} · everything is stored locally on this Mac`} />
      <div className="mx-auto grid max-w-6xl grid-cols-12 gap-5 px-8 pt-6">
        <div className="col-span-12 space-y-5 xl:col-span-7">
          <Card title="General">
            <div className="divide-y divide-line">
              <Row label="Your name" hint="Shown on the dashboard">
                <input className="field h-8 w-56" value={settings.userName} onChange={(e) => save({ userName: e.target.value }, 500)} />
              </Row>
              <Row label="Library location" hint={settings.libraryPath ?? `${dataDir}/songs`}>
                {settings.libraryPath && <Button size="sm" variant="ghost" onClick={() => save({ libraryPath: null })}>Default</Button>}
                <Button size="sm" variant="outline" icon={<FolderOpen className="size-3.5" />} onClick={async () => { const p = await pickFolder("Choose library folder"); if (p) save({ libraryPath: p }); }}>Change…</Button>
              </Row>
              <Row label="Interface size" hint="Bigger is easier to read from a distance">
                <Segmented<number> size="sm" value={settings.uiScale} onChange={(uiScale) => save({ uiScale })}
                  options={[{ value: 0.9, label: "S" }, { value: 1, label: "M" }, { value: 1.1, label: "L" }, { value: 1.25, label: "XL" }]} />
              </Row>
              <Row label="Display language" hint="Dates and relative times. Editor and Live labels stay in English for now.">
                <Segmented<"en" | "id"> size="sm" value={settings.uiLocale ?? "en"} onChange={(uiLocale) => save({ uiLocale })}
                  options={[{ value: "en", label: "English" }, { value: "id", label: "Indonesia" }]} />
              </Row>
            </div>
          </Card>

          <Card title="Playback">
            <div className="divide-y divide-line">
              <Row label="Default count-in" hint="Clicks for this many bars, then the song enters on beat 1">
                <Segmented<number> size="sm" value={settings.defaultCountInBars} onChange={(defaultCountInBars) => save({ defaultCountInBars })}
                  options={[{ value: 0, label: "Off" }, { value: 1, label: "1 bar" }, { value: 2, label: "2 bars" }]} />
              </Row>
              <Row label="Auto-advance in live mode" hint="Load the next song when one ends. “Auto” transitions also start it.">
                <Switch checked={settings.autoAdvance} onChange={(autoAdvance) => save({ autoAdvance })} />
              </Row>
              <Row label="Cue voice" hint="macOS voice used for spoken cues (“Verse”, “Chorus”…). Pre-rendered, works offline.">
                <select className="field h-8 w-52" value={settings.cueVoice ?? ""} onChange={(e) => save({ cueVoice: e.target.value || null })}>
                  <option value="">System default</option>
                  {english.length > 0 && <optgroup label="English">{english.map((v) => <option key={v.name} value={v.name}>{v.name} ({v.locale})</option>)}</optgroup>}
                  {indo.length > 0 && <optgroup label="Bahasa Indonesia">{indo.map((v) => <option key={v.name} value={v.name}>{v.name}</option>)}</optgroup>}
                  {others.length > 0 && <optgroup label="Other">{others.map((v) => <option key={v.name} value={v.name}>{v.name} ({v.locale})</option>)}</optgroup>}
                </select>
                <Button size="sm" variant="outline" icon={<Play className="size-3.5" />} onClick={() => ipc.previewCue("Chorus", settings.cueVoice).catch((e) => toast.error(errMsg(e)))}>Preview</Button>
              </Row>
            </div>
          </Card>

          <Card title="Stem separation" subtitle="Runs only during preparation, never during live playback">
            <div className="divide-y divide-line">
              <Row label="Profile" hint="Worship removes vocals with Inst HQ 3, then splits drums, bass and other with Demucs. Fast is Demucs only.">
                <Segmented<string> size="sm" value={settings.separationProfile ?? "worship"} onChange={(separationProfile) => save({ separationProfile })}
                  options={[{ value: "worship", label: "Worship" }, { value: "fast", label: "Fast" }]} />
              </Row>
              <Row label="Engine" hint="Instrument splitter. Auto tries MLX, then PyTorch on GPU, then CPU">
                <Segmented<string> size="sm" value={settings.separationEngine} onChange={(separationEngine) => save({ separationEngine })}
                  options={[{ value: "auto", label: "Auto" }, { value: "demucs-mlx", label: "MLX" }, { value: "demucs", label: "PyTorch" }]} />
              </Row>
              <Row label="Model" hint="htdemucs_ft is slightly cleaner but ~4× slower">
                <Segmented<string> size="sm" value={settings.separationModel} onChange={(separationModel) => save({ separationModel })}
                  options={[{ value: "htdemucs", label: "htdemucs" }, { value: "htdemucs_ft", label: "htdemucs_ft" }]} />
              </Row>
              <Row label="PyTorch device" hint="Used when MLX is unavailable">
                <Segmented<string> size="sm" value={settings.separationDevice} onChange={(separationDevice) => save({ separationDevice })}
                  options={[{ value: "auto", label: "Auto" }, { value: "mps", label: "GPU (MPS)" }, { value: "cpu", label: "CPU" }]} />
              </Row>
            </div>
          </Card>

          <Card title={<span className="flex items-center gap-2"><Keyboard className="size-4 text-muted" />Keyboard shortcuts</span>}
            actions={<Button size="sm" variant="ghost" icon={<RotateCcw className="size-3.5" />} onClick={() => save({ shortcuts: { ...DEFAULT_SHORTCUTS } })}>Reset</Button>}>
            <div className="grid grid-cols-2 gap-x-6 gap-y-1">
              {SHORTCUT_ACTIONS.map((a) => (
                <ShortcutRow key={a.id} label={a.label} code={shortcuts[a.id]} taken={Object.entries(shortcuts).filter(([k]) => k !== a.id).map(([, v]) => v)}
                  onChange={(code) => save({ shortcuts: { ...shortcuts, [a.id]: code } })} />
              ))}
            </div>
            <p className="mt-3 text-xs text-muted">PANIC always works, even while typing. With a dialog open, use Shift+Esc.</p>
          </Card>
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-5">
          <Card title={<span className="flex items-center gap-2"><Bot className="size-4 text-muted" />Asisten AI (Ollama, opsional)</span>}
            subtitle="Matikan = tidak ada probe jaringan. Aplikasi tetap penuh dengan aturan bawaan."
            actions={checkingAi ? <Loader2 className="size-4 animate-spin text-muted" /> : ai?.available ? <Badge tone="ok" dot>Connected</Badge> : settings.ollamaEnabled ? <Badge tone="warn" dot>Not reachable</Badge> : <Badge>Off</Badge>}>
            <div className="divide-y divide-line">
              <Row label="Aktifkan Asisten AI" hint="Contoh: “drummer tidak main”, “count-in 2 bar”. Bisa dimatikan kapan saja.">
                <Switch checked={settings.ollamaEnabled} onChange={(ollamaEnabled) => save({ ollamaEnabled })} />
              </Row>
              <Row label="Ollama URL">
                <input className="field h-8 w-56" value={settings.ollamaUrl} disabled={!settings.ollamaEnabled} onChange={(e) => save({ ollamaUrl: e.target.value }, 700)} />
              </Row>
              <Row label="Model" hint={ai?.models.length ? `${ai.models.length} installed` : "Pull one with: ollama pull qwen2.5:7b"}>
                <select className="field h-8 w-56" value={settings.ollamaModel ?? ""} disabled={!settings.ollamaEnabled || !ai?.models.length} onChange={(e) => save({ ollamaModel: e.target.value || null })}>
                  <option value="">Automatic{ai?.model ? ` (${ai.model})` : ""}</option>
                  {ai?.models.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              </Row>
            </div>
            <div className="mt-3 rounded-xl bg-bg-2/60 p-3 text-xs text-muted">
              <b className="text-fg-2">How it is used:</b> the model only translates your words into a structured request. A rules engine checks it,
              shows the exact changes, and nothing is applied until you confirm. Live playback never calls the model — if Ollama is closed,
              the service runs exactly the same.
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <Button size="sm" variant="outline" onClick={() => ipc.ollamaInstallInfo().then((i) => openUrl(i.downloadPage)).catch((e) => toast.error(errMsg(e)))}>
                Unduh Ollama (resmi)
              </Button>
              <Button size="sm" variant="ghost" onClick={checkAi} loading={checkingAi} disabled={!settings.ollamaEnabled}>Uji koneksi</Button>
              {settings.ollamaEnabled && (
                <Button size="sm" variant="ghost" onClick={() => ipc.ollamaPullModel("qwen2.5:1.5b").then(() => toast.success("Model ditarik")).catch((e) => toast.error(errMsg(e)))}>
                  Tarik qwen2.5:1.5b
                </Button>
              )}
            </div>
          </Card>

          <Card title="Backup & move" subtitle="Library metadata, setlists, presets and routing as one JSON file">
            <div className="flex gap-2">
              <Button variant="outline" icon={<Download className="size-4" />} onClick={async () => {
                const p = await saveDialog(`wpe-library-${new Date().toISOString().slice(0, 10)}.json`, "Library", "json");
                if (p) ipc.exportLibrary(p).then((n) => toast.success(`Exported ${n} songs`)).catch((e) => toast.error(errMsg(e)));
              }}>Export library</Button>
              <Button variant="outline" icon={<Upload className="size-4" />} onClick={async () => {
                const p = await openDialog("Library", "json");
                if (p) ipc.importLibrary(p).then((r) => toast.success(`Imported ${r.songs} songs, ${r.setlists} setlists`, { description: r.skippedSongs ? `${r.skippedSongs} songs skipped (audio files not found)` : undefined })).catch((e) => toast.error(errMsg(e)));
              }}>Import</Button>
            </div>
            <p className="mt-3 text-xs text-muted">Audio files stay in the library folder. To move to another Mac, copy the library folder and import this file.</p>
          </Card>

          <Card title="Privacy">
            <ul className="space-y-1.5 text-xs text-muted">
              {["No telemetry, no accounts, no cloud upload", "Songs never leave this Mac", "Models download only when you click Download, with checksum verification", "AI runs on your own Ollama — optional"].map((t) => (
                <li key={t} className="flex gap-2"><Check className="mt-px size-3.5 shrink-0 text-ok" />{t}</li>
              ))}
            </ul>
          </Card>
        </div>
      </div>
    </div>
  );
}

function ShortcutRow({ label, code, taken, onChange }: { label: string; code: string; taken: string[]; onChange: (c: string) => void }) {
  const [capturing, setCapturing] = useState(false);
  useEffect(() => {
    if (!capturing) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape" && code !== "Escape") return setCapturing(false);
      if (["MetaLeft", "MetaRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "ControlLeft", "ControlRight"].includes(e.code)) return;
      if (taken.includes(e.code)) {
        toast.warning(`${keyLabel(e.code)} is already used`);
        return;
      }
      onChange(e.code);
      setCapturing(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing, code, taken, onChange]);
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-sm text-fg-2">{label}</span>
      <button data-capturing-shortcut={capturing ? "" : undefined} onClick={() => setCapturing(!capturing)}
        className={`flex h-7 min-w-16 items-center justify-center gap-1 rounded-md border px-2 text-xs font-semibold ${capturing ? "border-accent bg-accent/15 text-accent" : "border-line-2 bg-panel-2 text-fg-2 hover:border-[#4a505b]"}`}>
        {capturing ? <>Press a key <X className="size-3" /></> : keyLabel(code)}
      </button>
    </div>
  );
}
