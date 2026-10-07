"""Separator selection. Order (Apple Silicon): MLX → Demucs/MPS → Demucs/CPU, unless configured."""

from __future__ import annotations

from ..protocol import SeparationConfig
from .base import AudioSeparator
from .demucs_mlx import DemucsMLXSeparator
from .demucs_torch import DemucsSeparator


def candidates(cfg: SeparationConfig) -> list[AudioSeparator]:
    mlx = DemucsMLXSeparator(cfg.model, cfg.mlx_models_dir)
    torch_auto = DemucsSeparator(cfg.model, cfg.models_dir, cfg.device)
    torch_cpu = DemucsSeparator(cfg.model, cfg.models_dir, "cpu")
    if cfg.engine == "demucs":
        order: list[AudioSeparator] = [torch_auto, torch_cpu]
    elif cfg.engine == "demucs-mlx":
        order = [mlx, torch_auto, torch_cpu]
    else:
        order = [mlx, torch_auto, torch_cpu]
    out: list[AudioSeparator] = []
    for s in order:
        ok, _ = s.available()
        if ok and not any(type(o) is type(s) and getattr(o, "device", None) == getattr(s, "device", None) for o in out):
            out.append(s)
    return out


def describe(cfg: SeparationConfig) -> list[dict]:
    rows = []
    for s in (DemucsMLXSeparator(cfg.model, cfg.mlx_models_dir), DemucsSeparator(cfg.model, cfg.models_dir, cfg.device)):
        ok, why = s.available()
        rows.append({"engine": s.name, "available": ok, "reason": why})
    return rows
