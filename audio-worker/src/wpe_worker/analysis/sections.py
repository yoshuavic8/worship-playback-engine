"""Section detection: bar-synchronous features → novelty boundaries → repetition clustering → naming.

Detection is probabilistic. The output is a starting point the MD corrects in the editor.
"""

from __future__ import annotations

import numpy as np


def _bar_times(duration: float, bpm: float | None, offset: float, bpb: int) -> np.ndarray:
    """Bar lines starting at Beat 1. Silence before Beat 1 is not bar 1."""
    if bpm and bpm > 0:
        bar = bpb * 60.0 / bpm
        start = max(0.0, float(offset))
        if start >= duration:
            return np.array([start])
        return np.arange(start, duration, bar)
    return np.arange(0.0, duration, 2.0)


PHRASE_BARS = 4


def _standardize(f: np.ndarray) -> np.ndarray:
    return (f - f.mean(axis=0)) / (f.std(axis=0) + 1e-6)


def _otsu(x: np.ndarray) -> float:
    """Two-class split of a 1-D sample (phrase-to-phrase distances: 'same section' vs 'boundary')."""
    v = np.sort(x)
    if len(v) < 2:
        return float(v[0]) if len(v) else 0.0
    best, thr = -1.0, float(v[-1])
    for i in range(1, len(v)):
        a, b = v[:i], v[i:]
        score = len(a) * len(b) * (a.mean() - b.mean()) ** 2
        if score > best:
            best, thr = score, float((v[i - 1] + v[i]) / 2)
    return thr


def boundaries_from_features(feat: np.ndarray, min_len: int = 1, max_sections: int = 14) -> list[int]:
    """Phrase-level boundaries: indices where a phrase differs clearly from the previous one.

    `feat` has one row per phrase. Returns phrase indices where sections start (always includes 0).
    """
    n = feat.shape[0]
    if n < 2:
        return [0]
    f = _standardize(feat)
    d = np.array([0.0] + [float(np.linalg.norm(f[i] - f[i - 1])) for i in range(1, n)])
    d /= np.sqrt(f.shape[1])
    thr = max(_otsu(np.log(d[1:] + 0.02)), np.log(0.2 + 0.02))
    cands = [i for i in range(1, n) if np.log(d[i] + 0.02) > thr]
    cands.sort(key=lambda i: d[i], reverse=True)
    chosen: list[int] = []
    for c in cands:
        if all(abs(c - x) >= min_len for x in chosen):
            chosen.append(c)
        if len(chosen) >= max_sections - 1:
            break
    return [0] + sorted(chosen)


def cluster_segments(seg_feats: np.ndarray, threshold: float = 0.25) -> list[int]:
    from scipy.cluster.hierarchy import fcluster, linkage

    if len(seg_feats) < 2:
        return [0] * len(seg_feats)
    x = seg_feats - seg_feats.mean(axis=0)
    x /= np.linalg.norm(x, axis=1, keepdims=True) + 1e-9
    z = linkage(x, method="average", metric="cosine")
    labels = fcluster(z, t=threshold, criterion="distance")
    # renumber in order of first appearance
    mapping: dict[int, int] = {}
    return [mapping.setdefault(int(l), len(mapping)) for l in labels]


