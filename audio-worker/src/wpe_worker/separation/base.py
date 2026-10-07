"""Model abstraction layer. The pipeline never depends on a specific separation model."""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Callable

import numpy as np

ProgressFn = Callable[[float, str], None]

STEMS = ("vocals", "drums", "bass", "other")


@dataclass
class SeparationResult:
    stems: dict[str, np.ndarray]
    sample_rate: int
    engine: str
    model: str
    device: str
    seconds: float = 0.0
    notes: list[str] = field(default_factory=list)


class AudioSeparator(ABC):
    """Separate a stereo mix into vocals / drums / bass / other."""

    name: str = "abstract"

    @abstractmethod
    def available(self) -> tuple[bool, str]:
        """(usable, reason). Must be cheap and must never download anything."""

    @abstractmethod
    def separate(self, audio: np.ndarray, sample_rate: int, progress: ProgressFn) -> SeparationResult:
        """`audio` is (2, samples) float32. Returned stems are (2, samples) at `result.sample_rate`."""


class FutureSeparator(AudioSeparator):
    """Placeholder for upcoming models (e.g. BS-RoFormer / MDX). Keeps the interface honest."""

    name = "future"

    def available(self) -> tuple[bool, str]:
        return False, "not implemented"

    def separate(self, audio, sample_rate, progress):  # pragma: no cover
        raise NotImplementedError


def normalize(wav: np.ndarray) -> tuple[np.ndarray, float, float]:
    """Demucs-style input standardisation (as done by `demucs.separate`)."""
    ref = wav.mean(axis=0)
    mean = float(ref.mean())
    std = float(ref.std()) + 1e-8
    return (wav - mean) / std, mean, std
