"""Official Demucs (PyTorch). Uses the Apple GPU through MPS when available, else CPU."""

from __future__ import annotations

import time
from pathlib import Path
from types import SimpleNamespace

import numpy as np

from ..audio.io import resample
from ..protocol import WorkerError, log
from .base import AudioSeparator, ProgressFn, SeparationResult, normalize

MODEL_FILES = {
    "htdemucs": ["955717e8-8726e21a.th"],
    "htdemucs_ft": ["f7e0c4bc-ba3fe64a.th", "d12395a8-e57c48e6.th", "92cfc3b6-ef3bcb9c.th", "04573f0d-f3cf25b2.th"],
}


class DemucsSeparator(AudioSeparator):
    name = "demucs"

    def __init__(self, model: str, models_dir: str, device: str = "auto"):
        self.model = model
        self.models_dir = Path(models_dir)
        self.device = device

    def available(self) -> tuple[bool, str]:
        try:
            import demucs  # noqa: F401
            import torch  # noqa: F401
        except ImportError as e:
            return False, f"demucs/torch not installed ({e})"
        files = MODEL_FILES.get(self.model, [])
        if not files or not all((self.models_dir / f).exists() for f in files):
            return False, f"model {self.model} not downloaded"
        if not (self.models_dir / f"{self.model}.yaml").exists():
            return False, f"{self.model}.yaml missing"
        return True, "ok"

    def resolve_device(self) -> str:
        import torch

        if self.device == "cpu":
            return "cpu"
        if torch.backends.mps.is_available():
            return "mps"
        return "cpu"

    def separate(self, audio: np.ndarray, sample_rate: int, progress: ProgressFn) -> SeparationResult:
        import torch
        import demucs.apply as dapply
        from demucs.apply import BagOfModels, apply_model
        from demucs.pretrained import get_model

        t0 = time.time()
        progress(0.0, f"Loading {self.model}")
        model = get_model(self.model, repo=self.models_dir)
        model.eval()
        sr = int(model.samplerate)
        wav = resample(audio, sample_rate, sr)
        norm, mean, std = normalize(wav)
        passes = len(model.models) if isinstance(model, BagOfModels) else 1
        state = {"pass": 0}

        def tq(iterable, **_kw):
            items = list(iterable)
            n = max(1, len(items))
            p = state["pass"]
            state["pass"] += 1
            for i, it in enumerate(items):
                yield it
                frac = (p + (i + 1) / n) / passes
                progress(min(frac, 1.0), f"Separating ({self.name}, pass {p + 1}/{passes})")

        original = dapply.tqdm
        dapply.tqdm = SimpleNamespace(tqdm=tq)
        device = self.resolve_device()
        try:
            for attempt in (device, "cpu") if device != "cpu" else ("cpu",):
                try:
                    state["pass"] = 0
                    log(f"device={attempt}")
                    with torch.no_grad():
                        x = torch.from_numpy(np.ascontiguousarray(norm, dtype=np.float32))[None]
                        out = apply_model(
                            model, x, device=attempt, shifts=1, split=True, overlap=0.25, progress=True, num_workers=0
                        )[0]
                    device = attempt
                    break
                except RuntimeError as e:
                    if attempt == "cpu":
                        raise
                    log(f"{attempt} failed ({e}); retrying on CPU", "warn")
        except MemoryError as e:
            raise WorkerError("out_of_memory", "Out of memory during separation", str(e)) from e
        finally:
            dapply.tqdm = original
        out = out.cpu().numpy() * std + mean
        stems = {name: out[i].astype(np.float32) for i, name in enumerate(model.sources)}
        del model
        if device == "mps":
            torch.mps.empty_cache()
        return SeparationResult(stems, sr, self.name, self.model, device, time.time() - t0)
