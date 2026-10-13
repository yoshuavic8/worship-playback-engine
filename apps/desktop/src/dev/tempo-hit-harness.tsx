import { useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import * as TooltipP from "@radix-ui/react-tooltip";
import { SongTimeline, type SnapMode } from "@/components/SongTimeline";
import { constantGrid, type Grid } from "@/lib/tempoMap";
import type { Song } from "@/lib/types";
import "../styles.css";

const prep = { presetId: null, vocals: "remove" as const, drums: "remove" as const, bass: "keep" as const, bassDb: 0, otherDb: 0, click: true, cue: true };
const grid = constantGrid(120, 2, 4, "user");

const initial: Song = {
  id: "hit", title: "Hit Test", artist: "Test", bpm: 120, key: "C", timeSignature: "4/4", beatOffset: 2,
  duration: 60, sampleRate: 48000, channels: 2, dir: "/tmp/hit", sourcePath: "source.wav",
  stems: { vocals: null, drums: null, bass: null, other: null },
  rendered: { backing: null, backup: null },
  status: "ready", prep, routing: { backingToFoh: true, backingToMonitor: true, guideVocalToMonitor: false, clickEnabled: true, cueEnabled: true },
  sections: [{ id: "intro", label: "Intro", kind: "intro", start: 0 }],
  arrangement: [], countInBars: 0, cueLeadBeats: 4, notes: "", lastError: null,
  analysis: { tempoMode: "changing", bpmConfidence: 0.4, downbeatConfidence: 0.4 },
  grid, createdAt: "", updatedAt: "", preparedAt: null,
};

function applyGrid(song: Song, next: Grid): Song {
  const first = [...next.points].sort((a, b) => a.bar - b.bar || a.beat - b.beat)[0];
  return { ...song, grid: next, bpm: first?.bpm ?? song.bpm, beatOffset: next.beat1Sec };
}

function Harness() {
  const scrollerRef = useRef<HTMLDivElement>(null);
  const [song, setSong] = useState(initial);
  const [snap, setSnap] = useState<SnapMode>("beat");
  const noop = () => {};
  return (
    <TooltipP.Provider>
    <div className="flex h-full" data-testid="beat" data-beat={song.beatOffset.toFixed(3)}>
      <SongTimeline
        song={song}
        sections={[{ id: "intro", label: "Intro", kind: "intro", start: 0, end: song.duration }]}
        wave={null}
        playhead={0}
        rolling={false}
        loaded={false}
        looping={false}
        loopRegion={null}
        zoom={1}
        scrollerRef={scrollerRef}
        selectedId={null}
        canUndo={false}
        canRedo={false}
        snap={snap}
        saving={false}
        dirty={false}
        tracks={[]}
        clickOn
        cueOn={false}
        preparing={false}
        activeSectionId={null}
        solo={null}
        tempoPoint={null}
        drawTempo={false}
        actions={{
          onBack: noop, onTitle: noop, onArtist: noop, onPlay: noop, onStop: noop, onLoop: noop,
          onSeek: noop, onSelect: noop, onAddSection: noop, onMoveSection: noop, onRename: noop,
          onRenameFocus: noop, onKind: noop, onDelete: noop,
          onMoveBeat: (t) => setSong((s) => ({ ...s, beatOffset: t, grid: s.grid ? { ...s.grid, beat1Sec: t } : s.grid })),
          onBpm: noop, onKey: noop, onTimeSignature: noop, onCountIn: noop, onUndo: noop, onRedo: noop,
          onTempoStart: noop,
          onTempo: (g) => setSong((s) => applyGrid(s, g)),
          onTempoEnd: (g) => setSong((s) => applyGrid(s, g)),
          onTempoPoint: noop, onToggleTempoCurve: noop,
          onSnap: () => setSnap((m) => (m === "bar" ? "beat" : m === "beat" ? "off" : "bar")),
          onSnapAll: noop, onSave: noop, onRevert: noop, onDone: noop, onPrepare: noop, onMix: noop,
          onMute: noop, onSolo: noop, onClickToggle: noop, onCueToggle: noop, onZoom: noop,
        }}
      />
    </div>
    </TooltipP.Provider>
  );
}

createRoot(document.getElementById("root")!).render(<Harness />);
