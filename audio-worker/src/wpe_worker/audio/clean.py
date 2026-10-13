"""Light cleanup after stem separation.

Demucs can leave a steady whistle (one frequency, the whole song) on top of an otherwise
good stem. A musical note moves. A whistle does not, so only that kind of tone is notched.
A high-pass also removes the DC / rumble that reads as a low hum in the house.
"""

from __future__ import annotations

import numpy as np
from scipy.signal import butter, iirnotch, sosfiltfilt, tf2sos


def polish(audio: np.ndarray, sr: int) -> np.ndarray:
    x = np.ascontiguousarray(audio, dtype=np.float32)
    if x.size == 0 or sr < 8000 or x.shape[-1] < sr // 4:
        return x
    x = x - np.mean(x, axis=-1, keepdims=True)
    sos = butter(2, 20.0, btype="highpass", fs=sr, output="sos")
    x = sosfiltfilt(sos, x, axis=-1).astype(np.float32)
    return _notch_stationary(x, sr)


def _notch_stationary(x: np.ndarray, sr: int, max_notches: int = 2) -> np.ndarray:
    mono = np.asarray(x.mean(axis=0), dtype=np.float32)
    n_fft = 4096
    if mono.shape[-1] < n_fft * 4:
        return x
    hop = n_fft // 2
    window = np.hanning(n_fft).astype(np.float32)
    frames = 1 + (mono.shape[-1] - n_fft) // hop
    if frames < 8:
        return x
    spec = np.empty((frames, n_fft // 2 + 1), np.float32)
    for i in range(frames):
        s = i * hop
        spec[i] = np.abs(np.fft.rfft(mono[s : s + n_fft] * window))
    mean = spec.mean(axis=0)
    cv = spec.std(axis=0) / (mean + 1e-9)
    power = mean**2
    total = float(power.sum()) + 1e-12
    freqs = np.fft.rfftfreq(n_fft, 1.0 / sr)
    y = x
    used = 0
    for i in np.argsort(power)[::-1][:48]:
        f = float(freqs[i])
        if f < 200.0 or f > min(15000.0, sr * 0.45):
            continue
        if cv[i] > 0.22 or power[i] / total < 0.015:
            continue
        left = mean[max(0, i - 8) : max(0, i - 2)]
        right = mean[i + 2 : i + 8]
        band = np.concatenate([left, right]) if left.size or right.size else np.empty(0)
        if band.size < 4 or mean[i] < float(np.median(band)) * 6.0:
            continue
        b, a = iirnotch(f, Q=35.0, fs=sr)
        y = sosfiltfilt(tf2sos(b, a), y, axis=-1).astype(np.float32)
        used += 1
        if used >= max_notches:
            break
    return y
