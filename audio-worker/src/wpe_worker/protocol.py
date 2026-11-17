"""JSON-lines protocol (stdout). See shared/schemas/worker-event.schema.json."""

from __future__ import annotations

import json
import sys
import threading
from dataclasses import dataclass, field
from typing import Any

_lock = threading.Lock()


def _json_default(o: Any) -> Any:
    """NumPy scalars and arrays are not JSON-serializable. Keep them as numbers."""
    if type(o).__module__ == "numpy":
        shape = getattr(o, "shape", None)
        if shape == ():
            return o.item()
        return o.tolist()
    raise TypeError(f"Object of type {type(o).__name__} is not JSON serializable")


def dumps(obj: Any) -> str:
    return json.dumps(obj, default=_json_default)


def emit(event: str, **payload: Any) -> None:
    line = dumps({"event": event, **payload})
    with _lock:
        sys.stdout.write(line + "\n")
        sys.stdout.flush()


def progress(stage: str, value: float, message: str = "") -> None:
    emit("progress", stage=stage, progress=round(max(0.0, min(1.0, value)), 4), message=message)


def log(message: str, level: str = "info") -> None:
    emit("log", level=level, message=message)


class WorkerError(Exception):
    def __init__(self, code: str, message: str, details: str | None = None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details


@dataclass
class StemSettings:
    vocals: str = "remove"
    drums: str = "remove"
    bass: str = "keep"
    bass_db: float = -6.0
    other_db: float = 0.0


@dataclass
class SeparationConfig:
    enabled: bool = True
    model: str = "htdemucs"
    engine: str = "auto"
    installed_engines: list[str] = field(default_factory=list)
    device: str = "auto"
    models_dir: str = ""
    mlx_models_dir: str = ""
    # worship = Inst HQ 3 vocals, then Demucs on the instrumental. fast = Demucs only.
    profile: str = "worship"
    mdx_models_dir: str = ""


@dataclass
class PrepareRequest:
    song_id: str
    input: str
    song_dir: str
    ffmpeg: str
    sample_rate: int = 48000
    settings: StemSettings = field(default_factory=StemSettings)
    separation: SeparationConfig = field(default_factory=SeparationConfig)
    analysis_enabled: bool = True
    time_signature: str = "4/4"

    @staticmethod
    def parse(d: dict) -> "PrepareRequest":
        if d.get("type") != "prepare_song":
            raise WorkerError("bad_request", f"unsupported request type {d.get('type')!r}")
        for key in ("songId", "input", "songDir"):
            if not isinstance(d.get(key), str) or not d[key]:
                raise WorkerError("bad_request", f"missing field {key}")
        s = d.get("settings") or {}
        sep = d.get("separation") or {}
        an = d.get("analysis") or {}
        mode = lambda v, default: v if v in ("remove", "keep", "optional") else default  # noqa: E731
        profile = str(sep.get("profile") or "worship")
        if profile not in ("worship", "fast"):
            profile = "worship"
        return PrepareRequest(
            song_id=d["songId"],
            input=d["input"],
            song_dir=d["songDir"],
            ffmpeg=d.get("ffmpeg") or "ffmpeg",
            sample_rate=int(d.get("sampleRate") or 48000),
            settings=StemSettings(
                vocals=mode(s.get("vocals"), "remove"),
                drums=mode(s.get("drums"), "remove"),
                bass=mode(s.get("bass"), "keep"),
                bass_db=float(s.get("bassDb", -6.0)),
                other_db=float(s.get("otherDb", 0.0)),
            ),
            separation=SeparationConfig(
                enabled=bool(sep.get("enabled", True)),
                model=str(sep.get("model") or "htdemucs"),
                engine=str(sep.get("engine") or "auto"),
                installed_engines=list(sep.get("installedEngines") or []),
                device=str(sep.get("device") or "auto"),
                models_dir=str(sep.get("modelsDir") or ""),
                mlx_models_dir=str(sep.get("mlxModelsDir") or ""),
                profile=profile,
                mdx_models_dir=str(sep.get("mdxModelsDir") or ""),
            ),
            analysis_enabled=bool(an.get("enabled", True)),
            time_signature=str(an.get("timeSignature") or "4/4"),
        )
