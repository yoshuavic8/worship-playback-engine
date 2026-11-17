# COMPAT — implementasi selesai (13 Okt 2026)

Ringkasan pekerjaan agar WPE jalan di **macOS 12+** dan **Intel Mac** tanpa menurunkan kualitas separasi, dan **tanpa bergantung pada Ollama**.

## Hasil tes (dijalankan di Mac pengembang, Apple Silicon)

| Suite | Perintah | Hasil |
|---|---|---|
| Rust | `cargo test` (apps/desktop/src-tauri) | **68 passed** |
| Frontend | `npm test` (vitest) | **27 passed** |
| Python | `uv run pytest` (audio-worker) | **28 passed** |

## 1. Bundle & build macOS lama + Intel

| Perubahan | Detail |
|---|---|
| `apps/desktop/src-tauri/tauri.conf.json` | `minimumSystemVersion` **13.0 → 12.0** |
| `apps/desktop/src-tauri/.cargo/config.toml` | `MACOSX_DEPLOYMENT_TARGET=12.0` |
| `apps/desktop/package.json` | script **`build:mac`** → `tauri build --target universal-apple-darwin` |
| `README.md` | rustup targets + verifikasi `lipo` / `otool` |
| `.github/workflows/release.yml` | CI release membangun **universal** |

**Belum diverifikasi di hardware nyata:** Intel Mac, macOS 12 Monterey fisik, dan DMG universal yang di-`lipo` (tidak dijalankan di mesin ini).

## 2. Python worker per platform (kualitas model sama)

| Perubahan | Detail |
|---|---|
| `audio-worker/pyproject.toml` | Lock **arm64 + x86_64**; torch **≥2.6 (arm64)**, **==2.2.2 (x86_64)**; **onnxruntime&lt;1.24**; **numpy&lt;2** di x86_64; **demucs-mlx** tetap arm64-only |
| `audio-worker/uv.lock` | Di-regenerate; onnxruntime **1.23.2**; torch dual-version |
| Verifikasi | `uv pip compile --python-platform x86_64-apple-darwin` sukses |

Tidak ada penggantian model diam-diam: pengaturan user (`htdemucs`, `htdemucs_ft`, dll.) tetap dipakai; Intel hanya lebih lambat (CPU/MPS).

## 3. Profil Full / Lite (Rust)

| File | Isi |
|---|---|
| `apps/desktop/src-tauri/src/compat.rs` | Deteksi arsitektur (Rosetta, `hw.optional.arm64`), macOS, RAM, Ollama; **Full** = Apple Silicon native + macOS ≥13.5; **Lite** = sisanya; MLX hanya jika eligible |
| `system.rs` | `Readiness.compat` + baris diagnostik (profil, arsitektur, backend separasi, Ollama) |
| Dashboard | Kartu System menampilkan profil, alasan, peringatan ETA Lite |

Prep queue sudah **satu job** (`prep.rs`); Lite menampilkan peringatan durasi, bukan model lebih rendah.

## 4. Tanpa Ollama (fungsional penuh)

| Area | Perilaku |
|---|---|
| Probe Ollama | Timeout ~1.5s, cache 30s — tidak memblokir UI |
| `ai.rs` | **Heuristic → rules engine** bila Ollama off/gagal; streaming, `num_ctx` 2048/4096, `keep_alive` 30m, cancel |
| Model Ollama | Intel / RAM ≤8 GB / Lite → tiny models (≤3B); hint `ollama pull qwen2.5:1.5b` |
| Arrangement | Editor: drag, **duplicate**, **preset** Intro-V-C-V-C-B-C-Outro |
| Assistant UI | Tetap bisa mengetik frase umum; badge “Built-in rules” |

## 5. Key detection & transpose-ready

| Layer | Perubahan |
|---|---|
| `audio-worker/.../key.py` | Krumhansl–Schmuckler + **pitch class + mode** |
| `pipeline.py` | `keyPitchClass`, `keyMode` di analysis |
| Rust `Song` | `keyPitchClass`, `keyMode`; sync di `sanitize_song` / `keys.rs` |
| Frontend | `formatKeyDisplay` — contoh ID: **“Nada dasar: sol = G”** |

## 6. Frontend WebKit

| File | Guard |
|---|---|
| `apps/desktop/src/main.tsx` | Jika `CSS.supports('color-mix…')` gagal → layar inline (update Software Update) |

## 7. Tes baru

- Rust: `compat.rs` (profil, model pick), `ai.rs` (heuristic, no-Ollama proposal), `keys.rs`
- Python: key pitch/mode di `test_analysis.py`
- Frontend: `keyDisplay.test.ts`, `arrangementPresets.test.ts`

## Yang masih perlu QA manual

1. Buka app di **Intel + macOS 12** — install `uv sync`, prep satu lagu, live playback.
2. **Universal binary**: `npm run build:mac` lalu `lipo -info` dan `LC_BUILD_VERSION minos 12.0`.
3. **Ollama v0.12.3** di macOS 12/13 dengan model 1.5B — streaming + cancel.
4. **Safari/WebKit lama** (mis. Catalina) — guard harus muncul (tidak didukung penuh).

## Pesan user (rekomendasi)

WPE butuh **macOS 12+** dengan **Safari terbaru** (WebKit 16.4+). Mac Intel didukung mode **Lite**: separasi stem memakan waktu lebih lama; model Demucs **sama**. Asisten AI opsional (Ollama atau aturan bawaan). Live playback normal di semua Mac yang didukung.

