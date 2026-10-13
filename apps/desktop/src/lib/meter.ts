/** Meters the band actually plays. Anything else already stored on a song still displays. */
export const METERS = ["4/4", "3/4", "2/4", "6/8"] as const;

export type Meter = (typeof METERS)[number];

export function beatsPerBar(timeSignature: string): number {
  const n = Number(timeSignature.split("/")[0]);
  return Number.isFinite(n) && n > 0 ? Math.round(n) : 4;
}

export function meterChoices(current: string): string[] {
  return (METERS as readonly string[]).includes(current) ? [...METERS] : [current, ...METERS];
}

/** How the click groups this meter. Tempo stays the click speed; the meter only sets the bar. */
export function meterHint(timeSignature: string): string {
  const n = beatsPerBar(timeSignature);
  if (timeSignature === "6/8") return "Six clicks in the bar. Beat 1 and beat 4 are accented.";
  if (n === 3) return "Three clicks in the bar. Beat 1 is accented.";
  if (n === 2) return "Two clicks in the bar. Beat 1 is accented.";
  return "Four clicks in the bar. Beat 1 is accented.";
}
