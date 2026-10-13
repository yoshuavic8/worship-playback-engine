# Review: Worship Playback Engine (analisis tempo/beat 1 + Song Editor)

*Snapshot yang di-review: salinan source di `/workspace/wpe/Worship Playback Engine` (diambil 9 Okt 2026 ±12:40 WIB).
Catatan penting: saat review ini dibuat, ada agent Cursor lain yang sedang mengedit repo yang sama (branch `main` dengan perubahan yang belum di-commit). Agent itu baru saja menambah zoom timeline (1×–256×, tombol "Beat 1") dan sedang mengerjakan mixer per-track di Live mode. Jadi di beberapa bagian snapshot ini mungkin sudah ketinggalan. Bagian yang mungkin sudah dikerjakan saya tandai **(verify)**.*

---

## TL;DR

Masalahnya **bukan** karena "AI-nya kurang pintar". Ada **6 bug struktural** di `tempo.py`, `sections.py`, dan cara Rust memakai hasilnya. Akibatnya, lagu yang sebenarnya sangat mudah pun (click-recorded, tempo konstan) tetap gagal. Saya buktikan dengan audio sintetis yang ground truth-nya diketahui pasti:

| Kasus (sintetis, ground truth diketahui) | Engine sekarang: Beat 1 | Prototype baru (`experiments/grid_fit.py`) |
|---|---|---|
| A. Lagu mulai di 0:00, intro 4 bar tanpa drum | **meleset 5 bar** (12.53 s, harusnya 0.00) | 0.020 s ✅ |
| B. Lead-in hening 2.3 s | **meleset 5 bar**; section 0/7 tepat | 2.320 s ✅ |
| C. Lead-in + pickup 1 ketukan | kebetulan benar (2.33 s) | 2.320 s ✅ |
| D. Ballad 72 BPM, intro pad 8 bar | **meleset 9 bar** (31.5 s, harusnya 1.5) | 1.518 s ✅ |
| E. Intro 2 bar | meleset 3 bar; **section 0/7 kena** | 0.820 s ✅ |
| H. 72.15 BPM | BPM **dibulatkan jadi 72** → click geser **~400 ms (setengah ketukan)** di akhir lagu | 72.150, error di akhir 18 ms ✅ |

Semua prototype punya bias konstan +20 ms (latensi onset envelope). Itu tinggal dikalibrasi.
Model `beat_this` (CPJKU, ISMIR 2024) sudah saya install dan jalankan di box. **Downbeat pertama di keempat kasus yang saya uji (B, C, D, H) tepat 100%**, termasuk intro pad tanpa drum. Tapi BPM median mentahnya terkuantisasi frame (96.77 untuk 96, 71.43 untuk 72) dan ada beat palsu di bagian hening. Jadi jawabannya: **neural tracker untuk phase/downbeat + robust grid fit untuk BPM presisi.**

Untuk editor: masalah utamanya bukan styling, tapi **model mentalnya**. Grid (BPM + Beat 1) adalah fondasi semua hal, tapi di UI posisinya tersebar di 3 kartu di sidebar kanan, dengan 2 model save yang berbeda dan tanpa undo. Section diedit lewat form dan 9 tombol nudge berbasis milidetik, bukan lewat bar. Sasarannya: **satu timeline horizontal dengan ruler bar.beat, section sebagai clip yang snap ke bar, Beat 1 sebagai handle, cue lane, stem lanes.** Mockup ada di `editor-mockup.png`.

---

## A. Akar masalah analisis (dengan bukti)

File yang saya baca end-to-end: `audio-worker/src/wpe_worker/analysis/tempo.py`, `sections.py`, `pipeline.py`, `tests/test_analysis.py`, `testaudio.py`, ditambah konsumennya di Rust (`prep.rs`, `player.rs`, `audio/click.rs`, `audio/arrangement.rs`).
Script reproduksi: `/workspace/wpe/experiments/repro.py` (engine sekarang), `compare.py` + `grid_fit.py` (prototype), `try_beat_this.py`. Hasil mentahnya ada di `repro_results.json` dan `compare_results.json`.

