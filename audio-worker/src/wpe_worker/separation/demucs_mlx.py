"""Demucs on Apple's MLX framework (demucs-mlx). Weights are read from the app's model folder
(DEMUCS_MLX_CACHE_DIR) and downloads are disabled (DEMUCS_MLX_NO_DOWNLOAD=1)."""

from __future__ import annotations

import os
import threading
import time
from pathlib import Path

import numpy as np

from ..audio.io import resample
from ..protocol import WorkerError, log
from .base import AudioSeparator, ProgressFn, SeparationResult, normalize


class DemucsMLXSeparator(AudioSeparator):
    name = "demucs-mlx"

    def __init__(self, model: str, models_dir: str):
        self.model = model
        self.models_dir = Path(models_dir)

    def available(self) -> tuple[bool, str]:
        try:
            import mlx.core  # noqa: F401
            import demucs_mlx  # noqa: F401
        except ImportError as e:
            return False, f"demucs-mlx not installed ({e})"
        for f in (f"{self.model}.safetensors", f"{self.model}_config.json"):
            if not (self.models_dir / f).exists():
                return False, f"MLX weights for {self.model} not downloaded"
        return True, "ok"

    def separate(self, audio: np.ndarray, sample_rate: int, progress: ProgressFn) -> SeparationResult:
        os.environ["DEMUCS_MLX_CACHE_DIR"] = str(self.models_dir)
        os.environ["DEMUCS_MLX_NO_DOWNLOAD"] = "1"
        from demucs_mlx.api import Separator

        t0 = time.time()
        progress(0.0, f"Loading {self.model} (MLX)")
        sep = Separator(model=self.model, progress=False)
        sr = int(sep.samplerate)
        wav = resample(audio, sample_rate, sr)
        norm, mean, std = normalize(wav)
        seconds = wav.shape[-1] / sr
        # MLX has no progress callback: show an estimate (≈6–10× realtime on an M-series GPU).
        done = threading.Event()

        def ticker():
            est = max(4.0, seconds / 7.0)
            start = time.time()
            while not done.wait(0.5):
                frac = 1 - np.exp(-(time.time() - start) / est * 1.2)
                progress(min(0.97, float(frac)), "Separating (MLX, Apple GPU)")

        th = threading.Thread(target=ticker, daemon=True)
        th.start()
        log("device=mlx-gpu")
        try:
            _, stems = sep.separate_tensor(np.ascontiguousarray(norm, dtype=np.float32))
        except MemoryError as e:
            raise WorkerError("out_of_memory", "Out of memory during MLX separation", str(e)) from e
        finally:
            done.set()
            th.join(timeout=1)
        out = {k: (np.asarray(v, dtype=np.float32) * std + mean) for k, v in stems.items()}
        sep.close()
        return SeparationResult(out, sr, self.name, self.model, "mlx-gpu", time.time() - t0)
