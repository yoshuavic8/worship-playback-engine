import numpy as np
import pytest

from wpe_worker import testaudio
from wpe_worker.analysis.key import detect_key, key_from_chroma
from wpe_worker.analysis.sections import boundaries_from_features, detect_sections, name_segments
from wpe_worker.analysis.tempo import detect_tempo
from wpe_worker.audio.io import resample


@pytest.fixture(scope="module")
def synth():
    mix, sections = testaudio.song(bpm=96.0)
    y = resample(mix.mean(axis=0)[None], testaudio.SR, 22050)[0]
    return y, sections


def test_bpm_is_detected(synth):
    y, _ = synth
    t = detect_tempo(y, 22050, 4)
    assert t.bpm is not None
    assert abs(t.bpm - 96.0) < 1.0, t.bpm
    assert t.confidence > 0.6
    # first downbeat is at 0 s (mod one bar of 2.5 s)
    bar = 4 * 60 / 96
    err = min(t.beat_offset % bar, bar - t.beat_offset % bar)
    assert err < 0.08, t.beat_offset


def test_fast_song_reports_double_tempo():
    mix, _ = testaudio.song(bpm=128.0, seed=3)
    y = resample(mix.mean(axis=0)[None], testaudio.SR, 22050)[0]
    t = detect_tempo(y, 22050, 4)
    # half-time feel is ambiguous: either detected directly or offered as the alternative
    assert t.bpm == 128.0 or 128.0 in t.alternatives, (t.bpm, t.alternatives)


def test_key_is_a_major(synth):
    y, _ = synth
    key, conf = detect_key(y, 22050)
    assert key in ("A", "F#m"), key  # relative minor is an acceptable confusion
    assert conf > 0


def test_key_profile_rotation():
    c_major = np.zeros(12)
    c_major[[0, 4, 7]] = 1.0
    c_major[[2, 5, 9, 11]] = 0.5
    assert key_from_chroma(c_major)[0] == "C"


def test_sections_find_structure(synth):
    y, truth = synth
    secs = detect_sections(y, 22050, 96.0, 0.0, 4)
    assert secs[0]["start"] == 0.0
    assert 4 <= len(secs) <= 12
    labels = [s["label"] for s in secs]
    assert "Chorus" in labels
    # most true boundaries are found within one bar
    bar = 4 * 60 / 96
    starts = [s["start"] for s in secs]
    hits = sum(1 for _, t in truth[1:] if min(abs(t - s) for s in starts) <= bar + 0.01)
    assert hits >= len(truth[1:]) * 0.6, (starts, truth)


def test_boundaries_simple_blocks():
    rng = np.random.default_rng(0)
    a = np.ones((2, 6)) + rng.normal(0, 0.02, (2, 6))
    b = -np.ones((2, 6)) + rng.normal(0, 0.02, (2, 6))
    feats = np.vstack([a, b, a, b])
    assert boundaries_from_features(feats) == [0, 2, 4, 6]


def test_naming_heuristics():
    clusters = [0, 1, 2, 1, 2, 3, 2, 4]
    energy = [0.3, 0.6, 1.0, 0.6, 1.0, 0.8, 1.0, 0.3]
    lengths = [4, 8, 8, 8, 8, 8, 8, 4]
    names = [n for n, _ in name_segments(clusters, energy, lengths)]
    assert names == ["Intro", "Verse 1", "Chorus", "Verse 2", "Chorus", "Bridge", "Chorus", "Outro"]
