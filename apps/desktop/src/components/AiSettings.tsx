import { openUrl } from "@tauri-apps/plugin-opener";
import { Bot, Check, KeyRound, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { FREELLM_DEFAULT_URL, modelOptions, normalizeOpenAiBase } from "@/lib/aiProvider";
import { errMsg, ipc } from "@/lib/ipc";
import type { AiStatus, AiTestResult, AppSettings } from "@/lib/types";
import { Badge, Button, Card, Row, Segmented } from "./ui";

type Provider = AppSettings["aiProvider"];

/** None (rules only) / Ollama / FreeLLMAPI. Live never calls the assistant. */
export function AiSettings({ settings, save }: {
  settings: AppSettings; save: (patch: Partial<AppSettings>, debounce?: number) => void;
}) {
  const provider: Provider = settings.aiProvider ?? (settings.ollamaEnabled ? "ollama" : "none");
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [test, setTest] = useState<AiTestResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [hasToken, setHasToken] = useState(false);
  const [token, setToken] = useState("");
  const [editingToken, setEditingToken] = useState(false);

  const refresh = async () => {
    setChecking(true);
    try {
      setStatus(await ipc.aiStatus());
    } catch {
      setStatus(null);
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    ipc.aiHasToken().then(setHasToken).catch(() => {});
  }, []);
  useEffect(() => {
    setTest(null);
    const t = window.setTimeout(refresh, 800);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider, settings.freellmUrl, settings.ollamaUrl, settings.freellmModel, settings.ollamaModel]);

  const runTest = async () => {
    setTesting(true);
    try {
      const r = await ipc.aiTest(provider, provider === "ollama" ? settings.ollamaUrl : settings.freellmUrl);
      setTest(r);
      if (r.ok) toast.success(r.message, { description: `${r.latencyMs} ms` });
      else toast.error("Connection failed", { description: r.message });
      refresh();
    } catch (e) {
      toast.error("Connection failed", { description: errMsg(e) });
    } finally {
      setTesting(false);
    }
  };

  const saveToken = async (value: string) => {
    try {
      const ok = await ipc.aiSetToken(value);
      setHasToken(ok);
      setToken("");
      setEditingToken(false);
      toast.success(ok ? "Token saved in Keychain" : "Token removed");
      refresh();
    } catch (e) {
      toast.error(errMsg(e));
    }
  };

  const badge = provider === "none" ? <Badge>Rules only</Badge>
    : checking ? <Loader2 className="size-4 animate-spin text-muted" />
      : status?.available ? <Badge tone="ok" dot>Connected</Badge>
        : <Badge tone="warn" dot>Not reachable</Badge>;

  const listed = test?.ok ? test.models : status?.models ?? [];

  return (
    <Card title={<span className="flex items-center gap-2"><Bot className="size-4 text-muted" />AI assistant (optional)</span>}
      subtitle="Turns your words into a proposal. Rules check it and nothing changes until you confirm." actions={badge}>
      <div className="mb-3">
        <Segmented<Provider>
          value={provider}
          onChange={(aiProvider) => save({ aiProvider, ollamaEnabled: aiProvider === "ollama" })}
          options={[
            { value: "none", label: "None", title: "Built-in rules only. No network." },
            { value: "ollama", label: "Ollama", title: "Local models on this Mac" },
            { value: "freellmapi", label: "FreeLLMAPI", title: "OpenAI-compatible proxy (localhost:31415)" },
          ]}
        />
      </div>

      {provider === "none" && (
        <p className="text-sm text-muted">Built-in rules only. Phrases like “drummer tidak main” or “count-in 2 bar” still work, with no network at all.</p>
      )}

      {provider === "ollama" && (
        <>
          <div className="divide-y divide-line">
            <Row label="Ollama URL">
              <input className="field h-8 w-56" value={settings.ollamaUrl} onChange={(e) => save({ ollamaUrl: e.target.value }, 700)} />
            </Row>
            <Row label="Model" hint={status?.models.length ? `${status.models.length} installed` : "Pull one with: ollama pull qwen2.5:7b"}>
              <select className="field h-8 w-56" value={settings.ollamaModel ?? ""} disabled={!status?.models.length} onChange={(e) => save({ ollamaModel: e.target.value || null })}>
                <option value="">Automatic{status?.model ? ` (${status.model})` : ""}</option>
                {status?.models.map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            </Row>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button size="sm" onClick={runTest} loading={testing}>Test connection</Button>
            <Button size="sm" variant="ghost" onClick={() => ipc.ollamaInstallInfo().then((i) => openUrl(i.downloadPage)).catch((e) => toast.error(errMsg(e)))}>
              Download Ollama (official)
            </Button>
            <Button size="sm" variant="ghost" onClick={() => ipc.ollamaPullModel("qwen2.5:1.5b").then(() => { toast.success("Model pulled"); refresh(); }).catch((e) => toast.error(errMsg(e)))}>
              Pull qwen2.5:1.5b
            </Button>
          </div>
        </>
      )}

      {provider === "freellmapi" && (
        <>
          <div className="divide-y divide-line">
            <Row label="Server URL" hint={`Uses ${normalizeOpenAiBase(settings.freellmUrl)}`}>
              <input className="field h-8 w-56" placeholder={FREELLM_DEFAULT_URL} value={settings.freellmUrl}
                onChange={(e) => save({ freellmUrl: e.target.value }, 700)} />
            </Row>
            <Row label="Unified token" hint="Stored in the macOS Keychain, never in a file">
              {hasToken && !editingToken ? (
                <div className="flex items-center gap-2">
                  <Badge tone="ok"><KeyRound className="size-3" />Saved</Badge>
                  <Button size="sm" variant="ghost" onClick={() => setEditingToken(true)}>Replace</Button>
                  <Button size="sm" variant="ghost" onClick={() => saveToken("")}>Remove</Button>
                </div>
              ) : (
                <form className="flex items-center gap-2" onSubmit={(e) => { e.preventDefault(); if (token.trim()) void saveToken(token); }}>
                  <input className="field h-8 w-44" type="password" autoComplete="off" placeholder="Paste token" value={token} onChange={(e) => setToken(e.target.value)} />
                  <Button size="sm" type="submit" disabled={!token.trim()} icon={<Check className="size-3.5" />}>Save</Button>
                </form>
              )}
            </Row>
            <Row label="Model" hint="auto lets FreeLLMAPI pick a working free model">
              <select className="field h-8 w-56" value={settings.freellmModel || "auto"} onChange={(e) => save({ freellmModel: e.target.value })}>
                {modelOptions(listed, settings.freellmModel).map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            </Row>
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button size="sm" onClick={runTest} loading={testing}>Test connection</Button>
            <Button size="sm" variant="ghost" onClick={() => openUrl("https://github.com/tashfeenahmed/freellmapi").catch(() => {})}>About FreeLLMAPI</Button>
            {test && <span className={test.ok ? "text-xs text-ok" : "text-xs text-err"}>{test.message}</span>}
            {!test && status && !status.available && status.error && <span className="text-xs text-warn">{status.error}</span>}
          </div>
        </>
      )}

      <div className="mt-3 rounded-xl bg-bg-2/60 p-3 text-xs text-muted">
        <b className="text-fg-2">Safe by design:</b> Live playback never calls the AI. If the AI is slow, offline or out of
        free quota, the assistant falls back to the built-in rules automatically.
      </div>
    </Card>
  );
}