### A1. Beat tracker mengunci di setengah tempo, tapi yang dikoreksi cuma angkanya, bukan beat-nya 🔴
- `tempo.py:260-265` memakai `librosa.beat.beat_track` pada onset **full mix** (median-aggregated).
- Di semua kasus uji (librosa 1.0), tracker mengunci di **½ tempo**: interval 1.25 s untuk lagu 96 BPM, hanya 96 dari 224 beat yang terdeteksi.
- `tempo.py:285-288` lalu menggandakan **angka BPM** (`while bpm < 58: bpm *= 2`). Hasilnya terlihat benar ("96 BPM"), padahal array `beats` (`tempo.py:378`) tetap berisi beat setengah tempo, lalu ditulis ke `analysis.json` (`pipeline.py:213-215`).
- Begitu user menekan **"Lock grid"**, `player.rs:205-225` + `click.rs:179-235` (`from_beats`) memakai array mentah itu. Click jadi bunyi setengah tempo, dan aksen (setiap 4 beat tracked) jatuh **setiap 2 bar**. Fitur yang dimaksudkan sebagai jalan keluar untuk lagu live justru merusak click.
- `tempo.py:311-313` (`confidence`) dihitung terhadap beat setengah tempo ini, dan setelah BPM dibulatkan. Pada kasus H confidence jatuh ke 0.267 walaupun lagunya 100% konstan.

### A2. Beat 1 harus menunggu kick/bass, jadi intro tanpa drum dilompati 🔴
- `_detect_musical_start` (`tempo.py:130-167`) mensyaratkan onset broadband × onset **<250 Hz** (`combined = onset_all * onset_low`, baris 156) > 25% dari puncak lagu (baris 160).
- Intro worship biasanya pad/piano/gitar tanpa kick, jadi `musical_start` = **saat drum masuk** (kasus A: 10.66 s, kasus D: 29.0 s).
- `tempo.py:349-355` lalu **memajukan Beat 1 per bar sampai melewati musical_start**. Akibatnya Beat 1 = bar ke-2 verse. Inilah persis keluhan "gagal mendeteksi intro / beat pertama".
- Test yang ada menyembunyikan bug ini: `test_analysis.py:40-42` hanya mengecek `beat_offset % bar`, jadi error 5 bar pun dianggap lulus.

### A3. Fase bar dihitung dari titik nol yang arbitrer 🟠
- `phase = intercept % period` (`tempo.py:310`) membuat grid mulai dari t≈0 (masih di area hening). Fase bar `j` dipilih dari rata-rata kick+chroma-flux per posisi (`tempo.py:326-344`), dengan bias "pilih yang paling awal kalau skornya ≥92%" (baris 341-344).
- Karena j=0 di sini tidak punya makna musikal, bias itu sama saja dengan lempar koin antara beat 1 dan beat 3, terutama saat kick main di 1 dan 3 (pola paling umum di worship).
- `downbeat_confidence` (baris 346-347) selalu ~0.2 di semua kasus uji, jadi angka ini tidak informatif dan UI tidak bisa memakainya untuk memutuskan apa pun.

### A4. BPM dibulatkan dua kali, jadi click drift di akhir lagu 🔴
- `tempo.py:303-304`: kalau |bpm − round(bpm)| < 0.2, BPM dibulatkan ke integer.
- `prep.rs:439`: dibulatkan lagi ke 0.1 sebelum disimpan ke `song.bpm`.
- Dalam mode default (grid tidak dikunci), `player.rs:247` memakai `BeatGrid::new(bpm, offset)`, yaitu rumus tetap. Setiap error BPM jadi **drift linear**. 72.15→72 menghasilkan ±400 ms di akhir lagu 3 menit (terukur di kasus H). Inilah "tempo tidak stabil sampai akhir": tempo lagunya sendiri stabil, tapi grid kita yang miring.
- Banyak lagu rekaman dengan click memang tidak bulat (rekaman yang di-resample, varispeed, master yang di-pitch). Jangan pernah membulatkan; simpan 3 desimal.

### A5. `_robust_fit` mengasumsikan tidak ada beat yang hilang 🟠
- `tempo.py:97`: `idx = np.arange(len(times))`. Regresi waktu terhadap **urutan** beat, bukan terhadap **nomor beat sebenarnya**. Begitu tracker melewatkan atau menambah satu beat (breakdown, fill, bagian hening), semua index setelahnya bergeser dan slope (BPM) bias.
- `tempo.py:368-369`: `_classify_stability(times[mask])` membuang outlier lalu memanggil `np.diff`. Celah bekas outlier jadi interval 2× lipat, dan lagu konstan bisa terklasifikasi "changing" dengan puluhan "tempo region" palsu. Jitter kuantisasi frame (hop 512 @22.05 kHz = 23 ms) juga hampir menyentuh threshold `cv < 0.03` (baris 196) dan `jump 35 ms` (baris 203).
- Perbaikannya: regresi terhadap **index integer hasil pembulatan `(t − phase)/P`**. Itu yang dilakukan prototype `grid_fit.py`, dan hasilnya tahan terhadap beat yang hilang (kasus I, bridge tanpa drum: end error 19 ms).

