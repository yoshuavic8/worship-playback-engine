"""BPM, constant-tempo beat grid and first downbeat estimation.

Worship recordings are almost always played to a click, so we fit a single constant tempo to the
tracked beats (robust linear regression). That yields a deterministic grid for the click engine.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

HOP = 512


@dataclass
class TempoResult:
    bpm: float | None
    confidence: float
    beat_offset: float
    beats: list[float]
    alternatives: list[float] = field(default_factory=list)


def _robust_fit(times: np.ndarray) -> tuple[float, float, np.ndarray]:
    idx = np.arange(len(times), dtype=np.float64)
    mask = np.ones(len(times), dtype=bool)
    slope, intercept = np.polyfit(idx, times, 1)
    for _ in range(4):
        resid = times - (slope * idx + intercept)
        mask = np.abs(resid) < max(0.06, 0.15 * slope)
        if mask.sum() < 8:
            break
        slope, intercept = np.polyfit(idx[mask], times[mask], 1)
    return float(slope), float(intercept), mask


def detect_tempo(y: np.ndarray, sr: int, beats_per_bar: int = 4) -> TempoResult:
    import librosa

    if y.ndim > 1:
        y = y.mean(axis=0)
    onset = librosa.onset.onset_strength(y=y, sr=sr, hop_length=HOP, aggregate=np.median)
    tempo, frames = librosa.beat.beat_track(
        onset_envelope=onset, sr=sr, hop_length=HOP, start_bpm=90.0, tightness=120, units="frames"
    )
    times = librosa.frames_to_time(frames, sr=sr, hop_length=HOP)
    tempo = float(np.atleast_1d(tempo)[0])
    if len(times) < 8 or tempo <= 0:
        return TempoResult(None, 0.0, 0.0, [])

    # Long recordings may drift slightly; fit on the dense middle 90 %.
    slope, intercept, mask = _robust_fit(times)
    bpm = 60.0 / slope
    while bpm < 58:
        bpm *= 2
    while bpm > 180:
        bpm /= 2
    phase = intercept % (60.0 / bpm)
    alternatives: list[float] = []
    # Half-tempo check: a backbeat (snare on 2 & 4) tracked at half speed lands exactly between
    # the tracked beats. If those off-beat onsets are nearly as strong, the real tempo is double.
    if bpm * 2 <= 180:
        p = 60.0 / bpm
        on_t = np.arange(phase, len(y) / sr - p, p)
        on_f = librosa.time_to_frames(on_t, sr=sr, hop_length=HOP)
        mid_f = librosa.time_to_frames(on_t + p / 2, sr=sr, hop_length=HOP)
        w = 2
        peak = lambda fr: np.mean([onset[max(0, f - w) : f + w + 1].max() for f in fr if f < len(onset)])  # noqa: E731
        on_s, mid_s = peak(on_f), peak(mid_f)
        ratio = mid_s / max(on_s, 1e-9)
        if ratio > 0.7 and (bpm < 80 or ratio > 0.85):
            bpm *= 2
        elif ratio > 0.3:
            alternatives.append(round(bpm * 2, 2))
    if bpm / 2 >= 58 and bpm > 140:
        alternatives.append(round(bpm / 2, 2))
    if abs(bpm - round(bpm)) < 0.2:
        bpm = float(round(bpm))
    period = 60.0 / bpm

    # confidence: share of tracked beats lying close to the fitted grid
    phase = intercept % period
    k = np.round((times - phase) / period)
    resid = np.abs(times - (phase + k * period))
    confidence = float(np.mean(resid < 0.05))

    # Downbeat: the beat phase in the bar with the strongest low-frequency onsets (kick / bass).
    mel = librosa.feature.melspectrogram(y=y, sr=sr, hop_length=HOP, n_mels=24, fmax=250)
    low = librosa.onset.onset_strength(S=librosa.power_to_db(mel), sr=sr, hop_length=HOP)
    grid_k = np.arange(0, int((len(y) / sr - phase) / period))
    grid_t = phase + grid_k * period
    grid_f = np.clip(librosa.time_to_frames(grid_t, sr=sr, hop_length=HOP), 0, len(low) - 1)
    # Chords usually change on the downbeat, kick is often on 1 *and* 3 — combine both cues.
    chroma = librosa.feature.chroma_stft(y=y, sr=sr, hop_length=HOP)
    flux = np.r_[0.0, np.linalg.norm(np.diff(chroma, axis=1), axis=0)]
    w = max(1, int(0.08 * sr / HOP))
    harm = np.array([flux[max(0, f - w) : f + w + 1].max() for f in grid_f])
    strength = low[grid_f]
    strength = strength / (strength.mean() + 1e-9) + 1.5 * harm / (harm.mean() + 1e-9)
    bpb = max(1, beats_per_bar)
    scores = [float(strength[j::bpb].mean()) if len(strength[j::bpb]) else 0.0 for j in range(bpb)]
    j = int(np.argmax(scores))
    first_downbeat = phase + j * period
    bar = bpb * period
    beat_offset = float(first_downbeat % bar)
    alternatives = [float(round(a)) if abs(a - round(a)) < 0.4 else a for a in alternatives]
    return TempoResult(round(bpm, 2), round(confidence, 3), round(beat_offset, 4), [float(t) for t in times], alternatives)
