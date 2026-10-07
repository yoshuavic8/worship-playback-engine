"""Musical key detection (Krumhansl–Schmuckler on averaged CQT chroma)."""

from __future__ import annotations

import numpy as np

MAJOR = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
MINOR = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])
# Spelling worship bands actually use.
MAJOR_NAMES = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"]
MINOR_NAMES = ["Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm"]


def key_from_chroma(chroma_mean: np.ndarray) -> tuple[str, float]:
    c = np.asarray(chroma_mean, dtype=np.float64)
    if c.sum() <= 0:
        return "C", 0.0
    scores = []
    for i in range(12):
        scores.append((np.corrcoef(np.roll(MAJOR, i), c)[0, 1], MAJOR_NAMES[i]))
        scores.append((np.corrcoef(np.roll(MINOR, i), c)[0, 1], MINOR_NAMES[i]))
    scores.sort(key=lambda s: s[0], reverse=True)
    best, second = scores[0][0], scores[1][0]
    conf = float(np.clip((best - second) * 5 + max(best, 0) * 0.5, 0, 1))
    return scores[0][1], round(conf, 3)


def detect_key(y: np.ndarray, sr: int) -> tuple[str, float]:
    import librosa

    if y.ndim > 1:
        y = y.mean(axis=0)
    chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=2048, bins_per_octave=36)
    # weight frames by energy so quiet intros do not dominate
    w = chroma.sum(axis=0)
    mean = (chroma * w).sum(axis=1) / max(w.sum(), 1e-9)
    return key_from_chroma(mean)
