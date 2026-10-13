# WPE — Kompatibilitas macOS lama & Intel Mac (13 Okt 2026)

Diperiksa read-only dari repo Mac (`/Users/yoshuavictor/react/Worship Playback Engine`), ditambah riset web. Repo tidak diubah.
Catatan: "iOS 12" yang dimaksud hampir pasti **macOS 12 Monterey**.

## Ringkasan singkat
| # | Gejala | Akar masalah utama |
|---|---|---|
| 1 | App tidak mau jalan di macOS 12 (dan 11/10.15) | Bundle mewajibkan **macOS 13.0** (`tauri.conf.json:48`), jadi Finder menolak membuka app. Kalaupun dibuka, setup Python gagal karena lockfile hanya berisi arm64 dan onnxruntime 1.30 butuh macOS 14. |
| 2 | Di Intel Mac app/prep/AI tidak jalan atau lambat | (a) Build hanya **aarch64**: app tidak bisa dibuka sama sekali di Intel kecuali di-build khusus. (b) `uv.lock` sengaja dibatasi ke arm64 (`audio-worker/pyproject.toml:21`), dan **torch ≥2.3 & onnxruntime ≥1.24 tidak punya wheel x86_64 macOS**, jadi `uv sync --frozen` gagal. (c) Ollama di Intel **CPU-only**, butuh macOS 14+, sementara `ai.rs` memakai num_ctx 8192, tanpa streaming, timeout 90 detik, dan memilih model qwen2.5 apa pun ukurannya, sehingga sering timeout. |

## Bukti (file:line / binary)
**Bundle / binary**
- `apps/desktop/src-tauri/tauri.conf.json:47-49` berisi `"macOS": { "minimumSystemVersion": "13.0" }`. Nilai ini menjadi `LSMinimumSystemVersion=13.0` di Info.plist, sehingga macOS 12 menampilkan "requires macOS 13.0 or later".
- `tauri.conf.json:32` berisi `targets ["app","dmg"]`. Tidak ada build universal: tidak ada `.cargo/config`, script build tidak memakai `--target`.
- `target/` hanya berisi `debug`. Tidak ada bundle release di Mac ini.
  - `target/debug/worship-playback-engine`: `otool -l` menunjukkan `LC_BUILD_VERSION minos 11.0, sdk 26.2`. Mac pembuatnya arm64 (macOS 26.5.2), jadi binary-nya **arm64 saja**.
  - Artinya batas sebenarnya dari Rust adalah 11.0. Angka 13.0 hanya ditetapkan di config.
- `rust-toolchain.toml` memakai `stable` tanpa target tambahan.

**Frontend (WKWebView = WebKit sistem)**
- `apps/desktop/vite.config.ts:17` memakai `build.target: "safari16"`.
- Tailwind **v4.1** (`package.json:39,49`) resmi butuh **Safari 16.4+** (memakai `@property`, `color-mix`, cascade layers).
- `color-mix()` juga dipakai langsung di `src/styles.css:89,121` dan `src/components/SongTimeline.tsx:402`.
- Dampaknya per versi macOS:
  - **Monterey dan Big Sur aman bila Safari di-update** (Monterey bisa sampai Safari 17.6, Big Sur sampai 16.6; WebKit sistem ikut ter-update).
  - **Catalina (maks. Safari 15.6) akan rusak tampilannya**: warna/variabel hilang, layout berantakan.

