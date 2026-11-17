# Worship Playback Engine

> **From Song to Sunday. Automatically.**

Local-first desktop app for church worship teams. Import a song, prepare a live-ready backing track (stems, click, cue, sections, tempo map), then run the set on Sunday — offline, no subscription, no cloud AI.

macOS 12+ universal (Apple Silicon + Intel). MIT licensed. **No Apple Developer account required** — releases are ad-hoc signed.

## Install from GitHub Releases

Builds are **ad-hoc signed, not Apple-notarized**. macOS will ask you to allow the app once.

### English

1. Download the `.dmg` from [Releases](../../releases).
2. Open it and drag **Worship Playback Engine** into `/Applications`.
3. **Right-click** the app → **Open** → **Open**. Do not double-click the first time.
4. If it is still blocked: **System Settings → Privacy & Security → Open Anyway**.
5. If macOS says the app is damaged (common when Gatekeeper quarantines a GitHub download):

```bash
xattr -cr "/Applications/Worship Playback Engine.app"
```

Then right-click → Open again.

6. On first launch, complete **Setup** (Python engine + separation models). Everything downloads once, then works offline.

### Bahasa Indonesia

1. Unduh `.dmg` dari halaman **Releases**.
2. Tarik aplikasi ke **Applications**.
3. **Klik kanan** → **Open** → **Open** (bukan double-click pertama kali).
4. Jika diblokir: **System Settings → Privacy & Security → Open Anyway**.
5. Jika “damaged”:

```bash
xattr -cr "/Applications/Worship Playback Engine.app"
```

6. Saat pertama dibuka, selesaikan **Persiapan** (mesin Python + model Demucs). Setelah itu offline penuh.

You supply your own songs. This project never ships copyrighted worship recordings.

## System requirements

| | |
|---|---|
| OS | macOS **12 Monterey** or later; keep Safari updated (WebKit 16.4+ for the UI) |
| **Full profile** | Apple Silicon, macOS 13.5+ — MLX + PyTorch, faster prep |
| **Lite profile** | Intel or older macOS — same Demucs models, slower CPU prep |
| Disk | ~2–3 GB after Setup (Python env + models) |
| Optional AI | [Ollama](https://ollama.com) — app is fully usable without it |

**No Homebrew, Xcode CLT, or manual `ffmpeg`/`uv` install** for end users. Those tools are bundled as sidecars; Setup installs Python via bundled `uv`.

## Third-party licenses

See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) (Demucs MIT, model weights, FFmpeg LGPL, uv, Python, Tauri, etc.).

## Build a release (maintainers)

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
chmod +x scripts/fetch-sidecars.sh scripts/build-ffmpeg-lgpl.sh
bash scripts/fetch-sidecars.sh   # pins in scripts/sidecars.manifest.json (static LGPL ffmpeg build)
cd apps/desktop && npm ci && npm run build:mac
```

Verify universal binary: `lipo -info …/Worship Playback Engine` → `x86_64 arm64`.

Tagged pushes (`v*`) build on GitHub Actions and attach the `.dmg` to the Release (ad-hoc sign, `GITHUB_TOKEN` only).

## Develop locally

```bash
cd apps/desktop && npm ci
# Optional: copy sidecars for Tauri build (or use PATH ffmpeg in dev)
bash ../../scripts/fetch-sidecars.sh
npm run tauri dev
```

In debug builds, `ffmpeg`/`uv` on your **PATH** are used if sidecars are missing.

Python worker tests: `cd audio-worker && uv run pytest`
