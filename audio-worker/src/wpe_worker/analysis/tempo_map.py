"""Tempo map shared with the desktop app and the Rust engine.

Time is linear. Bars get wider when the tempo slows. Audio is never stretched.
A constant-tempo song is a map with one step point at bar 1 beat 1.
"""

from __future__ import annotations

import math
from typing import Any

# Instantaneous BPM below this difference is treated as a step, not a ramp.
_FLAT = 1e-9


def q_of(bar: float, beat: float, beats_per_bar: int) -> float:
    bpb = max(1, int(beats_per_bar))
    return (bar - 1.0) * bpb + (beat - 1.0)


def bar_beat(q: float, beats_per_bar: int) -> tuple[int, int]:
    bpb = max(1, int(beats_per_bar))
    # Floor toward -inf so a pre-roll beat stays in the previous bar.
    bar_index = math.floor(q / bpb)
    beat0 = int(round(q - bar_index * bpb))
    if beat0 >= bpb:
        beat0 = 0
        bar_index += 1
    if beat0 < 0:
        beat0 += bpb
        bar_index -= 1
    return bar_index + 1, beat0 + 1


def _knots(grid: dict[str, Any]) -> list[dict[str, float | str]]:
    bpb = max(1, int(grid.get("beatsPerBar") or 4))
    raw = list(grid.get("points") or [])
    raw.sort(key=lambda p: q_of(float(p.get("bar") or 1), float(p.get("beat") or 1), bpb))
    if not raw:
        raw = [{"bar": 1, "beat": 1, "bpm": 120.0, "curve": "step"}]
    knots: list[dict[str, float | str]] = []
    t = float(grid.get("beat1Sec") or 0.0)
    for i, p in enumerate(raw):
        q = 0.0 if i == 0 else q_of(float(p.get("bar") or 1), float(p.get("beat") or 1), bpb)
        bpm = max(1e-6, float(p.get("bpm") or 120.0))
        curve = "linear" if p.get("curve") == "linear" else "step"
        if i > 0:
            prev = knots[-1]
            t = _time_across(float(prev["t"]), float(prev["q"]), float(prev["bpm"]), q, bpm, str(prev["curve"]))
        knots.append({"q": q, "t": t, "bpm": bpm, "curve": curve})
    return knots


def _time_across(t0: float, q0: float, v0: float, q1: float, v1: float, curve: str) -> float:
    span = q1 - q0
    if span <= 0:
        return t0
    if curve != "linear" or abs(v1 - v0) < _FLAT:
        return t0 + 60.0 * span / v0
    return t0 + 60.0 * span / (v1 - v0) * math.log(v1 / v0)


def _segment(knots: list[dict[str, float | str]], q: float) -> tuple[int, bool]:
    """Return (index of the knot that owns q, True when q is past the last knot)."""
    if q <= float(knots[0]["q"]):
        return 0, False
    for i in range(len(knots) - 1):
        if float(knots[i]["q"]) <= q < float(knots[i + 1]["q"]):
            return i, False
    return len(knots) - 1, True


def bpm_at_beat(grid: dict[str, Any], q: float) -> float:
    knots = _knots(grid)
    i, past = _segment(knots, q)
    v0 = float(knots[i]["bpm"])
    if past or i + 1 >= len(knots) or knots[i]["curve"] != "linear":
        return v0
    v1 = float(knots[i + 1]["bpm"])
    q0 = float(knots[i]["q"])
    span = float(knots[i + 1]["q"]) - q0
    if span <= 0 or abs(v1 - v0) < _FLAT:
        return v0
    return v0 + (v1 - v0) * (q - q0) / span


def beat_to_time(grid: dict[str, Any], q: float) -> float:
    knots = _knots(grid)
    i, past = _segment(knots, q)
    q0 = float(knots[i]["q"])
    t0 = float(knots[i]["t"])
    v0 = float(knots[i]["bpm"])
    if past or i + 1 >= len(knots) or q <= q0:
        return t0 + 60.0 * (q - q0) / v0
    v1 = float(knots[i + 1]["bpm"])
    q1 = float(knots[i + 1]["q"])
    span = q1 - q0
    if knots[i]["curve"] != "linear" or abs(v1 - v0) < _FLAT or span <= 0:
        return t0 + 60.0 * (q - q0) / v0
    bpm = v0 + (v1 - v0) * (q - q0) / span
    if bpm <= 0 or v0 <= 0:
        return t0 + 60.0 * (q - q0) / v0
    return t0 + 60.0 * span / (v1 - v0) * math.log(bpm / v0)


def time_to_beat(grid: dict[str, Any], t: float) -> float:
    knots = _knots(grid)
    if t <= float(knots[0]["t"]):
        v0 = float(knots[0]["bpm"])
        return float(knots[0]["q"]) + (t - float(knots[0]["t"])) * v0 / 60.0
    for i in range(len(knots) - 1):
        t0 = float(knots[i]["t"])
        t1 = float(knots[i + 1]["t"])
        if t0 <= t < t1 or i == len(knots) - 2 and t <= t1:
            q0 = float(knots[i]["q"])
            q1 = float(knots[i + 1]["q"])
            v0 = float(knots[i]["bpm"])
            v1 = float(knots[i + 1]["bpm"])
            span = q1 - q0
            if t >= t1:
                return q1
            if knots[i]["curve"] != "linear" or abs(v1 - v0) < _FLAT or span <= 0:
                return q0 + (t - t0) * v0 / 60.0
            bpm = v0 * math.exp((t - t0) * (v1 - v0) / (60.0 * span))
            return q0 + span * (bpm - v0) / (v1 - v0)
    last = knots[-1]
    return float(last["q"]) + (t - float(last["t"])) * float(last["bpm"]) / 60.0