def name_segments(clusters: list[int], energy: list[float], lengths: list[int]) -> list[tuple[str, str]]:
    """Heuristic worship-song naming. Returns [(label, kind)]."""
    n = len(clusters)
    if n == 0:
        return []
    if n == 1:
        return [("Song", "other")]
    e = np.asarray(energy, dtype=float)
    e = e / (e.max() + 1e-9)
    counts: dict[int, int] = {}
    for c in clusters:
        counts[c] = counts.get(c, 0) + 1
    repeating = [c for c, k in counts.items() if k >= 2]
    median = float(np.median(e))

    def mean_e(c):
        return float(np.mean([e[i] for i in range(n) if clusters[i] == c]))

    chorus = max(repeating, key=mean_e) if repeating else int(clusters[int(np.argmax(e))])
    verse_cands = [c for c in repeating if c != chorus]
    verse = min(verse_cands, key=lambda c: clusters.index(c)) if verse_cands else None

    kinds: list[str | None] = [None] * n
    first_chorus = next((i for i in range(n) if clusters[i] == chorus), n)
    for i, c in enumerate(clusters):
        if c == chorus:
            kinds[i] = "chorus"
        elif verse is not None and c == verse:
            kinds[i] = "verse"
    if kinds[0] != "chorus" and (e[0] < 0.8 * median or counts[clusters[0]] == 1 or lengths[0] <= 4):
        kinds[0] = "intro"
    last = n - 1
    if kinds[last] is None or (e[last] < 0.7 * median and lengths[last] <= 8):
        kinds[last] = "outro"
    for i in range(n):
        if kinds[i] is not None:
            continue
        nxt = kinds[i + 1] if i + 1 < n else None
        if i > first_chorus and counts[clusters[i]] == 1 and e[i] >= 0.45:
            kinds[i] = "bridge"
        elif nxt == "chorus" and lengths[i] <= 8 and i > 0:
            kinds[i] = "prechorus"
        elif e[i] < 0.45:
            kinds[i] = "instrumental"
        else:
            kinds[i] = "verse"

    names = {
        "intro": "Intro", "verse": "Verse", "prechorus": "Pre-Chorus", "chorus": "Chorus",
        "bridge": "Bridge", "instrumental": "Instrumental", "outro": "Outro",
    }
    out: list[tuple[str, str]] = []
    verse_n = 0
    for k in kinds:
        k = k or "verse"
        if k == "verse":
            verse_n += 1
            out.append((f"Verse {verse_n}", k))
        else:
            out.append((names.get(k, "Section"), k))
    return out


def detect_sections(y: np.ndarray, sr: int, bpm: float | None, beat_offset: float, bpb: int) -> list[dict]:
    import librosa

    if y.ndim > 1:
        y = y.mean(axis=0)
    duration = len(y) / sr
    bars = _bar_times(duration, bpm, beat_offset, bpb)
    if len(bars) < 6:
        start = float(bars[0]) if len(bars) else 0.0
        return [{"label": "Song", "kind": "other", "start": round(start, 3)}]
    hop = 2048
    chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop)
    mfcc = librosa.feature.mfcc(y=y, sr=sr, hop_length=hop, n_mfcc=13)
    rms = librosa.feature.rms(y=y, hop_length=hop)[0]
    frames = librosa.time_to_frames(np.append(bars, duration), sr=sr, hop_length=hop)
    frames = np.clip(frames, 0, chroma.shape[1])
    chroma_b, mfcc_b, energy = [], [], []
    for a, b in zip(frames[:-1], frames[1:]):
        b = max(b, a + 1)
        chroma_b.append(chroma[:, a:b].mean(axis=1))
        mfcc_b.append(mfcc[1:, a:b].mean(axis=1))
        energy.append(float(rms[a:b].mean()) if a < len(rms) else 0.0)
    chroma_b, mfcc_b = np.nan_to_num(np.asarray(chroma_b)), np.nan_to_num(np.asarray(mfcc_b))
    loud = np.log10(np.asarray(energy) + 1e-5)[:, None]

    # Phrases of 4 bars counted from Beat 1 (bars[0]), not from t = 0.
    # A lead-in must not shift every boundary by a bar.
    nb = len(bars)
    phrase_starts = list(range(0, nb, PHRASE_BARS))
    if nb - phrase_starts[-1] < 2 and len(phrase_starts) > 1:
        phrase_starts.pop()
    p_ends = phrase_starts[1:] + [nb]

    def block(s: int, e: int) -> np.ndarray:
        return np.concatenate([chroma_b[s:e].mean(0) * 1.5, mfcc_b[s:e].mean(0), np.repeat(loud[s:e].mean(0), 3)])

    phrases = np.asarray([block(s, e) for s, e in zip(phrase_starts, p_ends)])
    p_bounds = boundaries_from_features(phrases)
    starts = [phrase_starts[p] for p in p_bounds]
    ends = starts[1:] + [nb]
    seg_feats = np.asarray([block(s, e) for s, e in zip(starts, ends)])
    seg_energy = [float(np.mean(energy[s:e])) for s, e in zip(starts, ends)]
    clusters = cluster_segments(_standardize(seg_feats) if len(seg_feats) > 2 else seg_feats)
    named = name_segments(clusters, seg_energy, [e - s for s, e in zip(starts, ends)])
    return [
        {"label": label, "kind": kind, "start": round(float(bars[s]), 3)}
        for (label, kind), s in zip(named, starts)
    ]
