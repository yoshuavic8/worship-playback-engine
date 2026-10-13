# RETEST — Worship Playback Engine (setelah perubahan Cursor, 40 file +3516/−520 di atas 7c6712c)

Tanggal: 9 Okt 2026 (WIB). Ini pengujian nyata, tanpa mockup. Repo di Mac hanya dibaca, tidak ada file repo yang diubah.
Snapshot di box sudah dicek identik dengan repo Mac (md5 dari 5 file kunci sama).

## 1. Hasil test suite

| Suite | Box | Mac |
|---|---|---|
| Python worker `pytest` | 28 lulus / 0 gagal | 28 lulus / 0 gagal (`uv run pytest`, 13,9 s) |
| Frontend `vitest` | 18/18 lulus (2 file) | 18/18 lulus |
| `tsc --noEmit` | OK (exit 0) | OK (exit 0) |
| Rust `cargo test` | — (dilewati, butuh webkit2gtk) | 60 lulus / 0 gagal, termasuk golden tempo_map dan `parallel_renders_of_the_same_label_all_succeed` |

Golden vectors (`fixtures/tempo_map_vectors.json`) lolos di Python, TS, dan Rust, jadi rumus waktu ↔ beat konsisten di ketiganya.

## 2. Akurasi sebelum vs sesudah (sintetis, `experiments/repro.py`)

| Kasus | Beat 1 lama | Beat 1 baru | Section (±0,1 s) lama → baru | BPM baru | Error klik di akhir |
|---|---|---|---|---|---|
| A mulai di t=0 | salah | **salah 1 bar** (2,53 vs 0,0) | 0/7 → **0/7** | 96,003 | 24 ms |
| B lead-in 2,3 s | salah 3–9 bar | ✓ | 0/7 → 7/7 | 96,002 | 26 ms |
| C pickup 1 beat | salah | ✓ | 0/7 → 7/7 | 96,004 | 24 ms |
| D intro pad lembut | salah | ✓ | 0/7 → 7/7 | 72,002 | 22 ms |
| E intro 2 bar | salah | ✓ | 0/7 → **0/7** | 96,004 | 23 ms |
| F 71,8 | ✓ | ✓ | 7/7 | 71,799 | 28 ms |
| H 72,15 (dulu dibulatkan 72, drift ±400 ms) | drift | ✓ | 7/7 | 72,152 | 23 ms |
| I bridge tanpa drum | ✓ | ✓ | 7/7 | 95,985 | 43 ms |
| G 124,6 + noise | salah | ✓ | 0/7 → 7/7 | 124,603 | 30 ms |

Ringkasnya:
- Beat 1 sekarang benar di 8 dari 9 kasus (dulu 2 dari 7).
- Section benar di 7 dari 9 kasus.
- Pembulatan BPM sudah hilang.
- Masih ada bias konstan +22–30 ms karena latensi onset belum dikalibrasi.

**Tempo berubah (`experiments/repro_tempo.py`):**
- K konstan 71,8: ✓.
- S step 72→76 di bar 37: **gagal**. BPM terdeteksi 145,8 (oktaf ganda), Beat 1 salah, grid hanya 1 titik.
- R ritardando 76→66: **gagal**. Grid hanya 1 titik, error klik di akhir −3,56 s.

Artinya, deteksi tempo map otomatis belum bekerja.

**Data lagu asli di DB Mac (read-only):**
Kedua lagu terakhir dianalisis dengan pipeline lama atau setengah jalan.
- *Never Lost* (6/8) diberi label `changing`, stability 0,1, dan "18 tempo regions". Padahal semua region bernilai 123,047 BPM. Ini false positive dari `_classify_stability`.
- Beat 1 terdeteksi di 0,34 s, sedangkan yang dikoreksi user 4,16 s.
- *In Jesus Name*: Beat 1 terdeteksi di 2,39 s, sedangkan koreksi user 4,62 s (sekitar 1 bar).
- Downbeat confidence di kedua lagu 0,002–0,011.
- Log aplikasi: error "cue render failed" terakhir tercatat jam 12:37 (kode lama), belum muncul lagi sejak itu.

## 3. Status vs REVIEW.md / roadmap / spec tempo