def bpm_at_time(grid: dict[str, Any], t: float) -> float:
    return bpm_at_beat(grid, time_to_beat(grid, t))


def constant_grid(bpm: float, beat1_sec: float, beats_per_bar: int, source: str = "analysis") -> dict[str, Any]:
    return {
        "beat1Sec": round(float(beat1_sec), 4),
        "beatsPerBar": max(1, int(beats_per_bar)),
        "points": [
            {
                "id": "p1",
                "bar": 1,
                "beat": 1,
                "bpm": round(float(bpm), 3),
                "curve": "step",
            }
        ],
        "source": source,
        "version": 1,
    }


def _window_bpms(beat_times: list[float], beat1: float, bpb: int, window_bars: int = 4) -> list[tuple[int, float]]:
    """Median BPM of each `window_bars` block, keyed by the bar the block starts on."""
    if len(beat_times) < bpb * 2:
        return []
    times = sorted(beat_times)
    # Index of the tracked beat closest to Beat 1. That beat is q = 0.
    anchor = min(range(len(times)), key=lambda i: abs(times[i] - beat1))
    window = max(1, window_bars) * bpb
    out: list[tuple[int, float]] = []
    start = anchor
    while start + window < len(times):
        intervals = [times[i + 1] - times[i] for i in range(start, start + window) if times[i + 1] > times[i]]
        if len(intervals) >= window // 2:
            intervals.sort()
            mid = intervals[len(intervals) // 2]
            if mid > 1e-4:
                bar = (start - anchor) // bpb + 1
                out.append((bar, 60.0 / mid))
        start += window
    return out


def propose_grid(
    bpm: float | None,
    beat1_sec: float,
    beats_per_bar: int,
    beat_times: list[float] | None = None,
) -> dict[str, Any]:
    """One step point, plus a change only when a 4-bar window earns it.

    A step is added when neighbouring windows differ by at least 1 BPM, each
    side covers at least 4 bars, and the map's beat-interval error drops by
    40% or more. A steady slope of at least 2 BPM becomes one ramp instead.
    At most 8 points. Anything noisier stays a single tempo.
    """
    bpb = max(1, int(beats_per_bar))
    base = float(bpm or 0.0)
    if not (20.0 <= base <= 300.0):
        if beat_times and len(beat_times) >= 2:
            gaps = sorted(b - a for a, b in zip(beat_times, beat_times[1:]) if b > a)
            base = 60.0 / gaps[len(gaps) // 2] if gaps else 120.0
        else:
            base = 120.0
    grid = constant_grid(base, beat1_sec, bpb, "analysis")
    windows = _window_bpms(list(beat_times or []), beat1_sec, bpb)
    if len(windows) < 2:
        return grid

    # Candidate boundaries where the 4-bar median actually moves.
    cuts: list[tuple[int, float, float]] = []
    for (bar_a, bpm_a), (bar_b, bpm_b) in zip(windows, windows[1:]):
        if bar_b - bar_a < 4:
            continue
        if abs(bpm_b - bpm_a) >= 1.0:
            cuts.append((bar_b, bpm_a, bpm_b))
    if not cuts:
        return grid

    points = [dict(grid["points"][0])]
    i = 0
    while i < len(cuts) and len(points) < 8:
        bar, left, right = cuts[i]
        # Absorb a run of windows that keep moving the same direction into one ramp.
        j = i
        while j + 1 < len(cuts) and (cuts[j + 1][2] - cuts[j + 1][1]) * (right - left) > 0:
            j += 1
        end_bar, _, end_bpm = cuts[j]
        span_bars = end_bar - bar
        delta = end_bpm - left
        if j > i and abs(delta) >= 2.0 and span_bars >= 4:
            points.append({"id": f"p{len(points) + 1}", "bar": int(bar), "beat": 1, "bpm": round(left, 3), "curve": "linear"})
            points.append({"id": f"p{len(points) + 1}", "bar": int(end_bar), "beat": 1, "bpm": round(end_bpm, 3), "curve": "step"})
            i = j + 1
            continue
        points.append({"id": f"p{len(points) + 1}", "bar": int(bar), "beat": 1, "bpm": round(right, 3), "curve": "step"})
        i += 1

    proposed = {**grid, "points": points}
    if not beat_times or not _residual_improved(beat_times, beat1_sec, bpb, base, proposed):
        return grid
    return proposed


def _residual_improved(
    beat_times: list[float], beat1: float, bpb: int, constant_bpm: float, proposed: dict[str, Any]
) -> bool:
    times = sorted(beat_times)
    anchor = min(range(len(times)), key=lambda i: abs(times[i] - beat1))
    const = constant_grid(constant_bpm, beat1, bpb)

    def err(g: dict[str, Any]) -> float:
        acc = 0.0
        n = 0
        for i, t in enumerate(times):
            q = float(i - anchor)
            acc += (t - beat_to_time(g, q)) ** 2
            n += 1
        return math.sqrt(acc / max(1, n))

    before = err(const)
    after = err(proposed)
    if before < 1e-6:
        return False
    return (before - after) / before >= 0.40