### A6. Section tidak di-anchor ke Beat 1 🔴
- `sections.py:11-21` (`_bar_times`) mengekstrapolasi bar mundur sampai 0 dan **memaksa bar pertama = 0.0** (baris 16-19).
- `sections.py:172`: phrase 4-bar dihitung dari index 0 grid itu, **bukan dari Beat 1**. Dengan lead-in 2.3 s atau intro 2 bar, semua phrase bergeser 1–2 bar terhadap struktur lagu, fitur antar-section tercampur, dan boundary hilang. Hasilnya: kasus B 0/7 boundary tepat, kasus E **0/7 bahkan dalam toleransi 1 bar**.
- `sections.py:189` + `arrangement.rs:31-32`: section pertama selalu mulai di 0.0, jadi "Intro" ikut menelan silence/crowd noise, dan count-in serta cue pertama dihitung dari posisi yang salah.
- Section dianalisis di full mix (`pipeline.py:142`), padahal stem drums/bass/other sudah tersedia (`pipeline.py:90-99`). Stem memberi sinyal "drum masuk/keluar" yang sangat kuat untuk boundary worship (build, breakdown).
- Test `test_sections_find_structure` (`test_analysis.py:76-87`) memberi **BPM dan offset yang benar** (96.0, 0.0), bukan hasil deteksi, jadi integrasinya tidak pernah teruji.

### A7. Hal-hal kecil yang ikut berkontribusi
- Tidak ada kompensasi latensi onset envelope: grid konsisten **+20–30 ms terlambat** (terlihat di semua kasus, engine lama maupun prototype). Kalibrasi sekali dengan click track sintetis.
- `cue_lead_frames` = `frames_per_beat × cue_lead_beats` dihitung dari **start section** (`player.rs:509-511`). Kalau section tidak tepat di bar, cue juga ikut off-beat.
- `click.rs:203-208`: `offset_idx` = beat tracked **terdekat** ke offset. Kalau beat-nya setengah tempo (A1), aksen bisa jatuh di tempat yang salah.
- Test suite tidak punya satu pun kasus lead-in, pickup, intro tanpa drum, atau BPM non-integer. Semua sinyal sintetis mulai tepat di 0.0 (`testaudio.py:48-112`).

---

## B. Arsitektur perbaikan yang direkomendasikan

### B1. Pilihan teknologi (dicek Okt 2026)

