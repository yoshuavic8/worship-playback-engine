# Smart Tempo Logic Pro: cara kerja, engine, dan pelajaran buat Worship Playback Engine

_Riset 19 Okt 2026 (WIB). Label: ✅ = dikonfirmasi sumber resmi Apple / paper, 🟡 = sumber pihak ketiga yang kredibel, 🔮 = spekulasi/inferensi saya._

## TL;DR
- Smart Tempo (Logic 10.4, 2018) **bukan** pendeteksi transient biasa. Kata Apple, dia memasang beat marker berdasarkan "musical tempo detection", lalu hasilnya bisa dikoreksi user lewat **hint** (downbeat/beat/time-signature) dan **beat marker** yang bisa di-drag. ✅
- Teknologinya diturunkan dari analisis ritme app **Music Memos** (2016). 🟡 (Sound On Sound, ask.video)
- Apple **tidak pernah** bilang engine-nya ML atau DSP klasik. Patent Apple yang relevan (US8983082, diajukan 2010) memakai DSP klasik: envelope → *autocorrelation phase matrix* → filtering → deteksi meter → downbeat. Patent ini bukti arah riset Apple, **bukan** bukti isi Smart Tempo. 🔮
- Kekuatan sebenarnya ada di **UX koreksi**, bukan algoritmanya: hint lalu analisis ulang, tombol /2 ×2, kunci range, multitrack downmix, dan "set downbeat".
- Untuk WPE: pakai **beat_this** sebagai engine utama (SOTA F1, tanpa DBN, tahan tempo drift), librosa cuma buat cadangan, lalu tiru alur koreksinya Logic.

## 1. Yang dikonfirmasi Apple (user guide & support) ✅
1. **Tujuan:** rekam tanpa metronom, Logic menganalisis region, lalu menerapkan tempo region ke project (atau sebaliknya, region di-flex mengikuti project).
2. **Bukan transient:** "Unlike audio quantization, which identifies transient peaks… Smart Tempo analysis places beat markers based on the musical tempo detection."
3. **Mode Project Tempo:**
   - **Keep:** tempo project tetap.
   - **Adapt:** tempo project mengikuti rekaman atau file import.
   - **Automatic:** Keep kalau ada referensi tempo musikal (metronom aktif, region lain, Cycle). Kalau tidak ada, Adapt.
   - Warna kurva tempo: biru = tidak berubah, oranye = akan diisi rekaman baru, merah = akan menimpa tempo yang ada.
4. **Flex & Follow per region:** Off / On / Bars (sejajar di tiap downbeat) / Beats (sejajar di tiap beat), memakai beat marker Smart Tempo.
5. **Smart Tempo Editor:**
   - Beat marker oranye, downbeat ditampilkan lebih tebal.
   - Ada metronom preview.
   - Tap tombol **D** (downbeat) dan **T** (beat) sambil play.
   - Hasil analisis disimpan **di dalam file audio**, jadi ikut terbawa kalau file dicopy.
6. **Hint mode (default):**
   - Klik bagian atas untuk downbeat hint, bagian bawah untuk beat hint. Time-signature hint juga bisa.
   - Setelah itu **wajib analisis ulang**.
   - "Beberapa downbeat hint di awal biasanya cukup."
   - Perubahan time signature **hanya** terdeteksi lewat hint.
   - Rentang tempo bisa diberi lewat downbeat hint di 3 bar berturut-turut.
7. **Beat Marker mode:** tiap marker punya handle:
   - Set Downbeat
   - Move Marker
   - Scale Selection
   - Scale Left/Move Right (buat bagian yang melambat atau ngebut)
   - Scale All
   - Move All
   - Tahan ⌘ untuk snap ke titik analisis.
   - Edit marker akan diabaikan oleh analisis ulang, jadi kunci range-nya dulu (**Lock range**).
8. **Menu Edit:**
   - Analyze Again: kalau ada referensi tempo, yang dianalisis ulang cuma posisi beat, bukan tempo atau downbeat.
   - Set Average Tempo for Selection dan Set Average Tempo within each Bar.
   - Extend First/Last Tempo.
   - Apply Region Tempo to Project (opsi "Align downbeat to nearest project downbeat" dan "Maintain relative position").
9. **Multitrack:**
   - Logic membuat **downmix** dari track yang dicentang "Contributes to Analysis".
   - Downmix itu yang dianalisis, lalu hasilnya ditulis ke semua file dalam set.
   - Track FX atau synth bisa dikecualikan dari analisis tapi tetap mengikuti hasilnya.

