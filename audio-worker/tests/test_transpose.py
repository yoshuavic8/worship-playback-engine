import json

import numpy as np
import pytest

from wpe_worker.audio import io
from wpe_worker.protocol import WorkerError
from wpe_worker.rendering import transpose as tr


def test_stem_selection_never_pitches_drums_click_or_cue():
    assert tr.pitched_stems(["vocals", "drums", "bass", "other"]) == ["other", "bass", "vocals"]
    assert tr.pitched_stems({"drums": "a", "click": "b", "cue": "c", "percussion": "d"}) == []
    assert "drums" not in tr.pitched_stems(["drums", "other"])


def test_tag_and_range():
    assert tr.tag(2) == "p2" and tr.tag(-3) == "m3"
    with pytest.raises(ValueError):
        tr.tag(0)
    with pytest.raises(WorkerError):
        tr.TransposeRequest.parse({"type": "transpose_song", "songDir": "/x", "semitones": [5]})
    r = tr.TransposeRequest.parse({"type": "transpose_song", "songDir": "/x", "semitones": [2, 2, -1], "stems": {"other": "s/o.wav"}})
    assert r.semitones == [2, -1]


def _freq(x, sr):
    spec = np.abs(np.fft.rfft(x * np.hanning(len(x))))
    return np.argmax(spec) * sr / len(x)


def test_shift_changes_pitch_not_length_or_timing():
    pytest.importorskip("python_stretch")
    sr = 48000
    t = np.arange(sr * 2) / sr
    a = np.stack([np.sin(2 * np.pi * 220 * t)] * 2).astype(np.float32) * 0.5
    up = tr.shift(a, sr, 2, "bass")
    assert up.shape == a.shape
    f = _freq(up[0, sr // 2: sr // 2 + 16384], sr)
    assert abs(f - 220 * 2 ** (2 / 12)) < 4  # 246.9 Hz
    assert np.max(np.abs(up)) < 1.0


def test_run_writes_cache_and_leaves_drums(tmp_path):
    pytest.importorskip("python_stretch")
    sr = 48000
    n = sr
    tone = np.stack([np.sin(2 * np.pi * 330 * np.arange(n) / sr)] * 2).astype(np.float32) * 0.3
    for k in ("other", "bass", "vocals", "drums"):
        io.write(str(tmp_path / "stems" / f"{k}.wav"), tone, sr)
    req = tr.TransposeRequest("s", str(tmp_path), {k: f"stems/{k}.wav" for k in ("other", "bass", "vocals", "drums")}, [-2])
    res = tr.run(req)
    assert set(res["renders"]["-2"]) == {"other", "bass", "vocals"}
    d = tmp_path / "transpose" / "m2"
    assert (d / "done.json").is_file() and not (d / "drums.wav").exists()
    meta = json.loads((d / "done.json").read_text())
    assert meta["notPitched"] == ["drums"]
    out, r = io.read(str(d / "other.wav"))
    assert r == sr and out.shape == tone.shape
