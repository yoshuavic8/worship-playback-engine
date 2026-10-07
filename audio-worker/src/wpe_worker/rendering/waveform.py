"""Lightweight peak/RMS data (same format the Rust importer writes)."""

from __future__ import annotations

import numpy as np


def peaks(audio: np.ndarray, sr: int, per_second: int = 50) -> dict:
    mono = np.max(np.abs(audio), axis=0) if audio.ndim > 1 else np.abs(audio)
    per = max(1, sr // per_second)
    n = int(np.ceil(len(mono) / per))
    padded = np.pad(mono, (0, n * per - len(mono)))
    blocks = padded.reshape(n, per)
    pk = np.clip(blocks.max(axis=1), 0, 1)
    rms = np.clip(np.sqrt((blocks.astype(np.float64) ** 2).mean(axis=1)), 0, 1)
    return {
        "version": 1,
        "peaksPerSecond": per_second,
        "duration": len(mono) / sr,
        "peaks": np.round(pk * 255).astype(int).tolist(),
        "rms": np.round(rms * 255).astype(int).tolist(),
    }