## 2. Asal-usul & engine
| Klaim | Status | Sumber |
|---|---|---|
| Smart Tempo memakai teknologi analisis tempo dari Music Memos | 🟡 kredibel (SOS, ask.video), bukan pernyataan resmi Apple | SOS, ask.video |
| Music Memos: "detecting tempo and mapping the beats into measures" | 🟡 blog | Medium (Cerveny) |
| Smart Tempo = ML/neural net | 🔮 **tidak terbukti** | — |
| Smart Tempo = DSP klasik (onset/envelope + autokorelasi + pemilihan path) | 🔮 masuk akal, didukung arah patent Apple | US8983082 |
| Patent Apple US8983082 "Detecting musical structures" (diajukan 2010, terbit 2015): envelope (analytic signal) → downsample → autocorrelation phase matrix 2D (lag × fase) → filter dua dimensi → meter → downbeat = beat pertama | ✅ isi patent | FreePatentsOnline |
| Patent Apple US7148415 (2004): deteksi transient → statistik jarak antar-transient → parameter ritme → koreksi timing | ✅ isi patent (era Logic/GarageBand awal) | FreePatentsOnline |

Inferensi saya 🔮: perilakunya cocok dengan tracker klasik yang punya prior tempo kuat. Buktinya:
- Sering salah setengah atau dua kali lipat, makanya ada tombol /2 ×2.
- Time signature tidak bisa berganti tanpa hint.
- Hint memaksa ulang pencarian path (mirip HMM/DP yang diberi constraint).

Bisa juga sebagian sudah pakai neural onset/beat activation, karena Apple memakai Core ML di Stem Splitter. Tapi itu **tidak ada buktinya**. Jangan klaim salah satu.

## 3. Masalah nyata di komunitas 🟡
- Logic Pro Help (2020): lagu yang di bagian akhir jadi double-time terbaca tempo yang sama. Solusinya: pilih bar di Smart Tempo Editor, lalu klik **/2** (atau ×2).
- Thread lain: 3/4 vs 12/8 sering tertukar, jadi tetap perlu verifikasi manusia.

## 4. Perbandingan engine open-source
| Engine | Pendekatan | Kelebihan | Kekurangan buat lagu worship |
|---|---|---|---|
| **beat_this** (CPJKU, ISMIR 2024) | Spectrogram → conv + transformer (atensi bergantian frekuensi/waktu), output beat & downbeat, **tanpa DBN** | F1 SOTA, tahan tempo drift & perubahan sukat, downbeat sekaligus, sudah dipakai WPE | Metrik continuity lebih lemah (kadang loncat). Bisa pakai `--dbn` (madmom) kalau perlu |
| madmom (RNN/TCN + DBN) | Activation neural + DBN (rentang tempo 55–215, 3/4 atau 4/4) | Sangat stabil di pop 4/4 | Versi PyPI sudah tua (Py<3.10). DBN gagal di ritardando besar/ganti sukat |
| BeatNet | CRNN + particle filter, bisa online/real-time | Cocok untuk live follow | Akurasi offline di bawah beat_this |
| librosa `beat_track` | Onset strength + autokorelasi tempo + DP (Ellis 2007) | Ringan, tanpa model | Asumsi tempo konstan, sering salah setengah/dua kali lipat, tidak ada downbeat |
| Essentia (RhythmExtractor2013, TempoCNN) | Multifeature/degara + CNN tempo | Cepat, banyak fitur | Downbeat lemah, lisensi AGPL (perlu hati-hati untuk distribusi) |
| "Tempo map dari beats" | Ambil beat times → BPM lokal = 60/Δt → smoothing (median) → segmentasi jadi Step/Ramp | Cara inilah yang paling dekat dengan output Smart Tempo (kurva tempo per beat) | Kualitasnya tergantung beat tracker di depannya |

Catatan: angka GTZAN beat_this (F1 beat ≈0.90, downbeat ≈0.80) dikutip dari ringkasan pencarian, tidak saya cek tabelnya langsung.

## 5. Rekomendasi buat WPE

### Engine (akurasi worship: live drum, drift, intro tanpa kick)
1. **beat_this sebagai engine utama** (beat + downbeat). librosa hanya dipakai kalau beat_this gagal, atau di profil Lite kalau terlalu lambat.
2. **Analisis di stem, bukan cuma mix**, meniru multitrack downmix Logic:
   - Downmix drums + bass (+ keys) dengan bobot, **tanpa vokal** (stem demucs sudah ada).
   - Intro tanpa kick (pad/piano saja) sering lebih jelas di stem keys/bass.