**Python audio worker** (dipasang saat runtime via `uv`, bukan dibundel)
- `tauri.conf.json:42-46` hanya membundel `pyproject.toml`, `uv.lock` dan `src/`.
- `src-tauri/src/resources.rs:460-493` mencari `uv` di sistem (`brew install uv`) lalu menjalankan `uv sync --frozen --no-dev --python 3.12`.
- `audio-worker/pyproject.toml:21` berisi `environments = ["sys_platform == 'darwin' and platform_machine == 'arm64'"]`, jadi lockfile **hanya di-resolve untuk arm64**. Di Intel, `uv sync --frozen` gagal karena platform tidak tercakup.
- `pyproject.toml:14` berisi `torch>=2.6` (terkunci 2.14.1, `uv.lock:802-803`). PyTorch berhenti merilis wheel macOS x86_64 setelah **2.2.2**.
- `pyproject.toml:15` berisi `onnxruntime>=1.19` (terkunci **1.30.0**, `uv.lock:527-528`). Sejak **1.24.1** onnxruntime tidak lagi merilis wheel x86_64 dan **minimum macOS-nya 14.0**. Akibatnya bahkan **Apple Silicon dengan macOS 12/13 pun gagal** `uv sync`.
- `pyproject.toml:16`: `demucs-mlx` hanya untuk arm64 (benar). MLX butuh Apple Silicon dan macOS ≥13.5.
- `audio-worker/src/wpe_worker/separation/registry.py:17-25` mencoba urutan MLX → torch auto → torch CPU. Tidak ada jalur tanpa torch.
- ffmpeg dan uv diambil dari Homebrew (`system.rs:100-114`, `paths.rs:72-73`). Di Intel, Homebrew ada di `/usr/local`, sudah tercakup. Namun Homebrew sendiri sekarang Tier-3 untuk macOS 12.

**Ollama / AI**
- `src-tauri/src/ai.rs:43` memakai timeout **90 detik**.
- `ai.rs:46-49` memakai `"stream": false`, `num_ctx: 8192`, `format: schema`, `keep_alive: "5m"`.
- `ai.rs:76-84` memilih model: setting → model `qwen2.5*` mana pun (bisa 7b/14b) → `llama3*` → model pertama. **Ukuran model tidak dipertimbangkan.**
- `settings.rs:66-68` menetapkan URL `http://127.0.0.1:11434` dan `ollama_model: None`. Ollama tidak dibundel; user memasangnya sendiri.
- Di Intel, Ollama CPU-only. Model 7B dengan konteks 8192 dan prompt JSON-schema bisa makan >90 detik untuk prompt-eval saja, sehingga muncul error "not reachable/timeout". Setelah 5 menit idle model di-unload, jadi request berikutnya kembali lambat.
- Ollama versi terbaru butuh **macOS 14+**. Versi terakhir untuk macOS 12/13 adalah **v0.12.3**.

## Rencana perbaikan (urut prioritas)
1. **Turunkan minimum OS dan build universal.**
   - Ubah `minimumSystemVersion` menjadi `"12.0"`. Pilih 11.0 jika ingin, tetapi QA sulit.
   - Build dengan `rustup target add x86_64-apple-darwin aarch64-apple-darwin && npm run tauri build -- --target universal-apple-darwin`.
   - Set `MACOSX_DEPLOYMENT_TARGET=12.0` di `src-tauri/.cargo/config.toml` (`[env]`).
   - Verifikasi hasil dengan `lipo -info` (harus "x86_64 arm64") dan `otool -l | grep -A4 LC_BUILD_VERSION` (minos 12.0).
   - Tradeoff: ukuran ±2×. Alternatifnya dua DMG terpisah. Tauri v2 sendiri mendukung ≥10.15.
2. **Frontend:** pertahankan Tailwind v4, nyatakan syarat "Safari/WebKit ≥16.4".
   - Saat startup, cek `CSS.supports('color','color-mix(in oklab, red, blue)')`. Bila gagal, tampilkan layar polos (inline style): "Update Safari lewat Software Update". Tradeoff: tidak perlu mengubah ke Tailwind v3.
   - Untuk Catalina: tidak didukung. Downgrade ke Tailwind v3 terlalu mahal.
