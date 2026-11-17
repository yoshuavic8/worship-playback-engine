# wpe-worker

Locked for **darwin arm64** and **darwin x86_64** (`uv lock`). Intel uses PyTorch 2.2.2 and `onnxruntime<1.24`; Apple Silicon uses current torch and optional `demucs-mlx`.

Local Python audio preparation worker for [Worship Playback Engine](../README.md): stem separation, tempo/key/sections, backing render.

MIT licensed. See `uv.lock` for per-platform pins.

```bash
uv sync --group dev
uv run pytest
```
