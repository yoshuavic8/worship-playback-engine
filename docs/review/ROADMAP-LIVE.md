# Roadmap WPE: Fokus Live & Ibadah

Bahasa sehari-hari untuk Yoshua. Catatan teknis buat developer ada di bagian paling bawah.

**Ukuran:** S = kecil (1–2 hari) · M = sedang (±1 minggu) · L = besar (lebih dari 1 minggu)
**Status kode:** dicek dari salinan kode pertengahan Oktober. Semua status masih **belum dicek di kode terbaru**.

## Rencana rilis

| Versi | Isi | Fokus |
|---|---|---|
| v0.1.2 | Sisa pekerjaan lama | Jalan di Safari 15 bawaan Monterey, tes di Mac Intel / macOS 12 |
| v0.2.0 | Fase 1 | Yang paling terasa waktu ibadah |
| v0.3.0 | Fase 2 | Stabil: nggak drift, nggak putus |
| v0.4.0 | Fase 3 | Aman dipakai Live, tahan error |
| v0.5.0+ | Fase 4 | Fitur tambahan |

---

## v0.1.2: Sisa pekerjaan lama

### 0.1 Jalan di Safari 15 (Monterey bawaan) [S]
- **Kenapa:** Supaya operator nggak disuruh update Safari dulu sebelum bisa pakai app.
- **Selesai kalau:** App dibuka di macOS 12 dengan Safari 15, tampilan dan warnanya sama, dan nggak ada pesan minta update.

### 0.2 Tes di Mac Intel / macOS 12 [S]
- **Kenapa:** Banyak gereja masih pakai MacBook lama.
- **Selesai kalau:** DMG terpasang, setup pertama selesai, satu lagu bisa diputar dengan klik dan stem.

---

## Fase 1 (v0.2.0): Paling terasa waktu ibadah

### 1a. Cue hitungan sebelum section [M]
- **Kenapa:** Musisi dan singer tahu persis kapan masuk: "Verse… 1, 2, 3, 4". Nggak perlu nebak atau lihat MD terus.
- **Selesai kalau:**
  - Di bar terakhir sebelum section baru, suara menyebut nama section lalu menghitung pas di tiap ketukan.
  - Di lagu 3/4 hitungannya "1, 2, 3", di 6/8 bisa diatur (misalnya 1–6 atau 1–2).
  - Bisa pilih suara (cowok/cewek) dan jumlah hitungan, bisa dimatikan per lagu.
  - Suaranya cuma keluar di jalur cue (in-ear), nggak di speaker jemaat.
- *Status:* hitungan di awal lagu sudah ada. Hitungan per section belum ketemu.

### 1b. Fade out di akhir lagu [S–M]
- **Kenapa:** Lagu selesai dengan halus, nggak berhenti mendadak. Musik bisa diturunkan pelan sementara WL masih doa.
- **Selesai kalau:**
  - Ada tombol **"Stop dengan fade"** yang bisa diatur lamanya (misalnya 2–10 detik).
  - Tiap lagu bisa diset "fade otomatis di akhir" dengan durasi sendiri.
  - Klik dan cue berhenti bareng musik, nggak ada bunyi "klik" atau putus di akhir fade.
- *Status:* sudah ada fade out tetap 4 detik (tombol F). Durasi yang bisa diatur dan fade otomatis per lagu belum ada.

### 1c. Pindah section yang nunggu bar selesai [M]
- **Kenapa:** Kalau pindah di tengah bar, hitungan musisi berantakan. Pindah tepat di ketukan 1 bikin tempo tetap benar dan transisinya halus.
- **Selesai kalau:**
  - Pilih section, tombolnya **kedip** (tanda antre), lalu pindah tepat di ketukan 1 bar berikutnya.
  - Pilihan di Settings: **akhir bar** (default), **akhir section**, atau **2 bar**.
  - Cue hitungan (1a) jalan di bar terakhir sebelum pindah.
  - Tekan section yang sama lagi untuk **batal**.
  - Tombol **"Jump now"** untuk darurat, pindah saat itu juga.
- *Status:* antre sampai akhir section dan batal sudah ada. Pilihan akhir bar / 2 bar belum ketemu.

### 1d. Kontrol cepat tanpa Shift+klik [M–L]
- **Kenapa:** MD atau pemain playback sering sambil main musik. Harus bisa pindah section dengan satu sentuhan atau injakan kaki.
- **Selesai kalau:**
  - Layar Live punya **tombol section besar**, cukup satu ketukan.
  - Tombol **1–9** di keyboard langsung pilih section.
  - **Footswitch / pad MIDI** bisa dipakai (tombol not maupun knob/pedal).
  - Ada halaman **MIDI Learn**: klik fungsi, injak pedal, langsung tersambung.
