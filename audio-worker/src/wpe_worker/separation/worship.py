"""Worship profile: Inst HQ 3 removes the vocal, Demucs splits the remaining band.

Demucs runs on the instrumental, not the original mix. Live recordings often leave
the lead vocal inside Demucs' "other" stem; taking the vocal out first keeps that
bleed out of the backing the band hears.
"""

from __future__ import annotations

import gc
import time

import numpy as np

from ..audio.io import fit_length, resample
from ..protocol import SeparationConfig, log
from .base import STEMS, AudioSeparator, ProgressFn, SeparationResult
from .mdxnet import MDXNetSeparator


class WorshipSeparator(AudioSeparator):
    name = "worship"

    def __init__(self, cfg: SeparationConfig):
        self.cfg = cfg
        self.mdx = MDXNetSeparator(cfg.mdx_models_dir)

    def available(self) -> tuple[bool, str]:
        ok, why = self.mdx.available()
        if not ok:
            return False, why
        from .registry import demucs_candidates

        if not demucs_candidates(self.cfg):
            return False, "No Demucs model is installed for drums, bass and other"
        return True, "ok"

    def separate(self, audio: np.ndarray, sample_rate: int, progress: ProgressFn) -> SeparationResult:
        t0 = time.time()
        n = audio.shape[-1]
        vocal = self.mdx.separate(audio, sample_rate, lambda f, m: progress(0.45 * f, m))
        # Drop the ONNX session before Demucs allocates the GPU.
        gc.collect()
        instrumental = fit_length(resample(vocal.stems["instrumental"], vocal.sample_rate, sample_rate), n)
        vocals = fit_length(resample(vocal.stems["vocals"], vocal.sample_rate, sample_rate), n)

        from .registry import demucs_candidates

        demucs = demucs_candidates(self.cfg)[0]
        progress(0.45, f"Splitting instruments with {demucs.name}")
        band = demucs.separate(instrumental, sample_rate, lambda f, m: progress(0.45 + 0.55 * f, m))
        stems: dict[str, np.ndarray] = {"vocals": vocals}
        for name in ("drums", "bass", "other"):
            piece = band.stems.get(name)
            if piece is None:
                raise RuntimeError(f"{demucs.name} did not return a {name} stem")
            stems[name] = fit_length(resample(piece, band.sample_rate, sample_rate), n)
        missing = [name for name in STEMS if name not in stems]
        if missing:
            raise RuntimeError(f"worship separation missing {missing}")
        device = f"{vocal.device}+{band.device}"
        log(f"device={device}")
        log(f"worship separation: vocals {vocal.model}/{vocal.device}, instruments {band.model}/{band.device}")
        return SeparationResult(
            stems,
            sample_rate,
            self.name,
            f"{vocal.model}+{band.model}",
            device,
            time.time() - t0,
            notes=[
                "vocals: UVR-MDX-NET Inst HQ 3",
                f"drums/bass/other: {band.engine} {band.model} on the instrumental",
            ],
        )
