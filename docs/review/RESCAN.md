# RESCAN — Worship Playback Engine (UI di browser mode vs kode)

## Status implementasi (13 Okt 2026, lanjutan agent)

| Item | Status |
|------|--------|
| P0 konfirmasi keluar Live | Done |
| P1 Esc=PANIC hanya Live, layout tanpa BPM, Inspector BPM/Beat 1, warning basi | Done |
| P2 AI lock grid, toast tombol, stem overlay, Space→load+play, fmtDate Live | Done |
| P2 toggle ID/EN | Done (Settings → Display language; dates/relTime; UI mostly EN) |
| P3 mock desktop-only toast, harness di `src/dev/` | Done |
| §4 zoom max 32×, Snap menu, shortcuts `?` | Done |
| Hapus IPC: `getSong`, `toggle`, `status`, `unload`, `appPaths`, `lockBeatGrid` | Done |
| §4 preset custom UI, hapus IPC lain, full i18n UI | Belum / out of scope |

**Snapshot:** diambil dari Mac pada 13 Okt 2026 14:39 WIB.
- `git status --porcelain` = 60 entri. Semuanya belum di-commit sejak 7c6712c (7 Okt).
- File terakhir diubah 11 Okt 17:44 (`package.json`, `tempo-hit-harness.tsx`). Tidak ada agent lain yang sedang aktif mengedit.

**Cara uji:**
- Vite dev di box dengan mock bawaan `src/dev/mock.ts`, sama seperti `localhost:1420` di Mac.
- Puppeteer 1440×900 membuka 12 rute dan 6 editor lagu.
- Setiap tombol di setiap halaman diklik satu per satu (sweep 285 tombol), lalu dicatat efeknya: DOM berubah, toast, dialog, atau pindah rute.
- Alur yang dicoba: drag tempo, buka laci Mix, Import, menu baris lagu, Live (play/next).
- **Console error: 0. Page error: 0.**

## 1. Penilaian jujur

**Kekuatan**
- **Visual setingkat produk komersial.** Hierarki jelas, warna status konsisten (Live-ready hijau, Needs review kuning, Error merah), dan dark theme rapi. Dashboard langsung menjawab pertanyaan "apa ibadah berikutnya, apakah siap".
- **Paritas IPC 100%.** 69 perintah di `lib/ipc.ts` sama persis dengan 69 command yang terdaftar di `src-tauri/src/lib.rs`. Tidak ada tombol yang memanggil command yang tidak ada.
- **Live mode kuat.** Tampilan NOW/NEXT besar, section bisa diantrekan (klik = di akhir section, Shift = sekarang), mute per stem, meter FOH/MON, PANIC selalu terlihat, dan pintasan keyboard ditampilkan.
- **Banyak perbaikan dari TEMPO-BUG sudah masuk.**
  - Titik tempo bar 1.1 sekarang bisa di-drag (hit-test mengenai dot, 128 → 132 BPM).
  - Header GRID mengikuti tempo map.
  - Panel SongInfo memakai `commitBpm`/`commitBeat`/`applyMeter` (`SongEditor.tsx:810`).
  - Revert mengirim ulang grid (`SongEditor.tsx:775-783`).
  - Lock grid dan `Waveform.tsx` dihapus.
- **Pre-service check** di Setlist dan halaman Diagnostics ("Is this Mac ready for Sunday?") adalah ide yang sangat tepat untuk gereja.

**Kelemahan**
- **Keandalan hari Minggu.**
  - Tombol "← Setlist" dan "End live" langsung *menghentikan audio* tanpa konfirmasi.
  - Esc = PANIC berlaku global, termasuk di editor.
  - Dua hal ini paling berbahaya.
- **Editor terlalu padat untuk worship leader.**
  - Ada 3 tempat untuk BPM/Beat 1/meter: header pill, Inspector Beat Grid, dan laci Mix.
  - Ada 10+ pintasan, istilah DAW (Snap, Step/Ramp, pre-roll), dan zoom 1–256×.
  - Kuat untuk operator teknis, berat untuk music director.