- *Status:* MIDI belum ada di kode.

---

## Fase 2 (v0.3.0): Stabil

### 2a. Klik, cue, dan musik selalu pas [M]
- **Kenapa:** Kalau klik pelan-pelan bergeser dari musik, band jadi nggak rapat sama backing track.
- **Selesai kalau:** Lagu 10 menit diputar dan selisih klik dengan musik tetap **nol** dari awal sampai akhir, dibuktikan tes otomatis.

### 2b. Lagu berikutnya disiapkan duluan [S–M]
- **Kenapa:** Waktu ganti lagu nggak ada jeda atau suara putus.
- **Selesai kalau:** Lagu berikutnya sudah siap di memori. Pindah lagu langsung bunyi, walau file-nya di harddisk eksternal yang lambat.
- *Status:* sebagian sudah ada (lagu berikutnya disiapkan di belakang). Perlu dicek apakah benar-benar dimuat penuh.

### 2c. Catat jeda sebelum Beat 1 [S]
- **Kenapa:** Banyak track punya jeda atau suara pembuka sebelum ketukan 1. Kalau nggak dicatat, klik telat atau kecepatan.
- **Selesai kalau:** Waktu import, app tahu persis di mana Beat 1, dan klik pas dari bar pertama.

### 2d. Transisi antarlagu [M]
- **Kenapa:** Ibadah mengalir tanpa hening canggung antarlagu.
- **Selesai kalau:** Per lagu bisa pilih **lanjut otomatis**, **crossfade** ke lagu berikutnya, atau **pad tetap bunyi** sampai lagu berikutnya mulai.

### 2e. Pindah otomatis ke output cadangan [M]
- **Kenapa:** Kalau soundcard tercabut, musik nggak berhenti total.
- **Selesai kalau:** Cabut soundcard di tengah lagu, suara pindah ke output cadangan dan **lagu lanjut di posisi yang sama**. Colok lagi, bisa balik ke soundcard.
- *Status:* deteksi colok-cabut sudah ditambahkan dan dites Yoshua. Tinggal pindah otomatisnya.

---

## Fase 3 (v0.4.0): Aman dipakai Live

### 3a. Kunci Live [S–M]
- **Kenapa:** Supaya nggak ada yang kepencet edit atau stop di tengah ibadah.
- **Selesai kalau:** Di mode Live, edit terkunci, **stop harus ditahan**, Mac nggak tidur, dan muncul peringatan kalau baterai lemah atau Wi-Fi/notifikasi masih nyala.

### 3b. Lanjut setelah crash [M]
- **Kenapa:** Kalau app tertutup sendiri, operator bisa lanjut cepat tanpa panik.
- **Selesai kalau:** App dibuka lagi, muncul tawaran "Lanjut dari lagu X, bar Y", sekali klik langsung lanjut.

### 3c. Cek sebelum ibadah + tombol Perbaiki [M]
- **Kenapa:** Masalah ketemu sebelum ibadah, bukan saat lagu jalan.
- **Selesai kalau:** Satu tombol mengecek semua lagu di setlist (file hilang atau rusak, soundcard, routing). Tiap masalah punya tombol **"Perbaiki"**, bukan disuruh install ulang.

### 3d. Update yang aman [S–M]
- **Kenapa:** Setelah update, app nggak boleh macet di layar loading.
- **Selesai kalau:** Data otomatis di-backup sebelum update. Kalau gagal, app balik ke data lama dan tetap bisa dibuka.

### 3e. Live 100% offline [S]
- **Kenapa:** Internet gereja sering jelek. Live nggak boleh bergantung internet atau AI.
- **Selesai kalau:** Wi-Fi dimatikan, satu setlist penuh tetap jalan normal.

### 3f. Peringatan routing [S]
- **Kenapa:** Jangan sampai klik atau cue keluar di speaker jemaat.
- **Selesai kalau:** Kalau klik atau cue diarahkan ke output utama, muncul peringatan merah sebelum Live.

### 3g. Penghitung suara putus + saran buffer [S]
- **Kenapa:** Operator tahu kalau Mac-nya kewalahan sebelum suara mulai patah-patah.
- **Selesai kalau:** Ada angka "suara putus: 0" di layar. Kalau naik, app menyarankan ukuran buffer yang lebih aman.

---

## Fase 4 (v0.5.0+): Nanti

