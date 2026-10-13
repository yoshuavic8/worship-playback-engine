import type { Song } from "./types";

const TEMPO_WARNING = /bpm|tempo|uncertain|confidence|verify|drift|changing|grid/i;

/** Analysis warnings still worth showing. Tempo-related lines go stale after the user edits the map or when BPM no longer matches. */
export function visibleReviewWarnings(song: Song): { active: string[]; stale: string[] } {
  const all = (song.analysis?.warnings ?? []).filter(Boolean);
  if (!all.length) return { active: [], stale: [] };
  const userMap = song.grid?.source === "user";
  const active: string[] = [];
  const stale: string[] = [];
  for (const w of all) {
    if (userMap && TEMPO_WARNING.test(w)) {
      stale.push(w);
      continue;
    }
    const m = w.match(/(?:^|\b)(?:BPM\s*(\d+(?:\.\d+)?)|(\d+(?:\.\d+)?)\s*BPM)/i);
    const cited = m ? Number(m[1] ?? m[2]) : NaN;
    if (Number.isFinite(cited) && song.bpm != null && Math.abs(cited - song.bpm) > 2) {
      stale.push(w);
      continue;
    }
    active.push(w);
  }
  return { active, stale };
}