3. **Python environment per platform.**
   - Ganti `environments` menjadi tiga entri: darwin arm64, darwin x86_64, dan seluruh darwin. Tambahkan marker:
     ```
     torch>=2.6; platform_machine=='arm64'
     torch==2.2.2; platform_machine=='x86_64'
     onnxruntime>=1.19,<1.24; platform_machine=='x86_64' or platform_release<'23'
     ```
     (atau pin `onnxruntime<1.24` untuk semua demi macOS 12/13). Tambahkan `numpy<2` di x86_64; numpy saat ini sudah 1.26.4.
   - Jalankan `uv lock` ulang dan uji di Intel nyata.
   - Atau pakai profil yang jalan tanpa torch: separasi via onnxruntime 1.23 + model ONNX (mdxnet sudah ada di `separation/mdxnet.py`), atau demucs.cpp sebagai sidecar.
   - Tradeoff: torch 2.2.2 sudah usang, tetapi demucs 4 masih jalan. Di Intel CPU separasi ±1–3× durasi lagu.
4. **Compatibility profile saat startup (Rust `system.rs`).**
   - Deteksi `uname -m`/`sysctl hw.optional.arm64` (agar Rosetta terdeteksi), `sw_vers`, RAM (`hw.memsize`) dan versi Ollama.
   - Profil **Full**: Apple Silicon dan macOS ≥13.5. Engine MLX, AI default.
   - Profil **Lite**: Intel, atau macOS 12/13. Engine torch-CPU/ONNX, model htdemucs (bukan _ft 4×), satu job prep sekaligus, peringatan durasi; AI tiny atau off.
   - Tampilkan profil di Dashboard › System dan Diagnostics, berikut alasannya ("Intel Mac: separasi CPU, ±3 menit per lagu").
5. **AI / Ollama.**
   - Pilihan model sadar-hardware: Intel atau RAM ≤8 GB memakai `qwen2.5:1.5b` → `llama3.2:1b` → `gemma3:1b`. Apple Silicon ≥16 GB boleh `qwen2.5:7b`. Jangan pilih model >3B di Intel.
   - Turunkan `num_ctx` ke 2048–4096 (brief lagu kecil).
   - Gunakan `stream: true` dengan timeout per-chunk 60 detik dan total 300 detik, plus tombol batal.
   - Gunakan `keep_alive: "30m"` dan warm-up (request kosong) saat membuka editor.
   - Saat gagal atau timeout, fallback ke Rules Engine yang sudah ada, dengan pesan jelas.
   - macOS 12/13: arahkan ke Ollama v0.12.3, atau matikan AI otomatis.
   - (Opsional) toggle API cloud. Tradeoff: tidak lagi "local-first", perlu key dan privasi.
6. **Diagnostics:** tambahkan baris "Arsitektur / macOS / RAM / Profil / Ollama versi & model / backend separasi". Arahkan error `uv sync` ke penjelasan yang manusiawi.
7. **QA matrix:** VM/perangkat Monterey arm64, Intel macOS 12, dan Intel macOS 14. Uji install → prep satu lagu → Live → AI.

## Matriks dukungan (rekomendasi)
| Mesin | macOS | Status | Catatan |
|---|---|---|---|
| Apple Silicon | 14+ | **Full** | MLX, Ollama terbaru, AI 3–7B |
| Apple Silicon | 13.5–13.x | Full* | MLX ok; Ollama maks. v0.12.3; onnxruntime <1.24 |
| Apple Silicon | 12 | **Lite** | torch (MPS/CPU), tanpa MLX; Ollama v0.12.3 + model kecil |
| Intel | 14+ | **Lite** | torch 2.2.2 CPU / ONNX; prep lambat; AI 1–1.5B atau off |
| Intel | 12–13 | **Lite (AI off default)** | sama seperti di atas; Ollama v0.12.3 opsional |
| Semua | 11 | Best-effort | Safari harus 16.4+ (maks. 16.6), tidak di-QA |
| Semua | ≤10.15 | **Tidak didukung** | WebKit 15 tidak cukup untuk Tailwind v4 |

**Pesan untuk user:** "WPE butuh macOS 12 atau lebih baru dengan Safari terbaru. Mac Intel didukung dalam mode Lite: pemisahan stem memakan waktu lebih lama dan asisten AI memakai model kecil atau bisa dimatikan. Live playback berjalan normal di semua Mac yang didukung."

