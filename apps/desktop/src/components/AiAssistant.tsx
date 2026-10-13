import { ArrowRight, Check, Loader2, Send, ShieldCheck, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { errMsg, ipc } from "@/lib/ipc";
import type { AiProposal, AiStatus } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Button } from "./ui";

let cachedStatus: { at: number; s: AiStatus } | null = null;

export function useAiStatus() {
  const enabled = useApp((s) => s.settings?.ollamaEnabled);
  const [status, setStatus] = useState<AiStatus | null>(cachedStatus?.s ?? null);
  useEffect(() => {
    if (cachedStatus && Date.now() - cachedStatus.at < 30_000) {
      setStatus(cachedStatus.s);
      return;
    }
    ipc.aiStatus().then((s) => { cachedStatus = { at: Date.now(), s }; setStatus(s); }).catch(() => {});
  }, [enabled]);
  return status;
}

/**
 * Natural-language MD assistant. The local model only *proposes* a structured intent; the Rust rules engine
 * validates it and the MD has to confirm before anything changes. Without Ollama, the same rules engine runs
 * on keyword matches. Live playback never calls the model.
 */
export function AiAssistant({ examples, run, onApply, placeholder, warmUp }: {
  examples: string[];
  run: (text: string) => Promise<AiProposal>;
  onApply: (p: AiProposal) => void | Promise<void>;
  placeholder: string;
  warmUp?: boolean;
}) {
  const status = useAiStatus();
  const navigate = useApp((s) => s.navigate);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [proposal, setProposal] = useState<AiProposal | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef(false);

  useEffect(() => {
    if (!warmUp || !status?.available) return;
    ipc.aiWarmup().catch(() => {});
  }, [warmUp, status?.available]);

  const submit = async (t = text) => {
    if (!t.trim() || busy) return;
    abortRef.current = false;
    setBusy(true);
    setError(null);
    setProposal(null);
    try {
      setProposal(await run(t));
    } catch (e) {
      if (!abortRef.current) setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  };

  const cancel = () => {
    abortRef.current = true;
    void ipc.aiCancel();
    setBusy(false);
    setError(null);
  };

  const rulesOnly = status?.rulesOnly ?? !status?.available;

  return (
    <div>
      {rulesOnly && (
        <div className="mb-3 rounded-lg border border-line bg-bg-2/50 px-3 py-2 text-xs text-muted">
          <span className="font-medium text-fg-2">No Ollama model connected.</span>{" "}
          Use the arrangement editor and presets above, or type common phrases (e.g. “drummer tidak main”) — the built-in rules engine will propose changes.
          <Button size="sm" variant="ghost" className="ml-2 inline-flex h-7" onClick={() => navigate("settings")}>Set up Ollama (optional)</Button>
        </div>
      )}
      <div className="relative">
        <textarea
          className="field min-h-[4.5rem] pr-11 text-sm"
          placeholder={placeholder}
          value={text}
          maxLength={800}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <button onClick={() => submit()} disabled={busy || !text.trim()} aria-label="Ask"
          className="absolute right-2 bottom-2.5 grid size-8 place-items-center rounded-lg bg-accent text-accent-ink disabled:opacity-30">
          {busy ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
        </button>
      </div>
      {!proposal && !busy && (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {examples.map((x) => (
            <button key={x} onClick={() => { setText(x); submit(x); }} className="rounded-full border border-line px-2.5 py-1 text-xs text-muted hover:border-line-2 hover:text-fg-2">
              {x}
            </button>
          ))}
        </div>
      )}
      {busy && (
        <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted">
          <Loader2 className="size-3.5 animate-spin" />
          {status?.available
            ? <>Thinking locally with {status.model ?? "Ollama"}…</>
            : <>Matching built-in rules…</>}
          {status?.available && (
            <Button size="sm" variant="ghost" onClick={cancel}>Cancel</Button>
          )}
        </div>
      )}
      {error && <div className="mt-3 rounded-lg border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">{error}</div>}
      {proposal && (
        <div className="mt-3 rounded-xl border border-mon/30 bg-mon/[0.06] p-3">
          <div className="flex items-start justify-between gap-2">
            <div className="text-sm font-medium">{proposal.summary}</div>
            <Badge tone="mon"><ShieldCheck className="size-3" />Checked by rules</Badge>
          </div>
          {proposal.changes.length > 0 ? (
            <ul className="mt-2.5 space-y-1">
              {proposal.changes.map((c, i) => (
                <li key={i} className="flex items-center gap-2 text-xs">
                  <span className="w-28 shrink-0 truncate text-muted">{c.field}</span>
                  <span className="truncate text-fg-2 line-through decoration-faint">{c.from || "—"}</span>
                  <ArrowRight className="size-3 shrink-0 text-faint" />
                  <span className="truncate font-semibold text-fg">{c.to}</span>
                </li>
              ))}
            </ul>
          ) : proposal.setlistSongIds.length === 0 && <div className="mt-2 text-xs text-muted">No changes needed.</div>}
          {proposal.warnings.map((w) => <div key={w} className="mt-2 text-xs text-warn">⚠ {w}</div>)}
          {proposal.requiresRerender && <div className="mt-2 text-xs text-muted">The backing track will be re-rendered after applying.</div>}
          <div className="mt-3 flex gap-2">
            <Button size="sm" variant="primary" icon={<Check className="size-3.5" />}
              disabled={proposal.changes.length === 0 && proposal.setlistSongIds.length === 0}
              onClick={async () => { await onApply(proposal); setProposal(null); setText(""); }}>
              Apply
            </Button>
            <Button size="sm" variant="ghost" icon={<X className="size-3.5" />} onClick={() => setProposal(null)}>Dismiss</Button>
            <span className="ml-auto self-center text-[0.68rem] text-faint">
              {proposal.provider === "ollama" ? `Ollama · ${proposal.model ?? status?.model ?? ""}` : "Built-in rules"}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
