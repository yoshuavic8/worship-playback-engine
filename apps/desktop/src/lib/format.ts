import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { getUiLocale, localeTag } from "./locale";

export const cn = (...v: ClassValue[]) => twMerge(clsx(v));

/** 83.4 → "1:23" ; with tenths → "1:23.4" */
export function fmtTime(sec: number | null | undefined, tenths = false): string {
  if (sec == null || !Number.isFinite(sec) || sec < 0) sec = 0;
  const m = Math.floor(sec / 60);
  const s = sec - m * 60;
  if (tenths) {
    const whole = Math.floor(s);
    const t = Math.floor((s - whole) * 10);
    return `${m}:${String(whole).padStart(2, "0")}.${t}`;
  }
  return `${m}:${String(Math.floor(s)).padStart(2, "0")}`;
}

export function fmtDuration(sec: number): string {
  if (sec >= 3600) {
    const h = Math.floor(sec / 3600);
    const m = Math.round((sec - h * 3600) / 60);
    return `${h} h ${m} min`;
  }
  return `${Math.round(sec / 60)} min`;
}

export function fmtBytes(n: number | null | undefined): string {
  if (!n) return "0 B";
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${u[i]}`;
}

export function fmtDb(db: number): string {
  if (db <= -60) return "−∞";
  const r = Math.round(db * 10) / 10;
  return `${r > 0 ? "+" : r < 0 ? "−" : ""}${Math.abs(r)} dB`;
}

/** Linear peak (0..1+) → meter fill 0..1 on a −60…0 dBFS scale. */
export function meterLevel(peak: number): number {
  if (!peak || peak <= 0) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 60) / 60));
}

function parseDay(iso: string): Date {
  return new Date(iso.length === 10 ? `${iso}T12:00:00` : iso);
}

export function relTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return "—";
  const d = (Date.now() - t) / 1000;
  const id = getUiLocale() === "id";
  if (d < 60) return id ? "baru saja" : "just now";
  if (d < 3600) return id ? `${Math.floor(d / 60)} mnt lalu` : `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return id ? `${Math.floor(d / 3600)} jam lalu` : `${Math.floor(d / 3600)} h ago`;
  if (d < 86400 * 7) return id ? `${Math.floor(d / 86400)} hr lalu` : `${Math.floor(d / 86400)} d ago`;
  return new Date(iso).toLocaleDateString(localeTag());
}

export function fmtDate(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = parseDay(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(localeTag(), { weekday: "short", day: "numeric", month: "short", year: "numeric" });
}

/** Compact date for setlist titles, e.g. "Oct 16" / "16 Okt". */
export function fmtDateShort(iso: string | Date): string {
  const d = typeof iso === "string" ? parseDay(iso) : iso;
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(localeTag(), { day: "numeric", month: "short" });
}

export function pluralize(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}
