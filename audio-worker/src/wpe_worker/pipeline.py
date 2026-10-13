"""Preparation pipeline.

validate → FFmpeg normalise → stem separation → bass adjustment / backing render → BPM / beat grid
→ key → sections → waveform → quality control → song package
"""

from __future__ import annotations

import time
from pathlib import Path

import numpy as np

from .analysis.key import detect_key
from .analysis.sections import detect_sections
from .analysis.tempo import detect_tempo
from .analysis.tempo_map import propose_grid
from .audio import io
from .audio.clean import polish
from .protocol import PrepareRequest, WorkerError, dumps, emit, log, progress
from .rendering.backing import render_backing
from .rendering.waveform import peaks
from .separation.base import STEMS
from .separation.registry import candidates

# stage weights for the overall progress bar
W = {"validate": 0.02, "normalize": 0.05, "separate": 0.68, "render": 0.07, "analyze": 0.13, "waveform": 0.02, "qc": 0.03}


class Progress:
    def __init__(self):
        self.base = 0.0

    def stage(self, name: str, frac: float, msg: str):
        progress(name, self.base + W[name] * frac, msg)

    def done(self, name: str):
        self.base += W[name]


def run(req: PrepareRequest) -> dict:
    t_all = time.time()
    timings: dict[str, float] = {}
    warnings: list[str] = []
    p = Progress()
    song = Path(req.song_dir)

    p.stage("validate", 0, "Validating file")
    src = Path(req.input)
    if not src.is_file():
        raise WorkerError("decode_failed", f"Input not found: {src}")
    p.done("validate")

    t = time.time()
    p.stage("normalize", 0, "Converting to 48 kHz float (FFmpeg)")
    norm_path = song / "source" / "normalized.wav"
    io.normalize_with_ffmpeg(req.ffmpeg, str(src), str(norm_path), req.sample_rate)
    mix, sr = io.read(str(norm_path))
    if mix.shape[-1] < sr:
        raise WorkerError("decode_failed", "Audio is shorter than one second")
    duration = mix.shape[-1] / sr
    timings["normalize"] = time.time() - t
    p.done("normalize")

    result: dict = {"stems": {}, "rendered": {}, "warnings": warnings}
    stems: dict[str, np.ndarray] | None = None
    if req.separation.enabled:
        t = time.time()
        sep_list = candidates(req.separation)
        if not sep_list:
            raise WorkerError("model_missing", "No separation engine with installed model is available")
        if req.separation.profile == "worship" and sep_list[0].name != "worship":
            warnings.append(
                "UVR-MDX-NET Inst HQ 3 is not installed, so vocal removal used Demucs only. "
                "Download it in Diagnostics → AI Models for live recordings."
            )
        last_error: Exception | None = None
        for sep in sep_list:
            try:
                p.stage("separate", 0, f"Separating with {sep.name}")
                res = sep.separate(mix, sr, lambda f, m: p.stage("separate", f, m))
                break
            except WorkerError:
                raise
            except Exception as e:  # noqa: BLE001 — fall back to the next engine
                last_error = e
                log(f"{sep.name} failed: {e!r} — trying next engine", "warn")
                warnings.append(f"{sep.name} failed, used fallback engine")
        else:
            raise WorkerError("separation_failed", str(last_error), repr(last_error))
        stems = {}
        for name in STEMS:
            a = res.stems.get(name)
            if a is None:
                continue
            a = polish(io.fit_length(io.resample(a, res.sample_rate, sr), mix.shape[-1]), sr)
            stems[name] = a
            rel = f"stems/{name}.wav"
            io.write(str(song / rel), a, sr)
            result["stems"][name] = rel
        result["separator"] = {
            "engine": res.engine, "model": res.model, "device": res.device,
            "seconds": round(res.seconds, 2), "realtimeFactor": round(duration / max(res.seconds, 1e-3), 1),
        }
        timings["separate"] = time.time() - t
        log(f"separated with {res.engine}/{res.device} in {res.seconds:.1f}s ({duration / max(res.seconds, 1e-3):.1f}x realtime)")
    p.done("separate")

    t = time.time()
    p.stage("render", 0, "Rendering backing track")
    if stems and all(k in stems for k in ("drums", "bass", "other")):
        backing, backup, info = render_backing(stems, req.settings)
        io.write(str(song / "rendered/backing.wav"), backing, sr)
        io.write(str(song / "rendered/backup.wav"), backup, sr)
        result["rendered"] = {"backing": "rendered/backing.wav", "backup": "rendered/backup.wav"}
        result["render"] = info
        if info["limiterDbBacking"] < -0.5:
            warnings.append(f"Backing was {abs(info['limiterDbBacking']):.1f} dB too hot and was turned down")
    else:
        # Analysis-only preparation: the backup is the normalised original (vocals included).
        io.write(str(song / "rendered/backup.wav"), mix, sr)
        result["rendered"] = {"backup": "rendered/backup.wav"}
        if req.separation.enabled:
            warnings.append("Separation produced no usable stems")
    timings["render"] = time.time() - t
    p.done("render")

    analysis = None
    if req.analysis_enabled:
        t = time.time()
        bpb = int(req.time_signature.split("/")[0]) if "/" in req.time_signature else 4
        p.stage("analyze", 0.05, "Detecting BPM & beat grid")
        mono = mix.mean(axis=0)
        # analysis at 22.05 kHz is plenty and much faster
        y = io.resample(mono[None], sr, 22050)[0]
        tempo = detect_tempo(y, 22050, bpb)
        p.stage("analyze", 0.4, "Detecting key")
        harmonic = None
        if stems and "other" in stems and "bass" in stems:
            harmonic = io.resample((stems["other"] + stems["bass"]).mean(axis=0)[None], sr, 22050)[0]
        key, key_conf = detect_key(harmonic if harmonic is not None else y, 22050)
        p.stage("analyze", 0.6, "Detecting sections")
        sections = detect_sections(y, 22050, tempo.bpm, tempo.beat_offset, bpb)

        # Build the full analysis dict — all new fields flow through the free-form
        # analysis_json column in SQLite so NO schema migration is required.
        analysis = {
            # --- existing fields (consumed by Rust prep.rs / player.rs) ---
            "bpm": tempo.bpm,
            "bpmConfidence": tempo.confidence,
            "beatOffset": tempo.beat_offset,
            "bpmAlternatives": tempo.alternatives,
            "key": key,
            "keyConfidence": key_conf,
            "timeSignature": req.time_signature,
            "duration": round(duration, 3),
            "sections": sections,

            # --- new timeline fields ---
            "audioStart": tempo.audio_start,
            "musicalStart": tempo.musical_start,

            # --- tempo stability / mode ---
            "tempoMode": tempo.tempo_mode,         # "fixed" | "drifting" | "changing"
            "stability": tempo.stability,           # 0–1
            "downbeatConfidence": tempo.downbeat_confidence,

            # --- TempoMap (only non-empty when tempoMode != "fixed") ---
            "tempoMap": [
                {
                    "startBeat": r.start_beat,
                    "startTime": r.start_time,
                    "bpm": r.bpm,
                    "timeSignature": r.time_signature,
                }
                for r in tempo.tempo_map
            ],

            # --- beat grid lock (user must confirm before click follows grid) ---
            "beatGridLocked": tempo.beat_grid_locked,
            "grid": propose_grid(tempo.bpm, tempo.beat_offset, bpb, tempo.beats),
        }

        # --- Generate user warnings based on the richer data we now have ---
        if tempo.bpm is None:
            warnings.append(
                "BPM could not be detected — set the tempo manually so the click can follow the song"
            )
        elif tempo.tempo_mode == "changing":
            warnings.append(
                f"Intentional tempo change detected — this recording has {len(tempo.tempo_map)} tempo "
                f"regions. Verify the downbeat and tempo map in the editor."
            )
        elif tempo.tempo_mode == "drifting":
            warnings.append(
                f"Live tempo drift detected (stability {tempo.stability:.0%}) — "
                f"this recording may be from a live performance without a click track. "
                f"Verify the downbeat: play the song and use Set Downbeat at the first beat."
            )
        elif tempo.confidence < 0.35:
            warnings.append(
                f"BPM {tempo.bpm:g} is uncertain — listen and correct the tempo if the click does not match"
            )

        # Warn if we had to skip over a long pre-music section
        if tempo.musical_start > 2.0:
            warnings.append(
                f"Music starts at {tempo.musical_start:.1f}s — Beat 1 is the first bar line there. "
                f"The audio before that may be silence or a spoken introduction. Please verify."
            )

        (song / "analysis").mkdir(parents=True, exist_ok=True)
        # Write full analysis including explicit beat timestamps for the click engine
        (song / "analysis" / "analysis.json").write_text(
            dumps({**analysis, "beats": tempo.beats})
        )
        timings["analyze"] = time.time() - t
    result["analysis"] = analysis
    p.done("analyze")

    p.stage("waveform", 0, "Generating waveform")
    wf_src = None
    if result["rendered"].get("backing"):
        wf_src, _ = io.read(str(song / result["rendered"]["backing"]))
    (song / "analysis").mkdir(parents=True, exist_ok=True)
    (song / "analysis" / "waveform.json").write_text(dumps(peaks(wf_src if wf_src is not None else mix, sr)))
    p.done("waveform")

    p.stage("qc", 0, "Quality control")
    for kind, rel in {**result["stems"], **result["rendered"]}.items():
        a, r = io.read(str(song / rel))
        if r != req.sample_rate:
            warnings.append(f"{kind} is {r} Hz")
        if not np.isfinite(a).all():
            raise WorkerError("separation_failed", f"{kind} contains invalid samples")
        if abs(a.shape[-1] / r - duration) > 0.5:
            warnings.append(f"{kind} length differs from the source")
    if result["rendered"].get("backing"):
        b, _ = io.read(str(song / result["rendered"]["backing"]))
        if float(np.sqrt(np.mean(b**2))) < 1e-4:
            warnings.append("Backing track is almost silent — check the preset")
    p.done("qc")

    timings["total"] = time.time() - t_all
    result["timings"] = {k: round(v, 2) for k, v in timings.items()}
    progress("done", 1.0, "Live-ready")
    return result


def run_and_emit(req: PrepareRequest) -> int:
    try:
        result = run(req)
        emit("result", **result)
        return 0
    except WorkerError as e:
        emit("error", code=e.code, message=e.message, details=e.details)
        return 2
    except MemoryError as e:
        emit("error", code="out_of_memory", message="Out of memory", details=repr(e))
        return 3
    except Exception as e:  # noqa: BLE001
        import traceback

        emit("error", code="separation_failed", message=str(e) or repr(e), details=traceback.format_exc())
        return 1
