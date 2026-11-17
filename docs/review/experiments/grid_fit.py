"""Prototype: constant-tempo grid fit + bar phase + beat-1 anchoring (librosa only).
Not production code — demonstrates the proposed architecture on the same synthetic cases."""
import numpy as np, librosa

HOP = 256

def _interp(o, t, sr):
    f = t * sr / HOP
    i = np.clip(f.astype(int), 0, len(o) - 2); a = f - i
    return o[i] * (1 - a) + o[i + 1] * a

def fit_grid(y, sr, bpb=4, lo=60.0, hi=180.0):
    o = librosa.onset.onset_strength(y=y, sr=sr, hop_length=HOP)
    o = o / (o.max() + 1e-9)
    dur = len(y) / sr
    # 1. coarse tempo from global autocorrelation (period-level, not beat sequence)
    ac = librosa.autocorrelate(o - o.mean())
    lags = np.arange(len(ac)); bpms = 60 * sr / HOP / np.maximum(lags, 1)
    ok = (bpms >= lo / 2) & (bpms <= hi)
    lag = lags[ok][np.argmax(ac[ok])]; T = 60 * sr / HOP / lag
    while T < lo: T *= 2
    while T >= hi: T /= 2
    cands = [c for c in (T, T * 2, T / 2) if lo <= c < hi]

    def comb(P, phase):
        g = np.arange(phase, dur, P); m = np.arange(phase + P / 2, dur, P)
        return _interp(o, g, sr).mean() / (_interp(o, m, sr).mean() + 1e-9)

    def best_phase(P):
        ph = np.arange(0, P, 0.004)
        s = [_interp(o, np.arange(p, dur, P), sr).sum() for p in ph]
        return ph[int(np.argmax(s))]

    scored = []
    for c in cands:
        # coarse tempo is lag-quantised (~0.3 %): search period ±0.8 % before judging the octave
        best = None
        for Pc in (60 / c) * (1 + np.linspace(-0.008, 0.008, 33)):
            p = best_phase(Pc); sc = _interp(o, np.arange(p, dur, Pc), sr).mean()
            if best is None or sc > best[0]: best = (sc, Pc, p)
        _, Pc, p = best
        scored.append((comb(Pc, p), 60 / Pc, p))
    scored.sort(reverse=True)
    # keep the slower tempo unless the faster one is clearly stronger (ballad protection)
    by_bpm = sorted(scored, key=lambda x: x[1])          # slowest first
    pick = by_bpm[0]
    for cand in by_bpm[1:]:
        if cand[0] >= pick[0] * 1.25: pick = cand
    _, bpm, phase = pick
    P = 60 / bpm
    # 2. refine period+phase by regression of onset peaks on INTEGER beat index (robust to missed beats)
    for _ in range(3):
        k = np.arange(0, int((dur - phase) / P))
        pred = phase + k * P
        w = max(1, int(0.12 * P * sr / HOP))
        tk, wk, kk = [], [], []
        for kj, t in zip(k, pred):
            f = int(t * sr / HOP); a, b = max(0, f - w), min(len(o), f + w + 1)
            if b - a < 3: continue
            j = a + int(np.argmax(o[a:b]))
            if o[j] < 0.15: continue          # silent / weak beat → no evidence, skip (no index shift!)
            if 0 < j < len(o) - 1:            # parabolic sub-frame refinement
                d = (o[j - 1] - o[j + 1]) / (2 * (o[j - 1] - 2 * o[j] + o[j + 1]) + 1e-12)
            else: d = 0
            tk.append((j + np.clip(d, -0.5, 0.5)) * HOP / sr); wk.append(o[j]); kk.append(kj)
        tk, wk, kk = map(np.asarray, (tk, wk, kk))
        A = np.vstack([np.ones_like(kk), kk]).T.astype(float)
        for _h in range(5):  # Huber-ish IRLS
            sol = np.linalg.lstsq(A * np.sqrt(wk)[:, None], tk * np.sqrt(wk), rcond=None)[0]
            r = tk - A @ sol; s = 1.4826 * np.median(np.abs(r)) + 1e-4
            wk = wk * np.minimum(1, 1.5 * s / (np.abs(r) + 1e-9))
        phase, P = sol[0] % sol[1], sol[1]
    resid_ms = float(np.median(np.abs(r)) * 1000)
    bpm = 60 / P
    # 3. bar phase: low-band onsets + harmonic change per beat position
    mel = librosa.feature.melspectrogram(y=y, sr=sr, hop_length=HOP, n_mels=24, fmax=250)
    low = librosa.onset.onset_strength(S=librosa.power_to_db(mel), sr=sr, hop_length=HOP)
    chroma = librosa.feature.chroma_stft(y=y, sr=sr, hop_length=HOP)
    flux = np.r_[0, np.linalg.norm(np.diff(chroma, axis=1), axis=0)]
    k = np.arange(0, int((dur - phase) / P)); g = phase + k * P
    sc = _interp(low, g, sr) / (low.mean() + 1e-9) + 1.5 * _interp(flux, g, sr) / (flux.mean() + 1e-9)
    pos = np.array([sc[j::bpb].mean() for j in range(bpb)])
    j = int(np.argmax(pos)); db_conf = float((pos.max() - np.sort(pos)[-2]) / (pos.max() + 1e-9))
    # 4. beat 1 = first bar line at/after the first sustained musical energy (any instrument, not kick-only)
    rms = librosa.feature.rms(y=y, hop_length=HOP)[0]
    rdb = librosa.amplitude_to_db(rms, ref=rms.max())
    on = np.where(rdb > -30)[0]; music_start = on[0] * HOP / sr if len(on) else 0.0
    bars = phase + j * P + np.arange(-2, int(dur / (bpb * P)) + 2) * bpb * P
    beat1 = float(bars[bars >= music_start - 0.5 * P][0])
    return dict(bpm=round(bpm, 3), beat1=round(beat1, 4), resid_ms=round(resid_ms, 1),
                downbeat_conf=round(db_conf, 2), music_start=round(float(music_start), 3),
                alternatives=[round(c, 2) for _, c, _ in scored[1:]])