| Item | Kenapa | Selesai kalau | Ukuran |
|---|---|---|---|
| 4a. Deteksi tempo ala Smart Tempo | BPM dan Beat 1 lebih jarang salah | Setelah analisis muncul kartu BPM / birama / Beat 1 dengan tombol ½ dan ×2 plus preview klik. Bisa tap D (downbeat) dan T (beat) kalau meleset | L |
| 4b. Pad ikut kunci lagu | Pad otomatis pas nadanya | Ganti lagu, pad ikut key lagu itu | M |
| 4c. Transpose | Ganti nada untuk singer | Stem bisa naik/turun nada tanpa ganti tempo | L |
| 4d. Rig cadangan (Ableton Link / MIDI Clock) | Mac kedua bisa ikut jalan bareng | Dua Mac jalan sinkron, satu bisa ambil alih | L |
| 4e. Export setlist | Bagi ke tim | Setlist bisa di-export dan di-import di Mac lain | S–M |
| 4f. Mode editor Simple | Volunteer baru nggak bingung | Ada mode edit sederhana dengan pilihan sedikit | M |

---

## Catatan teknis (untuk developer)

Semua catatan "status" di atas berasal dari `/workspace/wpe6/app` (salinan pertengahan Oktober), **belum dicek di kode terbaru**.

- **Yang sudah ada:** `PlaybackState::CountIn` dan `song.count_in_bars` (hitungan hanya di awal lagu). `MixerCmd::FadeOut { seconds }`, tapi frontend memanggil `fadeOut(4)` dengan nilai tetap (`transport.ts`, shortcut `KeyF`). `MixerCmd::JumpToSegment { index, immediate }` + `CancelJump`, dan antrean dijalankan di akhir segment (`mixer.rs`, tes `queued_jump_happens_at_segment_end_and_loop_repeats`). `player.preload()` untuk lagu berikutnya. Tidak ditemukan kode MIDI.
- **1a:** Jadwalkan sampel cue dengan presisi sampel di mixer, dari tempo map (birama per segment). Sampel suara per angka + nama section disimpan per bahasa/suara. Hanya ke bus cue.
- **1b:** Field `fade_out_seconds` + `auto_fade` di Song (perlu migrasi DB). Kurva gain equal-power, berlaku ke semua bus, lalu Stop setelah gain sampai 0.
- **1c:** Ganti `immediate: bool` jadi `Quantize { Bar, Segment, Bars(n), Now }`. Hitung batas bar dari tempo map. Status `queued` sudah dikirim ke UI untuk efek kedip.
- **1d:** Crate `midir` (CoreMIDI), mapping Note/CC ke aksi transport, disimpan di settings. Mode learn: tangkap event MIDI berikutnya. Tombol 1–9 lewat `shortcuts.ts`.
- **2a:** Satu counter sampel di callback audio sebagai sumber waktu untuk klik, cue, dan stem. Tes: render offline 10 menit, cocokkan posisi onset klik dengan grid (selisih = 0 sampel).
- **2b:** Pastikan `preload` men-decode penuh ke RAM (bukan streaming dari disk). Ukur pemakaian memori.
- **2c:** Simpan `beat1_offset_samples` saat import/analisis.
- **2e:** Saat hot-plug kehilangan device, bangun ulang stream `cpal` ke device cadangan dan pertahankan `sample_pos`. Perlu resampling kalau sample rate beda.
- **3b:** Tulis posisi (lagu, sampel) ke file setiap ±1 detik secara atomik. Tawarkan lanjut saat start.
- **3d:** Salin file DB sebelum migrasi, migrasi dalam transaksi, dan rollback kalau gagal.
- **3e:** Live mode tidak boleh menyalakan worker Python / Ollama.
- **3g:** Hitung underrun/xrun di callback dan tampilkan di UI.
- **4a:** `beat_this` sebagai engine utama, librosa sebagai cadangan (Lite). Analisis campuran stem drum + bass + keys. Detail di `SMART-TEMPO.md`.
- **v0.1.2:** Ganti `color-mix()` dan `@property` dengan warna yang sudah dihitung dulu atau variabel CSS biasa.
- **Sumber:** `PRIME-RESEARCH.md` (Loop Community Prime: jump terkuantisasi, transisi, cue ikut aransemen, keluhan drift dan migrasi) dan `SMART-TEMPO.md`.


## Update dari Yoshua (21 Okt): aturan klik untuk Jump
- **1x klik** pada section = antre, pindah setelah section/part yang sedang jalan selesai.
- **2x klik** = pindah secepatnya, tetap menunggu bar yang sedang dimainkan selesai (pindah di ketukan 1 bar berikutnya).
- Tidak ada pindah instan di tengah bar (kecuali tombol darurat terpisah).
- Cue hitungan dimainkan di bar terakhir sebelum pindah, untuk kedua mode.
- Tampilan: 1x klik = section berkedip pelan (antre akhir part); 2x klik = berkedip cepat (pindah di akhir bar). Klik section yang sama lagi = batal.
- Angka 1-9 dan footswitch MIDI mengikuti aturan yang sama (tekan sekali / dua kali).