## Instruksi untuk Cursor agent (siap tempel)
```
Goal: make WPE run on macOS 12+ and on Intel Macs, with a "Lite" profile. Do not break Apple Silicon Full mode.

1. src-tauri/tauri.conf.json: bundle.macOS.minimumSystemVersion "13.0" -> "12.0".
   Add src-tauri/.cargo/config.toml with [env] MACOSX_DEPLOYMENT_TARGET = "12.0".
   Add npm script "build:mac": "tauri build --target universal-apple-darwin" (requires rustup targets x86_64-apple-darwin, aarch64-apple-darwin).
   Verify: lipo -info on the bundled binary shows x86_64 arm64; otool -l | grep -A4 LC_BUILD_VERSION shows minos 12.0.
2. audio-worker/pyproject.toml:
   - environments = darwin arm64 AND darwin x86_64.
   - torch: ">=2.6; platform_machine=='arm64'" and "==2.2.2; platform_machine=='x86_64'"; torchaudio to match.
   - onnxruntime: "<1.24" (1.24+ dropped x86_64 and needs macOS 14).
   - numpy<2 on x86_64.
   - Keep demucs-mlx arm64-only.
   Re-run `uv lock` and confirm `uv sync --frozen` resolves for both platforms (uv pip compile --python-platform x86_64-apple-darwin).
3. Rust system.rs: add a compat profile {arch (detect Rosetta via sysctl.proc_translated / hw.optional.arm64), macos_version, ram_gb, ollama_version} -> "full" | "lite".
   Expose it in the system status, show it on Dashboard System card and Diagnostics.
   In Lite: separation engine = torch cpu (or onnx), model htdemucs, prep concurrency 1, show ETA warning. MLX only if arm64 && macOS >= 13.5.
4. ai.rs:
   - Model choice by profile: Lite or RAM <= 8GB -> prefer qwen2.5:1.5b, llama3.2:1b, gemma3:1b; never auto-pick a model > 3B on Intel. Show "pull qwen2.5:1.5b" hint when none are installed.
   - num_ctx 4096 (2048 in Lite).
   - stream:true with a per-chunk idle timeout of 60s and a total of 300s; cancellable.
   - keep_alive "30m"; warm-up request when the song editor opens.
   - On timeout/error, fall back to the Rules Engine with a clear message.
5. Frontend startup guard: if !CSS.supports('color','color-mix(in oklab, red, blue)'), render a plain inline-styled screen: "Please update Safari (WebKit 16.4+) via Software Update".
   Keep the vite target safari16.
6. Diagnostics: rows for Architecture, macOS, RAM, Profile, Ollama version/model, Separation backend; map uv sync failures to a human message.
7. Add docs/SUPPORT.md with the support matrix (Full: Apple Silicon macOS 13.5+; Lite: Intel or macOS 12-13; unsupported: <=10.15).
Run cargo test, npm test and the Python tests. Build a universal DMG and test it on an Intel Mac with macOS 12 if available.
```

## Sumber
- Ollama macOS requirements (macOS 14+, x86 CPU-only): https://docs.ollama.com/macos
- Ollama v0.12.4 menghentikan dukungan macOS 12/13: https://www.what-version.com/compatibility/ollama-system-requirements/ dan https://github.com/ollama/ollama/issues/14001
- PyTorch berhenti merilis wheel macOS x86: https://dev-discuss.pytorch.org/t/pytorch-macos-x86-builds-deprecation-starting-january-2024/1690 dan https://github.com/pytorch/pytorch/releases/tag/v2.2.0
- onnxruntime 1.24.1 (tanpa x86_64 macOS, min macOS 14): https://github.com/microsoft/onnxruntime/releases/tag/v1.24.1
- Tailwind v4 browser support (Safari 16.4): https://tailwindcss.com/docs/compatibility
- Tauri v2 prasyarat dan build universal (min macOS 10.15): https://v2.tauri.app/start/prerequisites/ dan https://v2.tauri.app/distribute/macos-application-bundle/
- MLX (Apple Silicon, macOS ≥13.5): https://ml-explore.github.io/mlx/build/html/install.html
