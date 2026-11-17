import { KEYS, MINOR_KEYS } from "./keys";
import type { UiLocale } from "./locale";
import { getUiLocale } from "./locale";

/** Solfège names for Indonesian worship context (do = major tonic). */
const SOLFEGE = ["do", "di", "re", "ri", "mi", "fa", "fi", "sol", "si", "la", "li", "ti"] as const;

export function pitchClassFromKey(key: string | null | undefined): number | null {
  if (!key) return null;
  const major = KEYS.indexOf(key);
  if (major >= 0) return major;
  const minor = MINOR_KEYS.indexOf(key);
  if (minor >= 0) return minor;
  return null;
}

export function modeFromKey(key: string | null | undefined): "major" | "minor" | null {
  if (!key) return null;
  if (KEYS.includes(key)) return "major";
  if (MINOR_KEYS.includes(key)) return "minor";
  return null;
}

export function keyFromPitchClass(pc: number, mode: "major" | "minor"): string {
  const i = ((pc % 12) + 12) % 12;
  return mode === "minor" ? MINOR_KEYS[i]! : KEYS[i]!;
}

/** e.g. Indonesian: "Nada dasar: do = G" */
export function formatKeyDisplay(
  key: string | null | undefined,
  locale: UiLocale = getUiLocale(),
): string {
  if (!key) return "—";
  if (locale !== "id") return key;
  const pc = pitchClassFromKey(key);
  if (pc == null) return key;
  const sol = SOLFEGE[pc];
  return `Nada dasar: ${sol} = ${key.replace("m", "")}`;
}
