/** Average interval between taps → BPM (needs at least 4 taps). */
export function tapBpm(times: number[]): number | null {
  if (times.length < 4) return null;
  const iv = times.slice(1).map((t, i) => t - times[i]);
  const avg = iv.reduce((a, b) => a + b, 0) / iv.length;
  if (avg <= 0) return null;
  return Math.round((60000 / avg) * 10) / 10;
}