| Item | Status | Bukti |
|---|---|---|
| Musical start via RMS, Beat 1 untuk lead-in/pickup/intro lembut | **Fixed** | tempo.py:168-185, 368-379 |
| Pembulatan BPM | **Fixed** (0,001) | tempo.py:321-323, prep.rs:439-441 |
| Koreksi oktaf | **Partial** (konstan oke; step tempo masih oktaf ganda) | tempo.py:109-144 |
| Section mengikuti Beat 1 | **Fixed** | sections.py:11-20 |
| Section phrase grid tetap 4 bar | **Not fixed** (kasus E) | sections.py, PHRASE_BARS=4 |
| Race cue render (tmp path bersama) | **Fixed** | cue.rs:80-150, test paralel lulus |
| Satu tempo map untuk klik/cue/count-in di Rust | **Fixed** | tempo_map.rs:146-191, mixer.rs:264-305, player.rs:191-213 |
| Golden vectors lintas bahasa | **Fixed** | fixtures + 3 test |
| Usulan tempo map otomatis (`propose_grid`) | **Not fixed** (selalu 1 titik) | tempo_map.py:152-171, 235-255 |
| `_robust_fit` regresi pada indeks urutan | **Not fixed** | tempo.py:95-106 |
| Klasifikasi stabilitas | **Not fixed** (false "changing") | tempo.py:388-389 |
| Lane tempo editor (titik, step/ramp, drag, snap, undo) | **Mostly fixed** | TempoLane.tsx, SongEditor.tsx:123-178 |
| Inspector BPM/offset vs grid | **Regressed** (lihat #3) | SongEditor.tsx:1033-1040, 978; commands.rs:306-313 |

## 4. Isu terbuka, urut prioritas

1. **App macOS hang tanpa jendela kalau CoreAudio memblokir (P0 robustness).**
   - `core::init`, yang dipanggil dari `main.rs:5` di main thread, menjalankan `open_audio → list_devices → cpal supported_output_configs → audio_unit_from_device`. Lokasi: core.rs:~88 dan audio/engine.rs `list_devices`.
   - cpal mengecek *semua* device, termasuk mic built-in. Ini memicu prompt izin Microphone dari macOS, dan selama prompt belum dijawab main thread berhenti, jadi jendela tidak pernah muncul. Sudah dikonfirmasi lewat `sample` saat `npx tauri dev`.
   - Perbaikan:
     - Buka audio di thread terpisah setelah jendela tampil.
     - Lewati device yang tidak punya output stream sebelum `supported_output_configs`.
     - Tambahkan timeout.
2. **Tempo change tidak terdeteksi.**
   - Lokasi: tempo_map.py:152-171 (`_window_bpms` memberi label bar mulai dari beat pertama yang dilacak, bukan dari Beat 1) dan 235-255 (residual berbasis indeks).
   - Ritardando terlewat (−3,5 s), step 72→76 menyebabkan oktaf ganda dan Beat 1 salah.
   - Perbaikan: jendela berbasis waktu yang di-anchor ke Beat 1, ditambah fit IRLS dengan indeks beat integer, seperti di prototype grid_fit.
3. **Edit BPM/offset di panel Info diam-diam di-revert kalau lagu punya grid.**
   - Lokasi panel: SongEditor.tsx:1033-1034 (NumInput dan TapTempo), ÷2/×2 di ~1039, NumInput offset di 978, dan tombol "Use this" di ~1000. Semuanya hanya mengubah `bpm`/`beatOffset`.
   - Lokasi save: `save_song`, lalu `sanitize_song` (commands.rs:306-313) menimpa bpm dan beat_offset dengan nilai dari `grid`.
   - Perbaikan: semua kontrol tersebut harus lewat `writeGrid` (SongEditor.tsx:130).
4. **Klasifikasi stabilitas false positive.**
   - Lokasi: tempo.py:388-389. Fungsi ini memakai beat yang sudah di-mask dan menandai "changing" padahal BPM di semua region sama.
   - Terbukti di data asli: *Never Lost*, 18 region × 123,047 BPM.
   - Akibatnya muncul warning dan badge merah palsu.
5. **Lagu yang mulai di t=0 dapat Beat 1 terlambat 1 bar, dan intro 2 bar merusak section.**
   - Lokasi: tempo.py:160-165 (`_detect_audio_start` berbasis onset) dan 181 (musical start dijepit ke audio_start), serta sections.py dengan PHRASE_BARS=4.
   - Perbaikan: ambil musical start dari RMS sejak frame 0, dan buat phrase grid yang menyesuaikan dengan novelty, bukan kelipatan 4 bar yang kaku.
6. **Rust kecil-kecil:**
   - `to_beat_grid` membangun tabel dari t=0, jadi klik pre-roll sebelum Beat 1 ikut berbunyi. Lokasi: tempo_map.rs:171-184, mixer.rs:702-712. Perlu dicek apakah ini memang disengaja.
   - `SetBeatTable` men-drop Vec lama di audio thread. Lokasi: mixer.rs:503-510. Sebaiknya dikirim ke thread `wpe-audio-gc`.
   - `cue_lead_frames` fallback masih memakai periode tetap. Lokasi: player.rs:480-483.
   - Bias klik +25 ms belum dikalibrasi.
   - UI kecil: snap ⌘ "free" di lane tidak berfungsi karena `barBeatOf` membulatkan ke beat (tempoMap.ts:49-62), dan Rust `TempoPoint.beat` bertipe u32 (models.rs:135).

## 5. Uji jalan aplikasi di Mac

- `npx tauri dev` berhasil build dan start. Vite berjalan di :1420, log ada di `~/react/wpe-dev.log`.
- Log hanya berisi "Starting…" dan tidak ada error atau warning.
- Jendela **tidak tampil** karena isu #1: proses menunggu prompt Microphone. Prompt ini dialamatkan ke aplikasi yang menjalankan perintah, yaitu Grok Bot.
- Screenshot UI editor tidak bisa diambil, karena saya tidak bisa mengklik prompt itu.
- Jika dijalankan dari Terminal atau Cursor yang sudah punya izin mic, kemungkinan jendela langsung tampil. Namun hang di main thread tetap merupakan bug.
