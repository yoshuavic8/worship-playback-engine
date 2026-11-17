"""BPM, beat grid, downbeat estimation, and tempo stability analysis.

For studio recordings a single constant tempo is usually sufficient.
For live worship recordings — the most common source of problems — this module
does the extra work to:

  1. Detect where audio *actually* begins (audioStart).
  2. Detect where the first *musical* event begins, after crowd noise / WL speech (musicalStart).
  3. Fit a robust linear grid to the tracked beats and assess stability.
  4. Classify the recording as "fixed", "drifting" (live tempo drift), or "changing"
     (intentional mid-song tempo change) so the engine can warn the user.
  5. For non-fixed tempos, produce a TempoMap of (startBeat, startTime, bpm) regions
     instead of forcing a single BPM.

The click engine NEVER reads BPM + audioStart and assumes that is beat 1.
It must consume the explicit beat timestamps or TempoMap that this module provides.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

HOP = 512


# ---------------------------------------------------------------------------
# Public result type
# ---------------------------------------------------------------------------

@dataclass
class TempoRegion:
    start_beat: int        # 1-indexed beat number where this region begins
    start_time: float      # timestamp of that beat in the audio (seconds)
    bpm: float
    time_signature: str = "4/4"


@dataclass
class TempoResult:
    # Core (always set on success)
    bpm: float | None
    confidence: float            # 0–1: fraction of tracked beats on the grid
    beat_offset: float           # time of first downbeat (seconds) — authoritative anchor
    beats: list[float]           # explicit beat timestamps from the tracker

    # Alternate BPM candidates
    alternatives: list[float] = field(default_factory=list)

    # Timeline
    audio_start: float = 0.0    # first non-silent frame (seconds)
    musical_start: float = 0.0  # first onset that looks like an instrument (seconds)

    # Stability / mode
    tempo_mode: str = "fixed"   # "fixed" | "drifting" | "changing"
    stability: float = 1.0      # 0–1, higher = more stable
    downbeat_confidence: float = 0.0

    # TempoMap (only populated when tempo_mode != "fixed")
    tempo_map: list[TempoRegion] = field(default_factory=list)

    # Beat-grid lock state (False until user confirms)
    beat_grid_locked: bool = False


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

def _contrast(onset: np.ndarray, sr: int, bpm: float, phase: float) -> float:
    """How much stronger onsets are on the beat than halfway between beats.

    Eighth-note energy in a slow worship song is often high. That must not be
    enough on its own to decide the click should run at double time.
    """
    import librosa

    period = 60.0 / bpm
    dur = len(onset) * HOP / sr
    on_t = np.arange(phase, max(phase, dur - period), period)
    if len(on_t) < 8:
        return 1.0
    on_f = librosa.time_to_frames(on_t, sr=sr, hop_length=HOP)
    mid_f = librosa.time_to_frames(on_t + period / 2, sr=sr, hop_length=HOP)
    w = 2

    def peak(fr: np.ndarray) -> float:
        vals = [onset[max(0, int(f) - w) : int(f) + w + 1].max() for f in fr if 0 <= int(f) < len(onset)]
        return float(np.mean(vals)) if vals else 0.0

    return peak(on_f) / (peak(mid_f) + 1e-9)


def _robust_fit(times: np.ndarray) -> tuple[float, float, np.ndarray]:
    """Iteratively re-weighted least-squares: fit a line, reject outliers, repeat."""
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


def _beats_for_period(times: np.ndarray, tracked_period: float, period: float) -> list[float]:
    """Match the beat list to the BPM we actually chose.

    The tracker often locks at half tempo. Doubling the BPM number without
    doubling these timestamps makes a locked click tick every other beat.
    Inserted beats sit evenly between tracked ones. A halved tempo keeps the
    phase that already lies on the slower grid.
    """
    raw = [float(t) for t in times]
    if len(times) < 2 or tracked_period <= 0 or period <= 0:
        return raw
    factor = tracked_period / period
    copies = int(round(factor))
    if copies >= 2 and abs(factor - copies) < 0.25:
        out: list[float] = []
        for i in range(len(times) - 1):
            a = float(times[i])
            b = float(times[i + 1])
            for k in range(copies):
                out.append(a + (b - a) * k / copies)
        out.append(float(times[-1]))
        return out
    if factor < 0.75:
        step = max(2, int(round(1.0 / factor)))
        phase = float(times[0])
        best_off, best_err = 0, 1e9
        for off in range(step):
            sel = times[off::step]
            if len(sel) < 2:
                continue
            slots = (sel - phase) / period
            err = float(np.median(np.abs(slots - np.round(slots))))
            if err < best_err:
                best_off, best_err = off, err
        return [float(t) for t in times[best_off::step]]
    return raw


# ---------------------------------------------------------------------------
# New: onset / silence utilities
# ---------------------------------------------------------------------------

def _detect_audio_start(y: np.ndarray, sr: int) -> float:
    """Return the time (seconds) of the first non-silent frame.

    We look for the first frame where the onset strength is above 3 % of the
    track peak — this eliminates DC offsets and very soft room noise while still
    catching a gentle guitar strum.
    """
    import librosa

    onset = librosa.onset.onset_strength(y=y, sr=sr, hop_length=HOP)
    threshold = onset.max() * 0.03
    frames = np.where(onset > threshold)[0]
    if len(frames) == 0:
        return 0.0
    return float(librosa.frames_to_time(frames[0], sr=sr, hop_length=HOP))


def _detect_musical_start(y: np.ndarray, sr: int, audio_start: float) -> float:
    """First energy of any instrument, measured from the peak.

    A pad, piano, or guitar intro has no kick. Requiring a low-frequency hit
    skips that intro and parks Beat 1 on the verse. Same rule as the grid-fit
    prototype: the first frame whose RMS is within 30 dB of the peak.
    """
    import librosa

    rms = librosa.feature.rms(y=y, hop_length=HOP)[0]
    if rms.size == 0 or float(np.max(rms)) <= 0:
        return audio_start
    db = librosa.amplitude_to_db(rms, ref=float(np.max(rms)))
    start_frame = min(int(max(audio_start, 0.0) * sr / HOP), max(0, len(db) - 1))
    on = np.where(db[start_frame:] > -30.0)[0]
    if len(on) == 0:
        return audio_start
    return float((start_frame + int(on[0])) * HOP / sr)


# ---------------------------------------------------------------------------
# New: stability and tempo-mode classification
# ---------------------------------------------------------------------------

def _classify_stability(times: np.ndarray, slope: float) -> tuple[str, float, list[TempoRegion]]:
    """Classify whether the tempo is fixed, drifting, or changing.

    Returns (mode, stability_score, tempo_regions).

    Definitions:
      fixed    — all beat intervals within ±3 % of the median (studio / click-based live)
      drifting — gradual, monotonic change (live drummer without click)
      changing — one or more step-changes of >4 BPM (intentional key / meter changes,
                 or producer editing artifact)
    """
    if len(times) < 12:
        return "fixed", 1.0, []

    intervals = np.diff(times)
    med = np.median(intervals)
    if med <= 0:
        return "fixed", 1.0, []

    # Coefficient of variation (std / mean) in beat intervals
    cv = float(np.std(intervals) / med)

    if cv < 0.03:
        # Very stable — almost certainly studio or tight live take
        return "fixed", float(1.0 - cv * 10), []

    # Look for step-changes: a jump larger than ~4 BPM between consecutive intervals
    # 4 BPM at 72 BPM → Δinterval ≈ 0.046 s
    diffs = np.abs(np.diff(intervals))
    jump_threshold = max(0.035, 0.05 * med)   # ~3 BPM jump
    jumps = np.where(diffs > jump_threshold)[0]

    if len(jumps) > 0:
        # Build tempo regions around the jumps
        regions = _build_tempo_regions(times, jumps)
        return "changing", max(0.1, 1.0 - cv * 3), regions

    # Gradual drift
    stability = float(max(0.0, 1.0 - cv * 15))
    return "drifting", stability, []


def _build_tempo_regions(times: np.ndarray, jump_indices: np.ndarray) -> list[TempoRegion]:
    """Given beat timestamps and indices where tempo jumps, build TempoRegion list."""
    regions: list[TempoRegion] = []
    # np.where indices are numpy.int64. JSON cannot encode those, so boundaries
    # are plain Python ints before they become startBeat.
    jumps = [int(i) for i in np.asarray(jump_indices).reshape(-1)]
    boundaries = [0, *[i + 1 for i in jumps], len(times)]

    for a, b in zip(boundaries[:-1], boundaries[1:]):
        segment = times[a:b]
        if len(segment) < 2:
            continue
        intervals = np.diff(segment)
        bpm = round(60.0 / float(np.median(intervals)), 3)
        regions.append(TempoRegion(
            start_beat=int(a) + 1,  # 1-indexed
            start_time=float(times[a]),
            bpm=float(bpm),
        ))
    return regions


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------

def detect_tempo(y: np.ndarray, sr: int, beats_per_bar: int = 4) -> TempoResult:
    import librosa

    if y.ndim > 1:
        y = y.mean(axis=0)

    # -----------------------------------------------------------------------
    # Step 1: Detect where audio / music actually starts.
    # We do this BEFORE beat tracking so we can report it to the UI.
    # -----------------------------------------------------------------------
    audio_start = _detect_audio_start(y, sr)
    musical_start = _detect_musical_start(y, sr, audio_start)

    # -----------------------------------------------------------------------
    # Step 2: Beat tracking on the full signal (librosa is robust enough that
    # silence / speech at the start does not usually throw it off, but our
    # downbeat correction below will catch problems).
    # -----------------------------------------------------------------------
    onset = librosa.onset.onset_strength(y=y, sr=sr, hop_length=HOP, aggregate=np.median)
    tempo, frames = librosa.beat.beat_track(
        onset_envelope=onset, sr=sr, hop_length=HOP, start_bpm=90.0, tightness=120, units="frames"
    )
    times = librosa.frames_to_time(frames, sr=sr, hop_length=HOP)
    tempo = float(np.atleast_1d(tempo)[0])

    if len(times) < 8 or tempo <= 0:
        return TempoResult(
            None, 0.0, 0.0, [],
            audio_start=audio_start,
            musical_start=musical_start,
            tempo_mode="fixed",
            stability=0.0,
            downbeat_confidence=0.0,
        )

    # -----------------------------------------------------------------------
    # Step 3: Robust linear fit to reject outliers and get a clean grid.
    # Long recordings may drift slightly; fit on dense middle 90%.
    # -----------------------------------------------------------------------
    slope, intercept, mask = _robust_fit(times)
    tracked_period = slope
    bpm = 60.0 / slope

    # Keep BPM in a musical range
    while bpm < 58:
        bpm *= 2
    while bpm > 180:
        bpm /= 2

    # Octave selection: stay with tracked tempo unless doubling/halving locks
    # the onsets much more strongly (protects against 2× error on slow ballads).
    base = bpm
    candidates = [base]
    if 50.0 <= base * 2 <= 180.0:
        candidates.append(base * 2)
    if 50.0 <= base / 2 <= 180.0:
        candidates.append(base / 2)
    scores = {c: _contrast(onset, sr, c, intercept % (60.0 / c)) for c in candidates}
    best = max(scores, key=scores.get)
    if best != base and scores[best] >= scores[base] * 1.25:
        bpm = best
    # Never snap to an integer. 72.15 stored as 72 drifts about half a beat
    # by the end of a three-minute song.
    alternatives = [round(float(c), 3) for c in candidates if abs(c - bpm) > 0.5]
    period = 60.0 / bpm

    # -----------------------------------------------------------------------
    # Step 4: Confidence — fraction of tracked beats lying close to the grid.
    # -----------------------------------------------------------------------
    phase = intercept % period
    k = np.round((times - phase) / period)
    resid = np.abs(times - (phase + k * period))
    confidence = float(np.mean(resid < max(0.04, 0.10 * period)))

    # -----------------------------------------------------------------------
    # Step 5: Downbeat detection (first bar-line position).
    #
    # CRITICAL: the first audible event in a live recording is NOT necessarily
    # beat 1. We use low-frequency energy (kick + bass) and harmonic flux
    # (chord changes) to find the most likely downbeat phase.
    #
    # Additionally, we never let the detected downbeat sit BEFORE musicalStart.
    # If librosa picks up crowd noise or a spoken word as the beat anchor, we
    # advance the grid until we are past musicalStart.
    # -----------------------------------------------------------------------
    mel = librosa.feature.melspectrogram(y=y, sr=sr, hop_length=HOP, n_mels=24, fmax=250)
    low = librosa.onset.onset_strength(S=librosa.power_to_db(mel), sr=sr, hop_length=HOP)
    grid_k = np.arange(0, int((len(y) / sr - phase) / period))
    grid_t = phase + grid_k * period
    grid_f = np.clip(librosa.time_to_frames(grid_t, sr=sr, hop_length=HOP), 0, len(low) - 1)

    chroma = librosa.feature.chroma_stft(y=y, sr=sr, hop_length=HOP)
    flux = np.r_[0.0, np.linalg.norm(np.diff(chroma, axis=1), axis=0)]
    w = max(1, int(0.08 * sr / HOP))
    harm = np.array([flux[max(0, f - w) : f + w + 1].max() for f in grid_f])
    strength = low[grid_f]
    strength = strength / (strength.mean() + 1e-9) + 1.5 * harm / (harm.mean() + 1e-9)
    bpb = max(1, beats_per_bar)
    phase_scores = [float(strength[j::bpb].mean()) if len(strength[j::bpb]) else 0.0 for j in range(bpb)]

    # Prefer the earliest strong phase (avoids flip to beat 3 when kick is on 1 and 3)
    best_s = max(phase_scores) if phase_scores else 0.0
    close = [i for i, s in enumerate(phase_scores) if s >= best_s * 0.92]
    j = min(close) if close else int(np.argmax(phase_scores))
    first_downbeat = phase + j * period
    downbeat_confidence = float(best_s / (np.mean(phase_scores) + 1e-9) - 1.0)
    downbeat_confidence = float(np.clip(downbeat_confidence / 3.0, 0.0, 1.0))

    # Beat 1 is the first bar line at or just before the first musical energy
    # (half a beat of slack, so a one-beat pickup still belongs to that bar).
    duration = len(y) / sr
    bar = bpb * period
    grid = first_downbeat + np.arange(-8, int(duration / bar) + 8) * bar
    eligible = grid[(grid >= musical_start - 0.5 * period) & (grid < duration)]
    if len(eligible):
        beat_offset = float(eligible[0])
    else:
        beat_offset = float(max(0.0, first_downbeat))
    if beat_offset < 0:
        beat_offset += bar * float(np.ceil(-beat_offset / bar))

    # -----------------------------------------------------------------------
    # Step 6: Stability analysis.
    # Build a cleaned list of beat timestamps relative to the fitted grid,
    # then classify: fixed / drifting / changing.
    # -----------------------------------------------------------------------
    # Reconstruct beat times from the original tracked beats (not the fitted grid)
    # so we capture real tempo variation.
    good_times = times[mask] if mask.sum() >= 8 else times
    tempo_mode, stability, tempo_map = _classify_stability(good_times, slope)

    return TempoResult(
        bpm=round(float(bpm), 3),
        confidence=round(confidence, 3),
        beat_offset=round(beat_offset, 4),
        beats=_beats_for_period(times, tracked_period, period),
        alternatives=alternatives,
        audio_start=round(audio_start, 3),
        musical_start=round(musical_start, 3),
        tempo_mode=tempo_mode,
        stability=round(stability, 3),
        downbeat_confidence=round(downbeat_confidence, 3),
        tempo_map=tempo_map,
        beat_grid_locked=False,
    )
