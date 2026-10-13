import { useRef, useState } from "react";
import { Tip } from "@/components/ui";
import { tapBpm } from "@/lib/tapTempo";
import { cn } from "@/lib/format";

export function TapTempo({ onBpm }: { onBpm: (bpm: number) => void }) {
  const taps = useRef<number[]>([]);
  const [count, setCount] = useState(0);
  const tap = () => {
    const now = performance.now();
    const t = taps.current;
    if (t.length && now - t[t.length - 1] > 2000) t.length = 0;
    t.push(now);
    if (t.length > 9) t.shift();
    setCount(t.length);
    const bpm = tapBpm(t);
    if (bpm) onBpm(bpm);
  };
  return (
    <Tip label="Tap along with the beat (4+ taps)">
      <button type="button" onClick={tap} className={cn("h-9 shrink-0 rounded-lg border px-3 text-xs font-semibold", count > 1 ? "border-accent/50 text-accent" : "border-line text-fg-2 hover:border-line-2")}>
        TAP
      </button>
    </Tip>
  );
}