- **Bahasa campur aduk.** UI 100% Inggris, sementara contoh AI berbahasa Indonesia ("Drummer tidak main…", "kunci beat grid"). Format tanggal ada 3 macam.
- **Browser mode menyembunyikan 26 command** yang tidak di-mock (import, prepare, delete, test signal, dll.). Mock mengembalikan `null`, jadi tombolnya terlihat mati saat demo.

## 2. Tabel per halaman

| Halaman | Status | Real (Tauri) | Hanya jalan di mock / no-op di browser | Catatan + bukti |
|---|---|---|---|---|
| Dashboard | **solid** | `readiness`; data dari store | Import songs (dialog plugin tidak di-mock) | Next service, stats, Preparing now. `Dashboard.tsx:138, 212` |
| Songs | **solid** | delete/duplicate/pause, menu baris (Edit, Prepare again, Re-analyze, Duplicate, Show in Finder, Delete…) | Import songs, Prepare (`prepare_songs` tidak di-mock) | Filter, sort, kolom stems V/D/B/O. `Songs.tsx` |
| Song Editor (siap) | **partial → hampir solid** | `save_song`, `set_grid`, `set_downbeat`, play/seek/loop/track/click/cue, `rerender_backing`, `ai_song_command` | — | Drag tempo OK. Kontrol duplikat (§4). Space tidak memutar sebelum lagu di-load (`transport.ts:18-20`). |
| Song Editor (tanpa BPM, mis. *Still Waters*) | **broken (layout)** | sama | — | Lane Tempo tidak dirender saat `grid == null` (`SongTimeline.tsx:359`) tanpa placeholder 96 px. Semua lane naik satu baris: "Double-click to add a section" muncul di baris Tempo, waveform di baris Cues/Click, dan Guide vocal tanpa lane. Tombol "Step" mati. |
| Laci Mix (di editor) | **partial** | count-in, cue lead, key, meter, Beat 1, BPM (sudah lewat grid), preset/backing, arrangement, AI | AI proposal "Beat grid → locked" | Duplikat Inspector. AI masih punya `beat_grid_locked` (`ai.rs` 17 ref, contoh "kunci beat grid" di `SongEditor.tsx:844`), padahal UI/engine sudah tidak memakainya, sehingga ada perubahan yang ditampilkan tapi diabaikan. |
| Setlists | **solid** | save/duplicate/delete/export/import | Import/Export (dialog) | `Setlists.tsx` |
| Setlist editor | **solid** | `save_setlist`, `run_preflight`, `ai_setlist_command` | AI setlist (tidak di-mock) | Reorder dnd, transisi Auto/Stop, override per item. Tanggal "10/16/2026" (input native). |
| Live (pilih setlist) | **partial** | `live_open` | — | Tanggal mentah "2026-10-16" (`Live.tsx:435`). Tidak menyorot ibadah hari ini/berikutnya. |
| Live (berjalan) | **solid, dengan 2 risiko** | play/jump/loop/track/click/cue/fade/panic/step | — | "← Setlist" dan "End live" memanggil `endLive` (stop + close) tanpa konfirmasi (`Live.tsx:97-102, 117, 122`). |
| Preparation | **solid** (real) | `cancel_job`, `list_resources`, event job | Cancel (tidak di-mock) | Tahapan pipeline jelas. |
| Devices | **solid** (real) | `list_audio_devices`, `save_audio_settings`, `test_signal` | Test FOH/MON/click/cue tidak di-mock | Tidak ada indikator test mana yang sedang bunyi (`Devices.tsx:204`, fire-and-forget). Rescan tanpa umpan balik (`Devices.tsx:23, 69`). |
| Settings | **solid** | settings, voices, preview cue, export/import library, `ai_status` | Change…, Export, Import (dialog) | "Test connection" tanpa toast jika hasilnya sama (`Settings.tsx:25-33, 156`). |
| Diagnostics | **solid** | readiness, resources download/cancel/remove, logs, python env | Show in Finder/Source (opener) | Re-check tanpa umpan balik jika tidak ada perubahan. |

