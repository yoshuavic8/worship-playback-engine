"""UVR-MDX-NET Inst HQ 3.

This model is trained to pull a musical instrumental out of a full mix. It is the vocal
path in the worship profile. Drum, bass and other still come from Demucs, run on this
instrumental so the lead vocal is already gone.

Parameters are the UVR hash for UVR-MDX-NET-Inst_HQ_3.onnx
(md5 of the last 10_240_000 bytes = 55657dd70583b0fedfba5f67df11d711):
n_fft 6144, dim_f 3072, dim_t 256, compensate 1.022, primary stem Instrumental.
"""

from __future__ import annotations

import time
from pathlib import Path

import numpy as np
import torch

from ..audio.io import fit_length, resample
from ..protocol import WorkerError, log
from .base import AudioSeparator, ProgressFn, SeparationResult

FILENAME = "UVR-MDX-NET-Inst_HQ_3.onnx"
MODEL_SR = 44100
N_FFT = 6144
HOP = 1024
DIM_F = 3072
DIM_T = 256  # 2 ** mdx_dim_t_set
COMPENSATE = 1.022
OVERLAP = 0.25
CHUNK = HOP * (DIM_T - 1)
TRIM = N_FFT // 2


class _STFT:
    def __init__(self) -> None:
        self.window = torch.hann_window(N_FFT, periodic=True)

    def forward(self, wave: torch.Tensor) -> torch.Tensor:
        """(batch, 2, time) -> (batch, 4, dim_f, frames)."""
        batch, channels, frames = wave.shape
        spec = torch.stft(
            wave.reshape(batch * channels, frames),
            n_fft=N_FFT,
            hop_length=HOP,
            window=self.window,
            center=True,
            return_complex=True,
        )
        spec = torch.view_as_real(spec).permute(0, 3, 1, 2)
        spec = spec.reshape(batch, channels * 2, spec.shape[-2], spec.shape[-1])
        return spec[..., :DIM_F, :]

    def inverse(self, spec: torch.Tensor) -> torch.Tensor:
        """(batch, 4, dim_f, frames) -> (batch, 2, time)."""
        batch, channels, freq, frames = spec.shape
        n_bins = N_FFT // 2 + 1
        if freq < n_bins:
            spec = torch.cat([spec, torch.zeros(batch, channels, n_bins - freq, frames)], dim=-2)
        spec = spec.reshape(batch, channels // 2, 2, n_bins, frames)
        spec = spec.reshape(-1, 2, n_bins, frames).permute(0, 2, 3, 1)
        complex_spec = torch.complex(spec[..., 0], spec[..., 1])
        wave = torch.istft(complex_spec, n_fft=N_FFT, hop_length=HOP, window=self.window, center=True)
        return wave.reshape(batch, 2, -1)


class MDXNetSeparator(AudioSeparator):
    name = "mdx"

    def __init__(self, models_dir: str):
        self.models_dir = Path(models_dir) if models_dir else Path()
        self.model = "inst-hq-3"
        self.path = self.models_dir / FILENAME

    def available(self) -> tuple[bool, str]:
        try:
            import onnxruntime  # noqa: F401
        except ImportError as e:
            return False, f"onnxruntime not installed ({e})"
        if not self.path.is_file():
            return False, "UVR-MDX-NET Inst HQ 3 is not downloaded"
        return True, "ok"

    def separate(self, audio: np.ndarray, sample_rate: int, progress: ProgressFn) -> SeparationResult:
        ok, why = self.available()
        if not ok:
            raise WorkerError("model_missing", why)
        import onnxruntime as ort

        t0 = time.time()
        progress(0.0, "Loading UVR-MDX-NET Inst HQ 3")
        session, device = _open_session(ort, self.path)
        log(f"device=mdx-{device}")
        try:
            mix = resample(np.asarray(audio, dtype=np.float32), sample_rate, MODEL_SR)
            if mix.shape[0] == 1:
                mix = np.concatenate([mix, mix], axis=0)
            mix = mix[:2]
            instrumental, vocals = _separate_mix(session, mix, lambda f: progress(f, "Removing vocals (Inst HQ 3)"))
        finally:
            del session
        return SeparationResult(
            {"instrumental": instrumental, "vocals": vocals},
            MODEL_SR,
            self.name,
            self.model,
            f"mdx-{device}",
            time.time() - t0,
            notes=["vocals: UVR-MDX-NET Inst HQ 3"],
        )


def _open_session(ort, path: Path):
    options = ort.SessionOptions()
    options.log_severity_level = 3
    available = set(ort.get_available_providers())
    attempts = []
    if "CoreMLExecutionProvider" in available:
        attempts.append((["CoreMLExecutionProvider", "CPUExecutionProvider"], "coreml"))
    attempts.append((["CPUExecutionProvider"], "cpu"))
    errors: list[str] = []
    dummy = np.zeros((1, 4, DIM_F, DIM_T), dtype=np.float32)
    for providers, device in attempts:
        try:
            session = ort.InferenceSession(str(path), sess_options=options, providers=providers)
            session.run(None, {"input": dummy})
            return session, device
        except Exception as e:  # noqa: BLE001 — try the next execution provider
            errors.append(f"{device}: {e}")
    raise WorkerError("separation_failed", "Inst HQ 3 could not start", "; ".join(errors))


def _separate_mix(session, mix: np.ndarray, on_progress) -> tuple[np.ndarray, np.ndarray]:
    """Return instrumental and vocals at 44.1 kHz, same length as `mix`."""
    peak = float(np.max(np.abs(mix))) or 1.0
    norm = mix / peak
    instrumental = _demix(session, norm, on_progress) * peak * COMPENSATE
    instrumental = fit_length(instrumental, mix.shape[-1])
    vocals = fit_length(mix - instrumental, mix.shape[-1])
    return instrumental.astype(np.float32), vocals.astype(np.float32)


def _demix(session, mix: np.ndarray, on_progress) -> np.ndarray:
    stft = _STFT()
    gen = CHUNK - 2 * TRIM
    pad = gen + TRIM - (mix.shape[-1] % gen)
    mixture = np.concatenate(
        (np.zeros((2, TRIM), dtype=np.float32), mix.astype(np.float32), np.zeros((2, pad), dtype=np.float32)),
        axis=1,
    )
    step = int((1 - OVERLAP) * CHUNK)
    result = np.zeros((1, 2, mixture.shape[-1]), dtype=np.float32)
    divider = np.zeros((1, 2, mixture.shape[-1]), dtype=np.float32)
    stops = list(range(0, mixture.shape[-1], step))
    for n, start in enumerate(stops):
        end = min(start + CHUNK, mixture.shape[-1])
        actual = end - start
        piece = mixture[:, start:end]
        if piece.shape[-1] < CHUNK:
            piece = np.pad(piece, ((0, 0), (0, CHUNK - piece.shape[-1])))
        window = _hann(actual)[None, None, :]
        window = np.tile(window, (1, 2, 1))
        pred = _infer(session, stft, piece)
        pred = pred[..., :actual] * window
        result[..., start:end] += pred[..., : actual]
        divider[..., start:end] += window
        if n % 2 == 0 or n == len(stops) - 1:
            on_progress((n + 1) / len(stops))
    divided = np.divide(result, divider, out=np.zeros_like(result), where=divider > 1e-8)
    out = divided[0, :, TRIM : TRIM + mix.shape[-1]]
    return out.astype(np.float32)


def _hann(n: int) -> np.ndarray:
    if n <= 1:
        return np.ones(max(n, 0), dtype=np.float32)
    k = np.arange(n, dtype=np.float64)
    return (0.5 - 0.5 * np.cos(2 * np.pi * k / (n - 1))).astype(np.float32)


def _infer(session, stft: _STFT, piece: np.ndarray) -> np.ndarray:
    spec = stft.forward(torch.from_numpy(piece[None]))
    spec[:, :, :3, :] = 0
    pred = session.run(None, {"input": np.ascontiguousarray(spec.numpy(), dtype=np.float32)})[0]
    wave = stft.inverse(torch.from_numpy(np.asarray(pred, dtype=np.float32)))
    return wave.numpy()
