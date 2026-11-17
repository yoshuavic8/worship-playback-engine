import json
import shutil

import numpy as np
import pytest

from wpe_worker import testaudio
from wpe_worker.audio import io
from wpe_worker.protocol import PrepareRequest, StemSettings, WorkerError
from wpe_worker.audio.clean import polish
from wpe_worker.rendering.backing import render_backing, stem_gains
from wpe_worker.rendering.waveform import peaks


def stems(n=48000):
    one = np.ones((2, n), np.float32) * 0.1
    return {"vocals": one * 2, "drums": one, "bass": one, "other": one}


def test_full_band_backing_math():
    s = StemSettings(vocals="remove", drums="remove", bass="keep", bass_db=-6.0, other_db=0.0)
    backing, backup, info = render_backing(stems(), s)
    # other (0.1) + bass at -6 dB (0.0501) ; no drums, no vocals
    assert np.allclose(backing, 0.1 + 0.1 * 10 ** (-6 / 20), atol=1e-5)
    # backup = instrumental, never vocals
    assert np.allclose(backup, 0.3, atol=1e-5)
    assert info["gains"]["vocals"] == 0.0


def test_no_drummer_keeps_drums_and_bass_removal():
    g = stem_gains(StemSettings(drums="keep", bass="remove"))
    assert g["drums"] == 1.0 and g["bass"] == 0.0
    assert stem_gains(StemSettings(drums="optional"))["drums"] == 0.0


def test_limiter_protects_clipping():
    loud = {k: np.ones((2, 100), np.float32) * 0.9 for k in ("drums", "bass", "other")}
    b, _, info = render_backing(loud, StemSettings(drums="keep", bass_db=0.0))
    assert np.max(np.abs(b)) <= 0.98 + 1e-6
    assert info["limiterDbBacking"] < 0


def test_polish_removes_a_stationary_whistle():
    sr = 48000
    n = sr * 2
    rng = np.random.default_rng(1)
    noise = rng.standard_normal((2, n)).astype(np.float32) * 0.02
    t = np.arange(n) / sr
    whistle = np.sin(2 * np.pi * 3150 * t).astype(np.float32) * 0.25
    dirty = noise + whistle
    clean = polish(dirty, sr)

    def bin_at(a: np.ndarray) -> float:
        spec = np.abs(np.fft.rfft(a.mean(axis=0) * np.hanning(n)))
        return float(spec[int(round(3150 / sr * n))])

    assert bin_at(clean) < bin_at(dirty) * 0.35


def test_waveform_format():
    w = peaks(testaudio.tone(seconds=2.0), 48000, 50)
    assert w["peaksPerSecond"] == 50 and len(w["peaks"]) == 100
    assert max(w["peaks"]) <= 255 and abs(w["duration"] - 2.0) < 1e-6


def test_request_validation():
    with pytest.raises(WorkerError):
        PrepareRequest.parse({"type": "rm -rf"})
    with pytest.raises(WorkerError):
        PrepareRequest.parse({"type": "prepare_song", "songId": "x"})
    r = PrepareRequest.parse({"type": "prepare_song", "songId": "x", "input": "/a", "songDir": "/b",
                              "settings": {"drums": "weird"}})
    assert r.settings.drums == "remove"


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not installed")
def test_pipeline_analysis_only(tmp_path):
    from wpe_worker.pipeline import run

    mix, _ = testaudio.song(bpm=100.0)
    src = tmp_path / "in.wav"
    io.write(str(src), mix, testaudio.SR)
    req = PrepareRequest.parse({
        "type": "prepare_song", "songId": "t", "input": str(src), "songDir": str(tmp_path / "song"),
        "ffmpeg": shutil.which("ffmpeg"), "separation": {"enabled": False}, "analysis": {"enabled": True},
    })
    res = run(req)
    assert res["rendered"]["backup"] == "rendered/backup.wav"
    assert abs(res["analysis"]["bpm"] - 100.0) < 1.0
    assert (tmp_path / "song/analysis/waveform.json").exists()
    a, sr = io.read(str(tmp_path / "song/rendered/backup.wav"))
    assert sr == 48000
    json.loads((tmp_path / "song/analysis/analysis.json").read_text())
