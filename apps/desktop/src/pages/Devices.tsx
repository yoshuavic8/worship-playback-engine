import { AlertTriangle, AudioLines, Check, Headphones, Info, Mic, RefreshCw, Speaker, Square, XCircle } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { Metronome } from "@/components/icons";
import { BusMeters } from "@/components/Transport";
import { Badge, Button, Card, PageHeader, Segmented, Slider } from "@/components/ui";
import { cn, fmtDb } from "@/lib/format";
import { errMsg, ipc } from "@/lib/ipc";
import { PROFILES, channelMap, decodeTarget, encodeTarget, targetOptions, validateRouting } from "@/lib/routing";
import type { AudioSettings, DeviceInfo, RoutingProfile } from "@/lib/types";
import { useApp, usePlayback } from "@/store/app";

export function Devices() {
  const audio = useApp((s) => s.audio);
  const status = useApp((s) => s.audioStatus);
  const clickSounds = useApp((s) => s.clickSounds);
  const meters = usePlayback((s) => s.status?.meters);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [draft, setDraft] = useState<AudioSettings | null>(audio);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState<"foh" | "monitor" | "click" | "cue" | null>(null);
  const levelTimer = useRef<number | undefined>(undefined);

  const refresh = async () => {
    try {
      const list = await ipc.listDevices();
      setDevices(list);
      toast.success(list.length === 1 ? "1 audio device found" : `${list.length} audio devices found`);
    } catch (e) {
      toast.error(errMsg(e));
    }
  };
  useEffect(() => { refresh(); }, []);
  useEffect(() => { if (audio && !draft) setDraft(audio); }, [audio, draft]);

  const device = useMemo(() => devices.find((d) => d.name === (draft?.deviceName ?? status?.running?.deviceName)) ?? devices.find((d) => d.isDefault), [devices, draft, status]);
  const channels = draft?.deviceName === status?.running?.deviceName || !draft?.deviceName ? status?.running?.channels ?? device?.channels ?? 2 : device?.channels ?? 2;
  if (!draft) return null;

  const deviceChanged = draft.deviceName !== (audio?.deviceName ?? null) || draft.sampleRate !== audio?.sampleRate || draft.bufferSize !== audio?.bufferSize;
  const issues = validateRouting(draft.routing, channels);
  const blocked = issues.some((i) => i.level === "error");

  const apply = async (next: AudioSettings, quiet = false) => {
    setBusy(!quiet);
    try {
      const s = await ipc.saveAudio(next);
      useApp.getState().setAudioStatus(s);
      setDraft(s.settings);
      if (!quiet) toast.success(s.running ? `Output: ${s.running.deviceName}` : "Saved");
      if (s.error) toast.error(s.error);
    } catch (e) {
      toast.error("Could not apply audio settings", { description: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };

  // Routing & levels apply immediately (no device restart); device / rate / buffer need "Apply".
  const setRouting = (routing: RoutingProfile) => {
    const next = { ...draft, routing };
    setDraft(next);
    if (!deviceChanged && validateRouting(routing, channels).every((i) => i.level !== "error")) apply(next, true);
  };
  const setLevel = (patch: Partial<AudioSettings>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    window.clearTimeout(levelTimer.current);
    if (!deviceChanged) levelTimer.current = window.setTimeout(() => apply(next, true), 250);
  };

  const map = channelMap(draft.routing, channels);
  const rates = device?.sampleRates.filter((r) => [44100, 48000, 88200, 96000].includes(r)) ?? [48000];

  const runTest = async (kind: "foh" | "monitor" | "click" | "cue" | "stop") => {
    if (kind === "stop") {
      setTesting(null);
      try {
        await ipc.testSignal("stop");
      } catch (e) {
        toast.error(errMsg(e));
      }
      return;
    }
    setTesting(kind);
    try {
      await ipc.testSignal(kind);
      const label = kind === "foh" ? "FOH test tone" : kind === "monitor" ? "Monitor test tone" : kind === "click" ? "Click preview" : "Cue preview";
      toast.message(label, { description: "Playing on the monitor or FOH bus as routed." });
    } catch (e) {
      setTesting(null);
      toast.error(errMsg(e));
    }
  };

  return (
    <div className="pb-10">
      <PageHeader title="Devices & routing" subtitle="Where FOH and the band’s monitor mix go"
        actions={<Button variant="ghost" icon={<RefreshCw className="size-4" />} onClick={refresh}>Rescan</Button>} />
      <div className="grid grid-cols-12 gap-5 px-8 pt-6">
        <div className="col-span-12 space-y-5 xl:col-span-7">
          <Card title="Output device" actions={status?.running ? <Badge tone="ok" dot>Running</Badge> : <Badge tone="err" dot>Stopped</Badge>}>
            <div className="grid grid-cols-[1fr_9rem] gap-3">
              <label>
                <span className="label mb-1.5 block">Interface</span>
                <select className="field" value={draft.deviceName ?? ""} onChange={(e) => setDraft({ ...draft, deviceName: e.target.value || null })}>
                  <option value="">System default{devices.find((d) => d.isDefault) ? ` (${devices.find((d) => d.isDefault)!.name})` : ""}</option>
                  {devices.map((d) => <option key={d.name} value={d.name}>{d.name} — {d.channels} out</option>)}
                </select>
              </label>
              <label>
                <span className="label mb-1.5 block">Sample rate</span>
                <select className="field" value={draft.sampleRate} onChange={(e) => setDraft({ ...draft, sampleRate: Number(e.target.value) })}>
                  {(rates.length ? rates : [48000]).map((r) => <option key={r} value={r}>{r / 1000} kHz</option>)}
                </select>
              </label>
            </div>
            <div className="mt-4">
              <span className="label mb-1.5 block">Buffer size</span>
              <Segmented<number> value={draft.bufferSize} onChange={(bufferSize) => setDraft({ ...draft, bufferSize })}
                options={[64, 128, 256, 512, 1024].map((b) => ({ value: b, label: String(b), title: `${((b / draft.sampleRate) * 1000).toFixed(1)} ms` }))} />
              <p className="mt-1.5 text-xs text-muted">256 is a safe default. Larger = more stable, smaller = less latency. Playback is pre-buffered, so latency only affects button response.</p>
            </div>
            {status?.running && (
              <div className="mt-4 grid grid-cols-4 gap-3 rounded-xl bg-bg-2/60 p-3 text-center">
                <Mini label="Outputs" value={String(status.running.channels)} />
                <Mini label="Rate" value={`${status.running.sampleRate / 1000} kHz`} />
                <Mini label="Buffer" value={status.running.bufferSize ? String(status.running.bufferSize) : "auto"} />
                <Mini label="Latency" value={status.running.latencyMs ? `${status.running.latencyMs.toFixed(1)} ms` : "—"} />
              </div>
            )}
            {status?.error && <div className="mt-3 rounded-lg border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">{status.error}</div>}
            {deviceChanged && (
              <div className="mt-4 flex items-center justify-between rounded-xl border border-accent/30 bg-accent/[0.07] px-3 py-2.5">
                <span className="text-xs text-fg-2">Applying restarts the audio stream (stops playback).</span>
                <div className="flex gap-2">
                  <Button size="sm" variant="ghost" onClick={() => setDraft(audio)}>Cancel</Button>
                  <Button size="sm" variant="primary" loading={busy} onClick={() => apply(draft)}>Apply</Button>
                </div>
              </div>
            )}
          </Card>

          <Card title="Routing profile" subtitle="Click, cues and guide vocal are monitor-only by design — they cannot be routed to FOH">
            <div className="grid grid-cols-3 gap-2">
              <ProfileCard active={draft.routing.mode === "multi"} onClick={() => setRouting(PROFILES.multi)} disabled={channels < 4}
                icon={<AudioLines className="size-5" />} title="Multi-output" desc="FOH 1-2 · Monitor 3-4" note={channels < 4 ? "Needs 4+ outputs" : "Recommended"} />
              <ProfileCard active={draft.routing.mode === "split"} onClick={() => setRouting(PROFILES.split)}
                icon={<Speaker className="size-5" />} title="L/R split" desc="FOH left · Monitor right (mono)" note="Any stereo interface" />
              <ProfileCard active={draft.routing.mode === "rehearsal"} onClick={() => setRouting(PROFILES.rehearsal)}
                icon={<Headphones className="size-5" />} title="Rehearsal" desc="Monitor mix on 1-2" note="Headphones / laptop" />
            </div>
            <div className="mt-5 grid grid-cols-2 gap-3">
              <label>
                <span className="label mb-1.5 block text-foh">FOH bus (house)</span>
                <select className="field" value={encodeTarget(draft.routing.foh)}
                  onChange={(e) => setRouting({ ...draft.routing, id: "custom", name: "Custom", mode: "custom", foh: decodeTarget(e.target.value) })}>
                  {targetOptions(channels).map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
                </select>
              </label>
              <label>
                <span className="label mb-1.5 block text-mon">Monitor bus (in-ears)</span>
                <select className="field" value={encodeTarget(draft.routing.monitor)}
                  onChange={(e) => setRouting({ ...draft.routing, id: "custom", name: "Custom", mode: "custom", monitor: decodeTarget(e.target.value) })}>
                  {targetOptions(channels).map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
                </select>
              </label>
            </div>

            {/* diagram */}
            <div className="mt-5 rounded-xl border border-line bg-bg-2/60 p-4">
              <div className="label mb-3">Physical outputs</div>
              <div className="flex flex-wrap gap-2">
                {map.map((b, i) => (
                  <div key={i} className={cn("flex h-16 w-16 flex-col items-center justify-center rounded-xl border text-xs font-bold",
                    b === "foh" ? "border-foh/50 bg-foh/10 text-foh" : b === "mon" ? "border-mon/50 bg-mon/10 text-mon" : b === "conflict" ? "border-err bg-err/15 text-err" : "border-line text-faint")}>
                    <span className="tnum text-lg">{i + 1}</span>
                    <span className="text-[0.62rem] tracking-wider">{b === "foh" ? "FOH" : b === "mon" ? "MON" : b === "conflict" ? "CLASH" : "—"}</span>
                  </div>
                ))}
              </div>
              <div className="mt-4 grid grid-cols-2 gap-4 text-xs">
                <div><div className="mb-1 font-bold text-foh">FOH receives</div><div className="text-fg-2">Backing track (instrumental)</div></div>
                <div><div className="mb-1 font-bold text-mon">Monitor receives</div><div className="text-fg-2">Click · voice cues · guide vocal · backing (optional)</div></div>
              </div>
            </div>
            <div className="mt-4 space-y-1.5">
              {issues.map((i) => (
                <div key={i.message} className={cn("flex items-start gap-2 text-xs", i.level === "error" ? "text-err" : "text-warn")}>
                  {i.level === "error" ? <XCircle className="mt-px size-3.5 shrink-0" /> : i.level === "warning" ? <AlertTriangle className="mt-px size-3.5 shrink-0" /> : <Info className="mt-px size-3.5 shrink-0" />}
                  {i.message}
                </div>
              ))}
              {!issues.length && <div className="flex items-center gap-2 text-xs text-ok"><Check className="size-3.5" />Live-safe routing</div>}
              {blocked && <div className="text-xs text-muted">This routing is not saved until the problem is fixed.</div>}
            </div>
          </Card>
        </div>

        <div className="col-span-12 space-y-5 xl:col-span-5">
          <Card title="Test outputs" subtitle="Check before the service — each signal plays only on its bus">
            <div className="grid grid-cols-2 gap-2">
              <TestBtn active={testing === "foh"} tone="foh" icon={<Speaker className="size-4" />} label="Test FOH" sub={testing === "foh" ? "Playing on FOH…" : "440 Hz tone"} onClick={() => void runTest("foh")} />
              <TestBtn active={testing === "monitor"} tone="mon" icon={<Headphones className="size-4" />} label="Test monitor" sub={testing === "monitor" ? "Playing on monitor…" : "880 Hz tone"} onClick={() => void runTest("monitor")} />
              <TestBtn active={testing === "click"} tone="mon" icon={<Metronome className="size-4" />} label="Test click" sub={testing === "click" ? "Click on monitor…" : "1 bar @ 100 BPM"} onClick={() => void runTest("click")} />
              <TestBtn active={testing === "cue"} tone="mon" icon={<Mic className="size-4" />} label="Test cue" sub={testing === "cue" ? "Cue on monitor…" : "Spoken cue"} onClick={() => void runTest("cue")} />
            </div>
            <Button className="mt-3 w-full" variant="outline" icon={<Square className="size-3.5" />} onClick={() => void runTest("stop")}>Stop test</Button>
            <div className="mt-4 rounded-xl bg-bg-2/60 p-3"><BusMeters meters={meters} className="w-full" /></div>
          </Card>

          <Card title="Monitor levels">
            <div className="space-y-4">
              <LevelRow label="Click" value={draft.clickVolumeDb} onChange={(clickVolumeDb) => setLevel({ clickVolumeDb })} />
              <LevelRow label="Voice cues" value={draft.cueVolumeDb} onChange={(cueVolumeDb) => setLevel({ cueVolumeDb })} />
              <LevelRow label="Backing in monitor" value={draft.monitorBackingDb} onChange={(monitorBackingDb) => setLevel({ monitorBackingDb })} />
              <div>
                <span className="label mb-1.5 block">Click sound</span>
                <div className="flex gap-2">
                  <select className="field" value={draft.clickSound} onChange={(e) => setLevel({ clickSound: e.target.value })}>
                    {clickSounds.map(([id, name]) => <option key={id} value={id}>{name}</option>)}
                  </select>
                  <Button variant="outline" onClick={() => void runTest("click")}>Preview</Button>
                </div>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

function ProfileCard({ active, onClick, icon, title, desc, note, disabled }: {
  active: boolean; onClick: () => void; icon: React.ReactNode; title: string; desc: string; note: string; disabled?: boolean;
}) {
  return (
    <button onClick={onClick} disabled={disabled}
      className={cn("rounded-xl border p-3 text-left transition-colors disabled:opacity-40", active ? "border-accent/60 bg-accent/[0.08]" : "border-line bg-bg-2/60 hover:border-line-2")}>
      <div className={cn("mb-2", active ? "text-accent" : "text-muted")}>{icon}</div>
      <div className="text-sm font-semibold">{title}</div>
      <div className="mt-0.5 text-xs text-fg-2">{desc}</div>
      <div className="mt-1.5 text-[0.68rem] text-muted">{note}</div>
    </button>
  );
}

function TestBtn({ icon, label, sub, onClick, tone, active }: { icon: React.ReactNode; label: string; sub: string; onClick: () => void; tone: "foh" | "mon"; active?: boolean }) {
  return (
    <button onClick={onClick} className={cn("flex items-center gap-3 rounded-xl border px-3 py-3 text-left transition-colors",
      active ? (tone === "foh" ? "border-foh bg-foh/15" : "border-mon bg-mon/15") : tone === "foh" ? "border-foh/30 hover:bg-foh/[0.07]" : "border-mon/30 hover:bg-mon/[0.07]")}>
      <span className={tone === "foh" ? "text-foh" : "text-mon"}>{icon}</span>
      <div>
        <div className="text-sm font-semibold">{label}</div>
        <div className="text-xs text-muted">{sub}</div>
      </div>
    </button>
  );
}

function LevelRow({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }) {
  return (
    <div className="grid grid-cols-[8.5rem_1fr_4rem] items-center gap-3">
      <span className="text-sm">{label}</span>
      <Slider value={value} min={-40} max={6} step={0.5} onChange={onChange} tone="mon" />
      <span className="tnum text-right text-xs text-muted">{fmtDb(value)}</span>
    </div>
  );
}

function Mini({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="tnum text-sm font-semibold">{value}</div>
      <div className="text-[0.68rem] text-muted">{label}</div>
    </div>
  );
}
