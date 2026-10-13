from wpe_worker.protocol import PrepareRequest, SeparationConfig
from wpe_worker.separation.mdxnet import MDXNetSeparator
from wpe_worker.separation.registry import candidates


def test_fast_profile_does_not_use_worship(tmp_path):
    cfg = SeparationConfig(
        profile="fast",
        model="htdemucs",
        models_dir=str(tmp_path),
        mlx_models_dir=str(tmp_path),
        mdx_models_dir=str(tmp_path),
    )
    assert all(s.name != "worship" for s in candidates(cfg))


def test_worship_without_weights_falls_back(tmp_path):
    cfg = SeparationConfig(
        profile="worship",
        model="htdemucs",
        models_dir=str(tmp_path),
        mlx_models_dir=str(tmp_path),
        mdx_models_dir=str(tmp_path),
    )
    assert MDXNetSeparator(str(tmp_path)).available()[0] is False
    assert all(s.name != "worship" for s in candidates(cfg))


def test_request_profile_defaults_to_worship():
    req = PrepareRequest.parse({
        "type": "prepare_song", "songId": "x", "input": "/a", "songDir": "/b",
    })
    assert req.separation.profile == "worship"


def test_request_profile_fast():
    req = PrepareRequest.parse({
        "type": "prepare_song", "songId": "x", "input": "/a", "songDir": "/b",
        "separation": {"profile": "fast"},
    })
    assert req.separation.profile == "fast"
