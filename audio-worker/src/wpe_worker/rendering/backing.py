"""backing = other + adjusted bass + optional drums (+ vocals only if explicitly kept)."""

from __future__ import annotations

import numpy as np

from ..audio.io import db_to_gain
from ..protocol import StemSettings


def stem_gains(s: StemSettings) -> dict[str, float]:
    return {
        "other": db_to_gain(s.other_db),
        "bass": 0.0 if s.bass == "remove" else db_to_gain(s.bass_db),
        "drums": 1.0 if s.drums == "keep" else 0.0,
        "vocals": 1.0 if s.vocals == "keep" else 0.0,
    }


def _protect(x: np.ndarray, ceiling: float = 0.98) -> tuple[np.ndarray, float]:
    peak = float(np.max(np.abs(x))) if x.size else 0.0
    if peak > ceiling:
        g = ceiling / peak
        return x * g, 20 * np.log10(g)
    return x, 0.0


def render_backing(stems: dict[str, np.ndarray], s: StemSettings) -> tuple[np.ndarray, np.ndarray, dict]:
    n = max(v.shape[-1] for v in stems.values())
    acc = np.zeros((2, n), dtype=np.float32)
    backup = np.zeros((2, n), dtype=np.float32)
    gains = stem_gains(s)
    for name, audio in stems.items():
        a = audio if audio.shape[-1] == n else np.pad(audio, ((0, 0), (0, n - audio.shape[-1])))
        acc += a * gains.get(name, 0.0)
        if name != "vocals":
            backup += a
    acc, red_b = _protect(acc)
    backup, red_k = _protect(backup)
    return acc.astype(np.float32), backup.astype(np.float32), {
        "gains": gains,
        "limiterDbBacking": round(red_b, 2),
        "limiterDbBackup": round(red_k, 2),
    }
