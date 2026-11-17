# Third-party notices

Worship Playback Engine is MIT-licensed. It depends on other open-source
projects. Separation **models** are downloaded during Setup (not in the `.dmg`).

## Bundled sidecars (macOS app)

| Component | License | Source |
|---|---|---|
| **uv** 0.5.14 | MIT OR Apache-2.0 | https://github.com/astral-sh/uv |
| **FFmpeg / ffprobe** 7.1.1 | LGPL-2.1-or-later | Static build from [ffmpeg-7.1.1.tar.xz](https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz) via `scripts/build-ffmpeg-lgpl.sh` (`--disable-gpl --disable-nonfree`, no external dylibs). Pin: `scripts/sidecars.manifest.json`. |

## Desktop application

| Component | License | Source |
|---|---|---|
| Tauri | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| React | MIT | https://github.com/facebook/react |
| Vite | MIT | https://github.com/vitejs/vite |
| Zustand | MIT | https://github.com/pmndrs/zustand |
| cpal | Apache-2.0 | https://github.com/RustAudio/cpal |
| Symphonia | MPL-2.0 | https://github.com/pdeljanov/Symphonia |
| rusqlite | MIT | https://github.com/rusqlite/rusqlite |

## Python runtime (installed by Setup via uv)

| Component | License | Source |
|---|---|---|
| CPython 3.12 | PSF | Installed by uv into app support directory |
| Demucs | MIT | https://github.com/adefossez/demucs |
| demucs-mlx | MIT | https://huggingface.co/ssmall256/demucs-mlx |
| PyTorch | BSD-style | https://github.com/pytorch/pytorch |
| librosa | ISC | https://github.com/librosa/librosa |
| NumPy | BSD | https://github.com/numpy/numpy |
| SciPy | BSD | https://github.com/scipy/scipy |
| ONNX Runtime | MIT | https://github.com/microsoft/onnxruntime |

## Optional models (Setup download)

| Resource | License (as recorded in-app) | Source |
|---|---|---|
| Demucs HT / HT fine-tuned | MIT (Meta Platforms, Inc.) | https://github.com/adefossez/demucs |
| Demucs HT MLX ports | MIT (Meta Platforms; MLX port by ssmall256) | https://huggingface.co/ssmall256/demucs-mlx |
| UVR-MDX-NET Inst HQ 3 | Community model (UVR) | https://github.com/TRvlvr/model_repo |

## Optional AI (user-installed)

| Component | License | Source |
|---|---|---|
| Ollama | MIT | https://github.com/ollama/ollama — https://ollama.com/download |

This application never bundles copyrighted songs. Users supply their own audio.
