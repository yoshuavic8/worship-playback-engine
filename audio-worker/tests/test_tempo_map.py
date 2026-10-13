"""Tempo-map math shared with TypeScript and Rust, plus conservative seeding."""

import json
import random
from pathlib import Path

from wpe_worker.analysis.tempo_map import beat_to_time, bpm_at_beat, propose_grid, q_of, time_to_beat

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = json.loads((ROOT / "fixtures" / "tempo_map_vectors.json").read_text())


def _grid(case):
    return {
        "beat1Sec": case["beat1Sec"],
        "beatsPerBar": case["beatsPerBar"],
        "points": case["points"],
    }


def test_golden_vectors():
    for case in FIXTURE.values():
        g = _grid(case)
        for e in case["expect"]:
            q = e.get("qNeg", q_of(e["bar"], e["beat"], g["beatsPerBar"]))
            t = beat_to_time(g, q)
            assert abs(t - e["time"]) < 0.0001
            assert abs(bpm_at_beat(g, q) - e["bpm"]) < 0.0001
            assert abs(time_to_beat(g, t) - q) < 1e-6


def test_bar_45_is_one_sample():
    g = _grid(FIXTURE["mockup"])
    frame = round(beat_to_time(g, q_of(45, 1, 4)) * 48000)
    assert abs(frame - 7_187_585) <= 1


def _times(segments, offset=0.0):
    """segments: list of (beat_count, bpm or (bpm0, bpm1) for a linear ramp)."""
    t = offset
    out = [t]
    for beats, tempo in segments:
        if isinstance(tempo, tuple):
            v0, v1 = tempo
            for i in range(beats):
                bpm = v0 + (v1 - v0) * (i / beats)
                t += 60.0 / bpm
                out.append(t)
        else:
            step = 60.0 / tempo
            for _ in range(beats):
                t += step
                out.append(t)
    return out


def test_noise_stays_one_point():
    random.seed(1)
    t = 0.2
    times = [t]
    for _ in range(160):
        t += 60.0 / (120 + random.uniform(-0.4, 0.4))
        times.append(t)
    grid = propose_grid(120, 0.2, 4, times)
    assert len(grid["points"]) == 1


def test_step_change_adds_one_point():
    # 16 bars at 72, then 16 bars at 80. The change is bar 17.
    times = _times([(64, 72), (64, 80)])
    grid = propose_grid(72, 0.0, 4, times)
    assert len(grid["points"]) == 2
    assert grid["points"][1]["bar"] == 17
    assert grid["points"][1]["curve"] == "step"
    assert abs(grid["points"][1]["bpm"] - 80) < 1.5


def test_ritardando_is_one_ramp():
    # 8 bars at 76, then 8 bars gliding 76 → 66, then 8 bars at 66.
    times = _times([(32, 76), (32, (76, 66)), (32, 66)])
    grid = propose_grid(76, 0.0, 4, times)
    curves = [p["curve"] for p in grid["points"]]
    assert "linear" in curves
    assert len(grid["points"]) <= 4
