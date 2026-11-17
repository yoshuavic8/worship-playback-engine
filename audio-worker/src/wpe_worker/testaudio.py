"""Synthetic, copyright-free test audio (tone, click, a structured 'worship-like' song).

The synthetic song has real musical structure so BPM / key / section detection can be tested:
Intro(4) Verse(8) Chorus(8) Verse(8) Chorus(8) Bridge(8) Chorus(8) Outro(4) bars, in A major.
"""

from __future__ import annotations

import numpy as np

SR = 48000
NOTE = {"A": 0, "B": 2, "C#": 4, "D": 5, "E": 7, "F#": 9, "G#": 11}


def _hz(semis_from_a4: float) -> float:
    return 440.0 * 2 ** (semis_from_a4 / 12)


def tone(freq=440.0, seconds=2.0, sr=SR, amp=0.25) -> np.ndarray:
    t = np.arange(int(seconds * sr)) / sr
    x = (np.sin(2 * np.pi * freq * t) * amp).astype(np.float32)
    return np.stack([x, x])


def click_track(bpm=120.0, bars=4, bpb=4, sr=SR) -> np.ndarray:
    n = int(bars * bpb * 60 / bpm * sr)
    out = np.zeros(n, dtype=np.float32)
    t = np.arange(int(0.05 * sr)) / sr
    for k in range(bars * bpb):
        f = 1760 if k % bpb == 0 else 1175
        c = np.sin(2 * np.pi * f * t) * np.exp(-t * 90)
        s = int(k * 60 / bpm * sr)
        out[s : s + len(c)] += c[: max(0, n - s)]
    return np.stack([out, out]) * 0.8


def _kick(sr):
    t = np.arange(int(0.25 * sr)) / sr
    f = 50 + 90 * np.exp(-t * 30)
    return np.sin(2 * np.pi * np.cumsum(f) / sr) * np.exp(-t * 12)


def _snare(sr, rng):
    t = np.arange(int(0.18 * sr)) / sr
    return (rng.standard_normal(len(t)) * 0.6 + np.sin(2 * np.pi * 190 * t) * 0.4) * np.exp(-t * 22)


def song(bpm=96.0, sr=SR, seed=7, return_stems=False):
    """Returns (mix (2,n), stems dict, sections [(label, start_seconds)])."""
    rng = np.random.default_rng(seed)
    beat = 60.0 / bpm
    bar = 4 * beat
    layout = [("Intro", 4, 0.35), ("Verse 1", 8, 0.6), ("Chorus", 8, 1.0), ("Verse 2", 8, 0.6),
              ("Chorus", 8, 1.0), ("Bridge", 8, 0.8), ("Chorus", 8, 1.0), ("Outro", 4, 0.3)]
    # chord progressions (root offsets from A, major triads) per section type
    prog = {"Intro": ["A", "E"], "Verse": ["A", "D", "A", "E"], "Chorus": ["D", "A", "E", "F#"],
            "Bridge": ["B", "D", "E", "E"], "Outro": ["A", "A"]}
    total_bars = sum(b for _, b, _ in layout)
    n = int(total_bars * bar * sr) + sr
    drums = np.zeros(n, np.float32)
    bass = np.zeros(n, np.float32)
    keys = np.zeros(n, np.float32)
    vox = np.zeros(n, np.float32)
    kick, sections = _kick(sr), []
    bar_i = 0
    for label, nbars, energy in layout:
        sections.append((label, bar_i * bar))
        kind = label.split()[0]
        chords = prog[kind]
        for b in range(nbars):
            t0 = (bar_i + b) * bar
            root = chords[b % len(chords)]
            r = NOTE[root]
            s0 = int(t0 * sr)
            seg = int(bar * sr)
            t = np.arange(seg) / sr
            env = np.minimum(1, t * 20) * np.minimum(1, (bar - t) * 20)
            # pad chord (major triad, two octaves below A4) – brighter in the chorus
            chord = sum(np.sin(2 * np.pi * _hz(r + iv - 12) * t) for iv in (0, 4, 7))
            keys[s0 : s0 + seg] += (chord * 0.12 * (0.5 + energy) * env).astype(np.float32)[: n - s0]
            if kind != "Intro" and kind != "Outro":
                # bass on every beat
                for k in range(4):
                    bs = int((t0 + k * beat) * sr)
                    bt = np.arange(int(beat * 0.9 * sr)) / sr
                    note = np.sin(2 * np.pi * _hz(r - 36) * bt) * np.exp(-bt * 3) * 0.35 * energy
                    bass[bs : bs + len(bt)] += note[: max(0, n - bs)]
                # drums: kick 1&3, snare 2&4, hats in chorus
                for k in range(4):
                    ds = int((t0 + k * beat) * sr)
                    if k in (0, 2):
                        drums[ds : ds + len(kick)] += (kick * 0.9 * energy)[: max(0, n - ds)]
                    else:
                        sn = _snare(sr, rng)
                        drums[ds : ds + len(sn)] += (sn * 0.5 * energy)[: max(0, n - ds)]
                # melody ("vocal") – sine with vibrato on chord tones
                mel = [r + 12, r + 16, r + 19, r + 16]
                for k in range(4):
                    vs = int((t0 + k * beat) * sr)
                    vt = np.arange(int(beat * sr)) / sr
                    f = _hz(mel[k] - 12) * (1 + 0.004 * np.sin(2 * np.pi * 5.5 * vt))
                    v = np.sin(2 * np.pi * np.cumsum(f) / sr) * np.minimum(1, vt * 30) * 0.15
                    vox[vs : vs + len(vt)] += v[: max(0, n - vs)]
        bar_i += nbars
    stems = {k: np.stack([v, v]).astype(np.float32) for k, v in
             {"vocals": vox, "drums": drums, "bass": bass, "other": keys}.items()}
    mix = sum(stems.values())
    peak = float(np.max(np.abs(mix)))
    if peak > 0.95:
        mix = mix / peak * 0.95
        stems = {k: v / peak * 0.95 for k, v in stems.items()}
    return (mix.astype(np.float32), stems, sections) if return_stems else (mix.astype(np.float32), sections)
