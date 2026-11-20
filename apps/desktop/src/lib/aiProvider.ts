// AI provider helpers (mirror of the Rust rules, used for previews in Settings).

export const FREELLM_DEFAULT_URL = "http://localhost:31415";
export const FREELLM_ALIASES = ["auto", "auto:fast", "auto:smart"] as const;

/** Base URL with http:// and `/v1` exactly once (pasted endpoint URLs are accepted too). */
export function normalizeOpenAiBase(url: string): string {
  let u = url.trim().replace(/\/+$/, "");
  if (!u) u = FREELLM_DEFAULT_URL;
  if (!u.includes("://")) u = `http://${u}`;
  for (const suffix of ["/chat/completions", "/models"]) {
    if (u.endsWith(suffix)) u = u.slice(0, -suffix.length).replace(/\/+$/, "");
  }
  if (!u.endsWith("/v1")) u += "/v1";
  return u;
}

/** Model picker options: router aliases first, then listed models (no duplicates). */
export function modelOptions(listed: string[], current?: string | null): string[] {
  const out: string[] = [...FREELLM_ALIASES];
  for (const m of [...listed, current ?? ""]) if (m && !out.includes(m)) out.push(m);
  return out;
}