**Ada di backend tapi tanpa UI** (fungsi ipc tidak dipakai di mana pun):
- `savePreset` / `deletePreset`: preset kustom sudah didukung backend tapi tidak bisa dibuat dari UI.
- `lockBeatGrid`: tersisa di backend, AI, dan mock.
- `getSong`, `toggle`, `status`, `unload`, `appPaths`.
- `onboarding_done` (`settings.rs:24`): tidak ada alur onboarding.

## 3. 10 isu teratas

1. **P0. Keluar dari Live menghentikan musik tanpa konfirmasi.** Tombol back "← Setlist" adalah `endLive` (`Live.tsx:117` → 97-102: `ipc.stop()` + `liveClose`). "End live" (122) juga sama. Satu salah klik di tengah lagu langsung membuat ruangan sunyi. Perbaikan: konfirmasi ("Lagu sedang diputar — akhiri?") atau tahan 1 detik. Back seharusnya hanya navigasi, audio tetap jalan, dengan MiniTransport.
2. **P1. Esc = PANIC berlaku global, termasuk di editor dan laci Mix.** Listener capture di `useShortcuts.ts:15-20` dengan `panic: "Escape"` (`shortcuts.ts:20`). Laci Mix bukan `role=dialog` (`SongEditor.tsx` ~795), jadi Esc untuk menutup laci atau membatalkan pilihan titik tempo memicu PANIC. Batasi Esc=PANIC hanya di Live, dan gunakan Shift+Esc / tombol di halaman lain.
3. **P1. Layout editor rusak untuk lagu tanpa BPM** (`SongTimeline.tsx:359`). Render placeholder setinggi `H.tempo` ("Set a tempo or tap tempo"). Lihat screenshot `03-song-6-still-waters.png`.
4. **P1. Banyak sumber BPM/Beat 1/meter di UI.** Ada di header pill, Inspector Beat Grid (`SongTimeline.tsx` ~688-720), laci Mix "First downbeat" + "Tempo" (`SongEditor.tsx` ~950-1070), dan meter ganda (ruler kiri + Inspector + laci Mix). Sekarang datanya konsisten, tapi beban kognitifnya tinggi. Gabungkan ke Inspector saja.
5. **P1. Peringatan analisis membeku dan bertentangan.** Contoh *Open Heaven Skies*: banner "BPM 72 detected with low confidence", padahal tempo 96 dan Confidence 94%, ditambah "GRID Changing" lama. `analysis.warnings` berupa string statis dan hanya dihapus lewat "Tempo is correct". Tandai warning sebagai basi saat grid diedit user.
6. **P2. AI masih bisa mengusulkan "Beat grid → locked"** (`ai.rs`, contoh `SongEditor.tsx:844`), padahal fitur itu sudah dihapus. Hapus `beat_grid_locked` dari skema AI dan dari contoh prompt.
7. **P2. Bahasa dan format tidak konsisten.**
   - UI Inggris, AI Indonesia.
   - Tanggal: "Fri, Oct 16, 2026" (Dashboard), "10/16/2026" (Setlist), "2026-10-16" (`Live.tsx:435`).
   - Pilih bahasa UI (idealnya ada toggle ID/EN) dan satu `fmtDate`.
8. **P2. Tombol tanpa umpan balik.**
   - Rescan, Re-check, "Run" pre-service, Test connection: tidak ada toast bila hasilnya sama.
   - Test FOH/MON/click/cue tanpa status "sedang bunyi".
   - Tombol "Step" di lane saat tidak ada grid.
   - Space di editor tidak melakukan apa pun sebelum lagu di-load (`transport.ts:18-20`); seharusnya me-load lalu memutar.