## Follow-up typecheck (13 Okt 2026)

Perbaikan kecil TypeScript: import `Bot` tidak terpakai di `AiAssistant.tsx`; guard `settings?.ollamaEnabled` di `Settings.tsx` (`checkAi` bisa jalan sebelum bootstrap). Verifikasi: `npx tsc --noEmit` dan `npx vitest run` di `apps/desktop` — **lulus**.

## Open-source release prep (13 Okt 2026)

### Tanpa Homebrew (pengguna akhir)
- Sidecar Tauri: **uv**, **ffmpeg**, **ffprobe** (`externalBin`, `scripts/fetch-sidecars.sh`, checksum uv di `scripts/sidecars.manifest.json`).
- Python: `uv python install 3.12` → `UV_PYTHON_INSTALL_DIR` di app support; `uv sync --frozen` → `python-env/`.
- `find_tool()` → sidecar dulu; PATH hanya di **debug**.
- Pesan “brew install” dihapus dari Diagnostics / prep.

### Setup wizard (ID)
- `Setup.tsx`: mesin Python + model profil Full/Lite; progress, jeda/lanjut, unduhan resume+sha256 (logic `resources.rs`), retry backoff (`download_with_retries`).
- Lewati otomatis bila `bootstrap.setup.complete`.
- Diagnostics → **Repair setup**.

### Ollama opsional
- Toggle **Aktifkan Asisten AI**; mati = `probe_ollama_if_enabled` tidak dipanggil.
- Unduh resmi (ollama.com / GitHub releases); macOS 12–13 → v0.12.3; pull model via `/api/pull`.

### Release CI
- `.github/workflows/release.yml`: `bash scripts/fetch-sidecars.sh` (static LGPL ffmpeg), `npm run build:mac`, ad-hoc sign, DMG ke Release, hanya `GITHUB_TOKEN`.
- README + `THIRD_PARTY_NOTICES.md` diperbarui; `.gitignore` untuk sidecar binaries, secrets, models.

### Tes (Mac dev, 13 Okt 2026)
| Suite | Hasil |
|---|---|
| `cargo test` | **76 passed** |
| `npx tsc --noEmit` + `npx vitest run` | **28 passed** |
| `uv run pytest` | **28 passed** |

### Scan secrets (git history)
Pola `ghp_…`, `sk-…`, `BEGIN PRIVATE KEY`: **0** kecocokan pada scan cepat.

### Belum diverifikasi
- Release CI penuh dengan build FFmpeg **x86_64** dari sumber (±20 menit).
- DMG universal di Intel macOS 12 tanpa Homebrew.
- Setup wizard di jaringan lambat / offline penuh setelah unduhan pertama.

## v0.1.1 sidecar / LGPL ffmpeg (13 Okt 2026)

### Perubahan
- **`scripts/build-ffmpeg-lgpl.sh`**: bash 3.2 aman (`${extra_configure[@]+…}`); bangun static LGPL **ffmpeg 7.1.1** dari sumber; checksum tarball di manifest; prefix build tanpa spasi (`~/.cache/wpe-ffmpeg-build`); `--disable-x86asm` bila nasm/yasm tidak ada; **`otool -L` gate** (gagal jika dylib di luar `/usr/lib`, `/System`, `@rpath` loader).
- **`scripts/fetch-sidecars.sh`**: salin Homebrew/PATH **dihapus** (hanya `ALLOW_SYSTEM_FFMPEG_COPY=1` untuk dev). Default: build **aarch64** + **x86_64**; **`lipo`** → `*-universal-apple-darwin` untuk uv/ffmpeg/ffprobe (Tauri `externalBin` universal build). Cache uv: skip unduh jika arsip di `.cache` sudah sha256 benar.
- **`scripts/sidecars.manifest.json`**: ffmpeg **7.1.1**, sha256 sumber `733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1`.
- **`THIRD_PARTY_NOTICES.md`**, **release CI** (`FORCE_FFMPEG_BUILD` dihapus), **README** diperbarui.
- Versi app **0.1.1**: `tauri.conf.json`, `Cargo.toml` / `Cargo.lock`, `package.json` / `package-lock.json`.

### `bash scripts/fetch-sidecars.sh` (end-to-end)
- uv aarch64 + x86_64: checksum OK.
- ffmpeg/ffprobe: build dari sumber (cache tarball di `binaries/.cache/ffmpeg-7.1.1/`); skip rebuild bila sidecar sudah lulus `otool`.

### `otool -L` (semua sidecar ffmpeg/ffprobe)
Hanya dependensi sistem (sama untuk keempat binary):

| Library |
|---|
| `/usr/lib/libSystem.B.dylib` |
| `/usr/lib/libz.1.dylib` |
| `/System/Library/Frameworks/CoreFoundation.framework/...` |
| `/System/Library/Frameworks/CoreVideo.framework/...` |
| `/System/Library/Frameworks/CoreMedia.framework/...` |

Tidak ada `/opt/homebrew/Cellar/...` atau dylib pihak ketiga lain.

### Tes (Mac dev, 13 Okt 2026, setelah v0.1.1)
| Suite | Perintah | Hasil |
|---|---|---|
| TypeScript | `npx tsc --noEmit` (apps/desktop) | **lulus** |
| Frontend | `npx vitest run` | **28 passed** |
| Rust | `cargo test` (src-tauri) | **76 passed** |
| Python | `uv run pytest` (audio-worker) | **28 passed** |