3. **Tempo map dari beats:**
   - BPM lokal per beat, lalu median filter 4–8 beat.
   - Deteksi perubahan dengan change-point: lompatan lebih dari 4% jadi titik **Step**, tren monoton lebih dari 2 bar jadi **Ramp**.
   - Hasilnya ditulis ke breakpoint yang sudah ada.
4. **Skor kepercayaan per bar** (dari tinggi aktivasi beat dan jitter Δt). Bar yang ragu diberi warna kuning di editor, seperti cara Logic mewarnai kurva tempo.
5. **Cek setengah/dua kali lipat:** bandingkan kandidat tempo T, T/2, dan 2T dengan prior lagu worship (60–150 BPM, mayoritas 66–80 atau 120–140), lalu tampilkan sebagai pilihan, jangan diputuskan diam-diam.
6. **Intro tanpa kick:** kalau confidence awal rendah, ekstrapolasi mundur dari bagian yang stabil (seperti "Extend First Tempo to Beginning" di Logic). Beat 1 tetap dikonfirmasi user.

### UX yang layak ditiru
1. **Kartu konfirmasi setelah analisis:** "Tempo ≈ 72 BPM · 4/4 · Beat 1 di 0:03.2", dengan tombol **[½] [×2]**, **[3/4 · 4/4 · 6/8]**, dan preview klik. Ini meniru prompt "Edit Downbeat and Tempo" di Logic.
2. **Tap D / T:** sambil lagu diputar, tekan **D** di setiap "satu" (cukup 2–3 bar di awal), lalu app menganalisis ulang dengan constraint itu. Ini hint mode, cara paling Apple-like dan easy.
3. **Editor beat marker:** marker bisa di-drag, dengan handle sederhana: Move, Set Beat 1, Stretch Kiri/Geser Kanan (untuk ritardando), dan Geser Semua. Tahan ⌘ untuk snap ke transient.
4. **Konstan vs Variabel:** satu toggle "Tempo tetap / Ikuti drummer". Mode Tetap memakai Set Average Tempo (default yang cocok untuk click dan playback tracks). Mode Variabel memakai tempo map.
5. **Kunci range:** bagian yang sudah dikoreksi tidak ikut berubah saat analisis ulang.
6. **Seleksi lalu /2 per section** untuk bridge half-time atau ending double-time.
7. **Simpan hasil analisis per file** (hash audio), supaya import ulang tidak menganalisis lagi.

### Prioritas
1. Pertama: kartu konfirmasi (½/×2, sukat, Beat 1) dan analisis di stem tanpa vokal.
2. Kedua: tap D/T + analisis ulang dengan constraint (beat_this activation + DP sederhana yang dipaksa melewati hint).
3. Ketiga: handle beat marker + lock range + skor kepercayaan.

## Sumber
Dibuka langsung:
- https://support.apple.com/en-ca/guide/logicpro/lgcp9281e70c/10.7.5/mac/12.3 (Smart Tempo overview, teks lengkap)
- https://www.logicprohelp.com/forums/topic/133388-smart-tempo-doublehalf-time-section/

Dibaca lewat kutipan hasil pencarian (isi halaman ter-ekstrak, tidak dibuka penuh):
- https://support.apple.com/guide/logicpro/correct-tempo-analysis-results-beat-markers-lgcp644481aa/mac
- https://support.apple.com/guide/logicpro/work-in-the-smart-tempo-editor-lgcp2a428fed/10.7/mac/11.0
- https://support.apple.com/en-ca/guide/logicpro/lgcpb99c36d7/10.7.5/mac/12.3 (hints)
- https://support.apple.com/guide/logicpro/use-smart-tempo-with-multitrack-audio-lgcpc87041da/mac
- https://support.apple.com/en-us/102165 (Match the tempo automatically)
- https://www.soundonsound.com/techniques/logics-smart-tempo-part-1
- https://ask.video/article/audio-software/tempo-sync-all-your-audio-in-logic-pro-x
- https://gmcerveny.medium.com/music-memos-music-technology-aaf3ebf7bc0a
- https://www.freepatentsonline.com/8983082.html
- https://www.freepatentsonline.com/7148415.html
- https://arxiv.org/abs/2407.21658 (Beat This!)
- https://github.com/CPJKU/beat_this

madmom/BeatNet/librosa/Essentia: ringkasan dari pengetahuan umum MIR, tidak diverifikasi ulang di sesi ini.