9. **P2. Lane stem untuk lagu yang belum dipisah** menampilkan puncak full mix berwarna seolah stem sudah ada (*Rise Again*, *Still Waters*). Redupkan atau tulis "Prepare to separate".
10. **P3. Browser mode tidak representatif dan ada sisa harness.**
    - 26 command tidak di-mock (`mock.ts:211`, `default: return null`), sehingga Import/Prepare/Delete/Test terlihat mati saat demo.
    - `tempo-hit.html` dan `src/tempo-hit-harness.tsx` ada di `src/` dan ikut `tsc`. Pindahkan ke `src/dev/`.
    - Tambahkan mock minimal (toast "Desktop only") untuk command yang tidak di-mock.

## 4. Yang sebaiknya dihapus atau digabung

- **Gabung:**
  - Laci Mix "First downbeat · Intro entry" + "Tempo" dimasukkan ke Inspector Beat Grid.
  - Laci Mix sisakan Notes, Count-in, Backing/preset, Arrangement, dan AI. Ganti namanya menjadi "Song settings".
- **Gabung:** pemilih meter di ruler kiri dan Inspector, cukup satu.
- **Hapus:**
  - `lockBeatGrid` (ipc, `commands.rs`, `ai.rs`, mock).
  - ipc yang tidak terpakai: `getSong`, `toggle`, `status`, `unload`, `appPaths`.
  - Atau, kalau dipertahankan, beri UI preset kustom (`savePreset`/`deletePreset`).
- **Sederhanakan:**
  - "Snap every marker to the bar" cukup jadi item menu, karena resnap sudah otomatis.
  - Zoom 1–256× cukup sampai ~32× untuk lagu 3–6 menit.
  - Kotak Shortcuts di Inspector dipindah ke popover "?".
- **Pertahankan dan tonjolkan:** Pre-service check, Diagnostics "ready for Sunday", NOW/NEXT di Live.

## 5. Sudut pandang music director (alur Import → Prepare → Edit → Setlist → Live)

- **Import → Prepare:** jelas. Badge status dan antrean dengan tahapan sangat membantu. Prepare berjalan di hari kerja; Live tidak bergantung padanya.
- **Edit:** ini titik lemahnya. Untuk tugas yang biasanya cukup "cek klik pas, nama section benar", editor menyajikan DAW lengkap. Rekomendasi: mode "Simple" (Beat 1, Tempo, section) sebagai default, dan mode Pro untuk tempo map/zoom.
- **Setlist:** sangat baik. Transisi Auto/Stop, notes, pre-service check, AI.
- **Live:** sangat baik secara visual. Perlu pengaman keluar (P0) dan Esc (P1). Pertimbangkan juga "lock screen" saat ibadah, agar tidak ada navigasi tak sengaja.
- **Polish Apple-level:** spasi, tipografi, dan warna konsisten. Yang masih kurang: format tanggal, umpan balik tombol, layout lagu tanpa BPM, dan kepadatan editor.

## Screenshot terpilih (`/workspace/wpe4/shots/`)

- `01-dashboard.png`: kualitas visual dan IA yang baik.
- `03-song-3-open-heaven-skies.png`: warning "BPM 72" dan Confidence 94% vs tempo 96, plus kontrol duplikat.
- `03-song-6-still-waters.png`: layout lane rusak tanpa BPM.
- `12-editor-mix-drawer.png`: laci Mix menduplikasi Inspector.
- `05-setlist-sl-1.png`: setlist dan pre-service check (yang terbaik).
- `17-live-playing.png`: Live; tombol "← Setlist"/"End live" di kiri atas menghentikan musik.

Semua screenshot lain ada di folder yang sama: 01–18. Data mentah: `scan1.json`, `sweep.json`, `sweep.log`.
