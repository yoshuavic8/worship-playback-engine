"""Audio I/O. FFmpeg decodes/normalises the source once; everything after is lossless float WAV."""

from __future__ import annotations

import subprocess
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

from ..protocol import WorkerError


def normalize_with_ffmpeg(ffmpeg: str, src: str, dst: str, sample_rate: int = 48000) -> None:
    """Decode any input (MP3/WAV/FLAC/M4A/AIFF) to 32-bit float stereo WAV at `sample_rate`."""
    Path(dst).parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        ffmpeg, "-hide_banner", "-nostdin", "-y", "-loglevel", "error",
        "-i", src, "-map", "0:a:0", "-vn",
        "-ac", "2", "-ar", str(sample_rate), "-c:a", "pcm_f32le", dst,
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    except FileNotFoundError as e:
        raise WorkerError("ffmpeg_missing", "FFmpeg not found", str(e)) from e
    if r.returncode != 0:
        raise WorkerError("decode_failed", "FFmpeg could not decode the file", r.stderr[-2000:])


def read(path: str) -> tuple[np.ndarray, int]:
    """Returns (channels, samples) float32 and the sample rate."""
    data, sr = sf.read(path, dtype="float32", always_2d=True)
    return np.ascontiguousarray(data.T), int(sr)


def write(path: str, audio: np.ndarray, sr: int) -> None:
    """Write (channels, samples) float32 WAV."""
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    a = np.asarray(audio, dtype=np.float32)
    if a.ndim == 1:
        a = np.stack([a, a])
    sf.write(path, a.T, sr, subtype="FLOAT")


def resample(audio: np.ndarray, sr_from: int, sr_to: int) -> np.ndarray:
    if sr_from == sr_to:
        return audio
    from math import gcd

    g = gcd(sr_from, sr_to)
    return resample_poly(audio, sr_to // g, sr_from // g, axis=-1).astype(np.float32)


def fit_length(audio: np.ndarray, n: int) -> np.ndarray:
    if audio.shape[-1] == n:
        return audio
    if audio.shape[-1] > n:
        return audio[..., :n]
    pad = np.zeros(audio.shape[:-1] + (n - audio.shape[-1],), dtype=audio.dtype)
    return np.concatenate([audio, pad], axis=-1)


def db_to_gain(db: float) -> float:
    return 0.0 if db <= -96 else float(10 ** (db / 20))