| Opsi | Status | Rekomendasi |
|---|---|---|
| **beat_this** (CPJKU, ISMIR 2024) | Install dari GitHub (`pip install git+https://github.com/CPJKU/beat_this`), PyTorch (sudah jadi dependency kalian lewat demucs). Checkpoint `final0` di-download otomatis. **Sudah saya jalankan di box: downbeat pertama tepat di semua kasus uji.** | ✅ **Primary** untuk beat + downbeat. Jalankan tanpa DBN, lalu lakukan grid fit sendiri. Lisensi checkpoint perlu dicek dulu sebelum distribusi komersial. |
| madmom DBN downbeat | Rilis PyPI lama tidak mau build di Python ≥3.10; harus dari git. | Fallback saja. Jangan jadi dependency utama. |
| **allin1** (beats + downbeats + sections sekaligus) | v1.1.0 masih ada di PyPI, tapi bergantung pada NATTEN yang API-nya sudah berubah (issue #30, open). Butuh madmom dari git. Rapuh. | ⏸ Defer. Bagus sebagai referensi atau eksperimen offline untuk label section; terlalu rapuh untuk dibundle di app Mac. |
| librosa saja | Sudah ada. Prototype saya membuktikan **grid fit yang benar + librosa** sudah lulus semua kasus sintetis. | ✅ **Fallback** kalau model belum di-download (konsisten dengan prinsip "model manager" di master prompt). |

### B2. Pipeline baru (per langkah)

```
mix + stems (drums, bass, other) ──► [1] beat/downbeat candidates (beat_this on mix; fallback librosa on drums+bass)
                                   ──► [2] constant-tempo grid fit (IRLS on integer beat index)  → bpm (3 desimal), phase, residual
                                   ──► [3] octave + bar phase decision (downbeat activations + kick/bass + chord change)
                                   ──► [4] Beat 1 = first bar line ≥ first sustained musical energy (ANY instrument) – ½ beat
                                   ──► [5] mode: fixed | tempo-map (only if residual/drift > threshold)
                                   ──► [6] sections on a BAR grid that starts at Beat 1, phrase = 4/8 bars from Beat 1, stems features
                                   ──► confidences + "why" strings for the UI
```

**[2] Constant-tempo grid fit.** Sebagian besar lagu worship direkam dengan click, jadi model terbaiknya adalah **satu BPM + satu phase**:
1. Tempo kasar dari autokorelasi onset (atau median interval beat_this).
2. Untuk tiap kandidat oktaf {T, 2T, T/2}, **search periode ±0.8%** (autokorelasi terkuantisasi lag ±0.3%; tanpa langkah ini ballad 72 BPM salah jadi 143.5 di prototype awal saya).
3. Assign tiap onset/beat ke **index integer** `k = round((t − φ)/P)`, lalu regresi `t = φ + kP` dengan bobot (IRLS/Huber). Beat yang lemah atau hening dilewati **tanpa menggeser index**.
4. Output: `bpm` (float, jangan dibulatkan), `phase`, `residual_ms` (median |r|), `inlier_ratio`.
Kode referensinya ada di `/workspace/wpe/experiments/grid_fit.py` (±120 baris, sudah teruji di 9 kasus).

**[4] Beat 1.** Beat 1 = bar line pertama ≥ (awal energi musikal *apa pun* − ½ beat), dengan RMS > −30 dB dari puncak, **tanpa syarat kick**. Pickup 1–3 ketukan otomatis tertangani karena bar line sebelum pickup berada sebelum energi pertama. Kalau ada beat_this, pakai downbeat pertamanya sebagai kandidat utama dan cek konsistensinya terhadap grid.

**[5] Tempo map hanya kalau perlu.** Kalau `residual_ms > ~15` atau drift sistematis (fit kuadratik signifikan), baru pecah jadi region per 8–16 bar. Jangan per jump antar interval seperti `_classify_stability` sekarang. Untuk lagu live tanpa click, simpan `beats[]` yang **sudah dibersihkan dan sudah di-snap ke metrical level yang benar**, bukan output mentah tracker.

**Confidence yang berarti** (untuk UI):
- `tempoConfidence` = f(residual_ms, inlier_ratio, rasio comb oktaf terpilih vs alternatif)
- `beat1Confidence` = margin skor fase bar terbaik vs kedua + kesepakatan beat_this vs heuristik
- `sectionConfidence` per boundary (novelty peak height)
- Tambahkan string penjelasan: "Drum masuk di bar 5, intro 4 bar terdeteksi", "Ada 2 kandidat tempo: 72 / 144".

### B3. Perubahan konkret per file

| File | Perubahan |
|---|---|
| `analysis/tempo.py` | Ganti `detect_tempo` dengan `detect_grid(mix, stems, bpb)` → `{bpm, beat1, phase, residual_ms, mode, tempo_map, beats_clean, confidences, explain}`. Hapus pembulatan (303-304). `_robust_fit` → regresi index integer. `_detect_musical_start` → RMS any-instrument. Kalau BPM dikoreksi oktaf, **array beats juga harus dikoreksi** (interpolasi atau desimasi). Kalibrasi latensi −20 ms. |
| `analysis/beats_nn.py` (baru) | Wrapper beat_this (lazy load, device mps/cpu, checkpoint via Model Manager). Return beats + downbeats + activations. Fallback ke librosa kalau model tidak ada. |
| `analysis/sections.py` | `_bar_times(beat1, bpm/tempo_map)` mulai **dari Beat 1**, tanpa ekstrapolasi ke 0. Tambah section "Pre-roll" (0 → Beat 1) yang tidak ikut arrangement. Phrase dihitung dari Beat 1. Fitur tambahan dari stems (energi drums on/off, bass on/off). Output start dalam **bar index** + detik. |
| `pipeline.py` | Pindahkan analisis **setelah separation** dan kirim stems (`pipeline.py:132-142`). Tulis `grid` sebagai satu objek di `analysis.json`. Hapus `beatGridLocked` sebagai keputusan algoritma. |
| `tests/test_analysis.py` | Tambahkan kasus dari `experiments/repro.py` (lead-in, pickup, soft intro, 2-bar intro, 72.15, breakdown). Assert **Beat 1 absolut** (bukan modulo bar), drift di akhir lagu < 10 ms, section boundary ±0.1 s. |
| `prep.rs:438-441` | Jangan bulatkan BPM. Simpan objek `grid` utuh ke `Song` (bukan hanya `bpm` + `beat_offset`). |
| `player.rs:193-247` + `click.rs` | **Satu sumber kebenaran**: `BeatGrid::from_grid(grid)`. Mode fixed pakai rumus dengan BPM float. Mode map/beats pakai tabel. Aksen dihitung dari bar index grid, bukan "beat terdekat". Edit BPM/Beat 1 dari user selalu berlaku (sekarang edit BPM diabaikan diam-diam kalau grid locked). |
| `player.rs:509` | Cue lead = N beat sebelum **bar line** boundary (pakai grid), bukan detik dari start section. |
| `arrangement.rs:31` | Jangan paksa section pertama = 0. Pre-roll sebelum Beat 1 jadi segmen terpisah yang bisa di-skip, dengan count-in yang berakhir tepat di Beat 1. |

### B4. Alur koreksi manusia (harus 10 detik, bukan 10 menit)

1. **Konfirmasi otomatis**: buka lagu → otomatis loop 2 bar pertama dengan click. Ada satu pertanyaan besar: *"Click pas?"* [Ya] / [Geser].
2. **Tap Beat 1**: tekan `T` (atau tombol besar) tepat di ketukan 1 sambil lagu jalan. Ambil tap terakhir, **snap ke beat grid terdekat** (bukan ke milidetik tap), lalu jadikan bar line. Kalau user tap 4× atau lebih, sekaligus refine BPM.
3. **Nudge grid**: `◀ beat / beat ▶` (pindah fase bar, solusi untuk "aksen jatuh di 3"), `◀ 5 ms / 5 ms ▶`, `÷2 / ×2`. Semua live saat playback (click langsung berubah), tanpa save dan reload.
4. **Kandidat**: kalau confidence rendah, tampilkan 2 chip: "72 BPM · Beat 1 di 0:01.5" vs "144 BPM". User cukup memilih, bukan mengetik.
5. **Re-analyze dengan constraint**: setelah user menetapkan Beat 1 dan BPM, re-run section detection dengan grid yang sudah dikunci user (murah, cuma beberapa detik).

---

## C. Song Editor: masalah UX & redesign ala Ableton Arrangement

### C1. Masalah utama (snapshot + screenshot `.tmp/shots/editor.jpg`, `wave.jpg`)

1. **Grid adalah fondasi, tapi diperlakukan sebagai setting.** BPM, Beat 1, time signature, tempo mode, lock grid, dan count-in tersebar di 3 kartu kanan (`SongEditor.tsx:669-936`), terpisah dari waveform di kiri. User harus bolak-balik antara mata dan tangan.
2. **Dua model penyimpanan.** Section/BPM masuk draft dan butuh Save, sedangkan Beat 1 langsung dipersist via `ipc.setDownbeat` (`SongEditor.tsx:264-268, 646-657, 770`). Akibatnya "Revert" tidak mengembalikan Beat 1, dan "Unsaved changes" jadi tidak jujur.
3. **Tidak ada undo** (cuma Revert all, `SongEditor.tsx:367`). Untuk editor direct manipulation ini fatal: orang takut drag.
4. **Edit berbasis detik, bukan musik.** Ada 9 tombol nudge per section (−50 ms, +50 ms, ±beat, ±bar, At playhead; `SongEditor.tsx:514-545`). Arrow key = 50 ms (`:149`). Snap saat drag hanya dengan Shift dan hanya ke beat (`Waveform.tsx:146-151`). "Snap to bars" adalah aksi batch terpisah (`:338-344`). Di Ableton default-nya kebalikannya: snap ke grid aktif, tahan ⌘ untuk bebas.
5. **Beat 1 di-clamp maksimum 30 s** (`SongEditor.tsx:265, 654, 748`; `Waveform.tsx:156-166`). Lagu dengan intro panjang, doa, atau pembacaan ayat di awal tidak bisa di-set dengan benar.
6. **Lock grid membingungkan dan berbahaya.** Saat locked, click memakai beat mentah dan mengabaikan BPM dari user (`player.rs:205-225`), dan beat mentah itu setengah tempo (A1). Hint UI-nya bilang "Studio recording… locking is optional", padahal locking bisa merusak click.
7. **Section = form, bukan objek di timeline.** Rename dan ganti tipe dilakukan lewat input + select di list. Arrangement adalah daftar chip terpisah (`Arrangement.tsx`), dan label di waveform terpotong ("In…", "Pr…" di screenshot).
8. **Zoom dan scroll bertabrakan.** Scroll vertikal dua jari di waveform = zoom (`SongEditor.tsx:192-206`), padahal halamannya sendiri scroll vertikal. Saat mau scroll ke bawah halaman, malah zoom. **(verify: zoom baru saja dikerjakan agent Cursor)**
9. **Tidak ada stem lanes.** Hanya satu waveform (backing). Mute/solo hanya ada di "Rehearsal mixer", yang cuma muncul kalau lagu sudah di-load dan sifatnya "temporary" (`SongEditor.tsx:595, 941-972`). **(verify: mixer per-track sedang dikerjakan di Live mode; idealnya komponennya dipakai bersama)**
10. **Cue tidak terlihat dan tidak bisa diedit.** Cue = TTS dari label section, otomatis N beat sebelum section (`player.rs:466-511`). Tidak bisa menambah "Build", "Break", "All in", "Key up", atau "Ending", dan tidak ada cue Bahasa Indonesia.
11. **Shortcut `M` menaruh marker di posisi 0** kalau lagu belum di-load (`SongEditor.tsx:118-121`, `position ?? 0`).
12. **Kepadatan kognitif.** Satu halaman berisi editor timeline, preset backing FOH, routing monitor, mixer, notes, dan AI assistant. Untuk MD yang sedang membenarkan Beat 1, 80% layar itu noise.

### C2. Redesign: satu timeline (lihat `editor-mockup.png`)

```
┌ Header: Judul · [⏮ ▶ ⟲Loop] · 12.3.1 · Tempo 71.80 ● · Beat 1 2.320s 94% · Grid Fixed ±2ms · Undo · Snap: Bar · Done
├────────────┬──────────────────────────────────────────────────────────────┬──────────────┐
│ Lane heads │ Ruler bar.beat (klik = seek, drag = loop region)              │ Inspector    │
│ Arrangement│ [Intro 4][Verse 1 8][Chorus 1 8][Verse 2 8]…  ← clips         │ (konteks:    │
│ Cues       │   🔊Verse   🔊Chorus   🔊Build↑                ← cue events   │  section /   │
│ Click      │ ▮▯▯▯▮▯▯▯ (aksen terlihat)                                    │  grid / cue) │
│ Drums  M S │ ~~~~~ waveform per stem                                      │              │
│ Bass   M S │ ~~~~~                                                        │              │
│ Guitar M S │ ~~~~~                                                        │              │
│ Keys   M S │ ~~~~~                                                        │              │
│ Vocal  M S │ ~~~~~ (guide, default mute)                                  │              │
└────────────┴──────────────────────────────────────────────────────────────┴──────────────┘
  Beat 1 = handle emas di ruler; area sebelum Beat 1 diarsir ("pre-roll")
```

**Prinsip interaksi:**
- **Ruler dalam bar.beat**, waktu di tooltip. Semua angka yang dilihat MD dalam bar ("Chorus mulai bar 13, 8 bar").
- **Section = clip.** Drag tepi clip untuk memindah boundary (otomatis menggeser tetangganya, snap ke bar). Double-click untuk rename inline. Klik kanan / inspector untuk tipe (segmented control, bukan dropdown). `M` = split di playhead, `⌫` = merge dengan sebelumnya.
- **Snap default = Bar**, ⌥ = Beat, ⌘ = bebas. Pilihan snap ada di header.
- **Beat 1 = handle emas di ruler.** Drag untuk menggeser seluruh grid (waveform tetap, garis bar yang bergerak), `T` = tap Beat 1, inspector Grid punya `÷2 ×2 ◀beat beat▶ ◀5ms 5ms▶`.
- **Playhead + loop**: drag di ruler = loop region, `L` = loop section terpilih (loop chorus untuk cek click adalah use case nomor 1).
- **Zoom**: pinch / ⌘scroll, `Z` = zoom ke selection, `⇧Z` = fit. Scroll dua jari horizontal = pan. Scroll vertikal **tidak** dipakai untuk zoom.
- **Undo/redo** (`⌘Z / ⇧⌘Z`) untuk semua aksi, termasuk Beat 1 dan BPM. Autosave draft. Tombol "Done" hanya menutup editor.
- **Live-apply**: perubahan grid langsung terdengar di click tanpa reload (kirim `MixerCmd::SetGrid`).
- **Arrangement**: MVP = urutan natural + toggle "skip" per clip dan "repeat ×2". Arrangement custom drag-drop (repeat chorus dsb.) ditampilkan sebagai baris kedua "Live order" di bawah clips (defer ke v2).
- Pindahkan preset backing, routing monitor, dan AI assistant ke tab terpisah ("Mix & Routing"). Editor = timeline.

### C3. Prioritas: MVP vs defer

**MVP (yang benar-benar dibutuhkan MD setiap minggu):**
1. Konfirmasi atau perbaiki **Beat 1** (handle + tap + nudge beat/5 ms), tanpa batas 30 s.
2. Konfirmasi atau perbaiki **BPM** (angka 2–3 desimal, ÷2 ×2, tap, kandidat alternatif), langsung terdengar.
3. **Rename / ganti tipe / geser / split / merge section** sebagai clip yang snap ke bar.
4. **Cue lane**: cue otomatis dari section, bisa dihapus, ditambah dari preset kata (Build, Break, All in, Key up, Ending, Tag, Bahasa Indonesia), lead dalam beat/bar.
5. Playhead, loop region, zoom, undo, shortcut inti (Space, T, M, L, C, ⌘Z, ←/→ = 1 bar).
6. Stem lanes dengan **mute/solo saja** (tanpa fader/automation).

**Defer:**
- Edit tempo map per region, warp marker, time signature campuran.
- Automation volume per stem, fade per clip.
- Drag-drop arrangement custom dengan repeat bebas (cukup skip/×2 dulu).
- Waveform per stem resolusi tinggi (cukup peaks ringan), spectrogram.
- MIDI/Ableton Link, AI assistant di editor.

---

## D. Bug cue: "cue render failed: No such file or directory (os error 2)"

Log di screenshot Mac: `[warn] Audio cue render failed: No such file or directory (os error 2)` lalu `In Jesus Name: Cue audio could not be generated.`

**Catatan jalur:** cue **tidak** dirender oleh Python. Python (`rendering/backing.py`) hanya me-mix backing. Cue dirender di Rust: `player.rs:477` → `cue::ensure_cue` (`cue.rs:48-70`) → `/usr/bin/say -o <slug>.tmp.wav` → `rename` ke `<root>/resources/audio/cue/<voice>/<slug>.wav` (`paths.rs:48-50`), lalu `get_buffer` (decode).

**Diagnosis:** pesan error-nya *tanpa prefix*, jadi asalnya `io::Error` yang dinaikkan lewat `?`. Kalau `say` tidak ditemukan, pesannya akan berbunyi "macOS speech synthesis unavailable…" (`cue.rs:64`). Kandidatnya tinggal `create_dir_all` (`cue.rs:57`, sangat jarang ENOENT) atau **`std::fs::rename(&tmp, &path)` (`cue.rs:68`) saat file tmp tidak ada**. Penyebab yang paling mungkin:

1. **Race condition (paling mungkin).** `save_song` (`commands.rs:385-392`) men-spawn thread background yang merender cue untuk semua label. `SongEditor.save()` lalu **langsung** memanggil `loadSong` (`SongEditor.tsx:97-99`), yang memanggil `ensure_cue` untuk label yang sama (`player.rs:477`). Re-render backing dan prepare juga lewat save + load. Dua thread melihat file belum ada (`cue.rs:54`), keduanya menjalankan `say` ke **path tmp yang sama** (`<slug>.tmp.wav`, `cue.rs:58`). Thread pertama berhasil rename, rename thread kedua gagal dengan **ENOENT**. Ini cocok dengan log yang menunjukkan "Re-rendered backing…" dan beberapa kali "Loaded… / Live session started" berturut-turut.
2. **`say` keluar dengan status 0 tapi tidak menulis file**, misalnya voice default `"Samantha"` (`settings.rs:62`) tidak terinstall di macOS terbaru, atau voice "premium/Siri" yang tidak bisa dipakai untuk output file. Hasilnya rename ENOENT juga.
3. Efek samping: satu kegagalan men-set `cue_failed = true` dan **mematikan semua cue untuk lagu itu** (`player.rs:483, 499-505`).

**Cara verifikasi cepat di Mac:**
```bash
say -v Samantha -o /tmp/t.wav --file-format=WAVE --data-format=LEF32@48000 "Chorus"; echo "exit=$?"; ls -la /tmp/t.wav
ls -la ~/Library/Application\ Support/app.worshipplayback.engine/resources/audio/cue/*/ | grep -i tmp   # sisa tmp = jejak race
```

**Quick-win fix (±2 jam):**
- Nama tmp unik: `<slug>.<pid>-<thread-id>-<nanos>.tmp.wav` (atau crate `tempfile` di direktori yang sama).
- Mutex per path (`static CUE_LOCK: Mutex<HashSet<PathBuf>>`) atau satu `Mutex<()>` global. Cue kecil, serialisasi tidak masalah.
- Setelah `say`, cek tmp ada dan >1 KB. Kalau tidak ada, error dengan konteks: `"say produced no audio for voice 'Samantha' (text 'Chorus')"`. Kalau rename gagal **dan** file tujuan sudah ada, anggap sukses.
- Fallback: ulangi tanpa `-v` (voice default sistem) sebelum menyerah. Tampilkan peringatan "Voice X tidak tersedia" di Settings.
- Semua error diberi `.with_context(|| path.display())`.
- Jangan matikan seluruh cue karena satu label gagal; skip label itu saja.
- Render cue di akhir **prepare** (bukan saat load), supaya load live tidak pernah menjalankan TTS.
- Tambah test Rust: 8 thread `ensure_cue` label yang sama secara paralel (cfg macOS).

---

## E. Roadmap prioritas (urut)

| # | Item | Isi | Estimasi |
|---|---|---|---|
| 1 | **Quick wins analisis + cue** | Fix race cue (D). Hapus pembulatan BPM (`tempo.py:303`, `prep.rs:439`). Hapus clamp 30 s Beat 1. `musical_start` tanpa syarat kick. Koreksi array beats saat BPM dikoreksi oktaf, **atau** sembunyikan "Lock grid" sampai #3 selesai. Section phrase dihitung dari Beat 1. | 1–2 hari |
| 2 | **Harness evaluasi** | Masukkan kasus dari `experiments/repro.py` ke pytest. Anotasi 15–20 lagu worship nyata (Beat 1, BPM, boundary section), lalu laporkan metrik (beat1 error, drift akhir, F-measure boundary ±0.5 s) di CI. Tanpa ini, setiap "perbaikan" cuma tebakan. | 2 hari + anotasi |
| 3 | **Grid engine baru** | `grid_fit.py` → `tempo.py` (IRLS index-integer, octave search ±0.8%, Beat 1 any-instrument, kalibrasi latensi). beat_this sebagai primary lewat Model Manager, librosa sebagai fallback. Analisis setelah separation (pakai drums+bass). Confidence + penjelasan. | 4–6 hari |
| 4 | **Satu model `grid` end-to-end** | `analysis.json` → `Song.grid` → `BeatGrid::from_grid` (click, cue lead per bar, count-in tepat di Beat 1, pre-roll segment). Hapus semantik "lock" yang ambigu. Edit user selalu menang. `MixerCmd::SetGrid` untuk live-apply. | 2–3 hari |
| 5 | **Timeline editor MVP** | Ruler bar.beat, arrangement lane (clip snap bar, drag edge, split/merge, rename inline), handle Beat 1 + Tap Beat 1 + nudge, loop region, zoom (pinch/⌘scroll, tanpa scroll-jacking), undo stack, shortcut. Pisahkan Mix & Routing ke tab lain. Prototype canvas yang sudah ada (`Waveform.tsx`) bisa dipakai ulang sebagai renderer lane. **(verify: zoom 1–256× sudah ada dari agent Cursor; lanjutkan, jangan tulis ulang)** | 1.5–2.5 minggu |
| 6 | **Cue lane** | Cue sebagai event eksplisit (`{bar, text, voice}`): auto-generate dari section, bisa diedit, preset kata worship (EN + ID), lead dalam beat/bar, preview. Render saat prepare. | 3–4 hari |
| 7 | **Section detection v2** | Bar-synchronous dari Beat 1, fitur stems (drums on/off, energi bass, chroma), boundary snap ke phrase 4/8 bar, confidence per boundary. Re-run cepat setelah user mengoreksi grid. Eksperimen allin1 offline sebagai pembanding. | 4–6 hari |
| 8 | **Stem lanes mute/solo di editor** | Lane per stem dengan peaks ringan + M/S, routing ke player rehearsal. **(verify: mixer per-track di Live sedang dikerjakan; pakai komponen yang sama supaya tidak dobel)** | 2–4 hari |

Setelah itu: tempo map editing untuk lagu live tanpa click, arrangement custom (repeat/skip drag-drop), input YouTube yang rapi (yt-dlp + cek lisensi), dan voice pack Bahasa Indonesia.

---

### Lampiran: artefak

- `/workspace/wpe/experiments/repro.py`: generator lagu sintetis dengan ground truth + evaluasi engine sekarang (`repro_results.json`).
- `/workspace/wpe/experiments/grid_fit.py`: prototype constant-tempo grid fit + bar phase + Beat 1 (librosa saja).
- `/workspace/wpe/experiments/compare.py`: prototype vs ground truth (`compare_results.json`; 9/9 kasus, error ≈ +20 ms konstan).
- `/workspace/wpe/experiments/try_beat_this.py`: uji beat_this (downbeat pertama tepat di 4/4 kasus; BPM median terkuantisasi, jadi tetap butuh grid fit).
- `/workspace/wpe/editor-mockup.png` (+ sumber `mockup/editor-mockup.html`): mockup timeline editor.
- Test lama (`pytest tests/test_analysis.py`): 9/9 lulus. Artinya test sekarang **tidak** menangkap bug-bug di atas.
- Keterbatasan: semua angka berasal dari audio sintetis. Validasi dengan lagu nyata (roadmap #2) wajib sebelum mengganti engine.
