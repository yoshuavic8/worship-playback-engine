# Worship Playback Engine

> **From Song to Sunday. Automatically.**

Local-first desktop app for church worship teams. Import a song, prepare a live-ready backing track (stems, click, cue, sections, tempo map), then run the set on Sunday — offline, no subscription, no cloud AI.

macOS Apple Silicon only (M1–M4). MIT licensed.

## Install from GitHub Releases

Builds are **ad-hoc signed, not Apple-notarized**. That is normal for unsigned open-source Mac apps. macOS will ask you to allow the app once.

### English

1. Download the `.dmg` from [Releases](../../releases).
2. Open it and drag **Worship Playback Engine** into `/Applications`.
3. **Right-click** the app → **Open** → **Open**. Do not double-click the first time.
4. If it is still blocked: **System Settings → Privacy & Security → Open Anyway**.
5. If macOS says the app is damaged (common on Apple Silicon when Gatekeeper quarantines a GitHub download):

```bash
xattr -cr "/Applications/Worship Playback Engine.app"
```

Then right-click → Open again.

### Bahasa Indonesia

1. Unduh file `.dmg` dari halaman **Releases**.
2. Buka, lalu tarik aplikasinya ke folder **Applications**.
3. **Klik kanan** app → **Open** → **Open**. Jangan double-click pada pembukaan pertama.
4. Jika masih diblokir: **System Settings → Privacy & Security → Open Anyway**.
5. Jika macOS bilang app rusak / *damaged*:

```bash
xattr -cr "/Applications/Worship Playback Engine.app"
```

Lalu klik kanan → Open lagi. Ini izin Gatekeeper, bukan crack dan bukan pelanggaran lisensi.

### After first launch

Preparation needs two Homebrew tools and a one-time Python environment:

```bash
brew install ffmpeg uv
```

In the app, open **Diagnostics** and install the Python environment, then download the recommended models (Demucs / UVR). Models are MIT or community-licensed weights; they are not inside the `.dmg`.

You supply your own songs. This project never ships copyrighted worship recordings.

## Requirements

- macOS 13 or later
- Apple Silicon (M1, M2, M3, or M4)
- [Homebrew](https://brew.sh) for `ffmpeg` and `uv` (Python package manager)

Intel Macs are not supported. The audio worker lockfile and MLX Demucs path target `darwin-arm64` only.

## Build a release on GitHub

After this repo is on GitHub (public is free for macOS runners):

**Manual:** Actions → **Release** → **Run workflow**. Download the `.dmg` from the run artifacts.

**Tagged release:**

```bash
git tag v0.1.0
git push origin v0.1.0
```

That publishes a GitHub Release with the `.dmg` attached. CI also runs tests on `main` / pull requests.

No Apple Developer account is required. To later remove the Gatekeeper prompt, join the [Apple Developer Program](https://developer.apple.com/programs/) (~USD 99/year) and add Developer ID + notarization secrets to the workflow.

## Develop locally

```bash
# Xcode Command Line Tools, Node 20+, Rust stable, Homebrew
brew install node rustup-init ffmpeg uv
rustup default stable

cd apps/desktop
npm install
npm run tauri dev
```

```bash
npm test            # frontend (apps/desktop)
npm run typecheck

cargo test          # from apps/desktop/src-tauri

cd audio-worker
uv sync --group dev
uv run pytest
```

Release build on your machine:

```bash
cd apps/desktop
npm run tauri -- build --target aarch64-apple-darwin
```

The `.dmg` lands in `apps/desktop/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`.

## Layout

```text
apps/desktop/          Tauri 2 + React + TypeScript
  src-tauri/           Rust audio engine, SQLite, IPC
audio-worker/          Python preparation (stems, tempo, sections)
```

## License

[MIT](LICENSE). Third-party notices and optional model licenses: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
