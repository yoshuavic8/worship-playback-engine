# Riset Prime (Loop Community) → pelajaran stabilitas untuk WPE
_21 Okt 2026 (WIB). Sumber dibaca lewat ringkasan web search dari halaman resmi, App Store, help center, dan Reddit. Yang nggak terverifikasi diberi label **[belum terverifikasi]**._

## Ringkasan
- **Prime MultiTrack App** dari Loop Community: gratis di iOS/iPadOS & Mac (Mac: **macOS 12.0+**). Nggak ada Android. Versi App Store terakhir **8.4.3 (30 Sep 2026)**, isinya "crash fixes". Rating iOS **4.7 (±2,1K)**.
- Jualan utamanya **"simple + reliable"**: user awam bisa jalanin tanpa takut. Contohnya ada review App Store yang bilang pindah dari Ableton karena "anxiety tiap minggu", dan sesudahnya "ALWAYS worked 100%".
- Fiturnya banyak, tapi di sisi stabilitas Prime **lebih tipis** dibanding MultiTracks Playback. Prime **nggak punya Redundancy, SMPTE/LTC, MIDI Clock, atau perubahan birama dalam satu lagu** (ini menurut tabel perbandingan buatan MultiTracks, jadi sumbernya pihak pesaing).
- Pelajaran terbesar untuk WPE ada di **sisi kelemahan Prime**: click sempat nggak sinkron (de-sync), app macet di tahap migrasi data, dan solusi resmi mereka cuma "hapus lalu install ulang".

## Kelebihan Prime (terverifikasi dari halaman resmi/App Store)
- **Kontrol live sekali tap:** loop section (sekali atau tak terbatas), lompat section dengan waktu lompat yang bisa dipilih (*akhir section / setelah 1 bar / setelah 2 bar*), crossfade/auto-advance/overlap, start/end trim, fade out, tombol auto fade in/out.
- **Output cuma 2 untuk mulai:** Auto-Pan (click + cue di KIRI, instrumen di KANAN). Kalau butuh, bisa multi-out dan "Global Audio Routing" ke bus.
- **Dynamic Cues:** kalau arrangement diubah, cue-nya ikut pindah. Custom Band Cues: ketik teks, lalu audio cue-nya dibuat otomatis.
- **MIDI in:** controller class-compliant, Looptimus, dan pedal ekspresi untuk master volume. **MIDI out:** PC/CC/Note untuk lirik dan lampu.
- **Setlist cloud sync** antar device, plus share ke tim. Setlist bisa dibuat di web.
- **Tonic Pad Player** (kerja sama dengan Sunday Sounds), Loop Connect (chart, Sunday Keys, CCLI).
- **Prime MD** (v8.3, Mei 2026): AI "Mix My Tracks" yang menyeimbangkan track terhadap band live.
- UI v8.2: font lebih besar, setlist lebih gampang di-scan, tombol edit dipindah ke header.

## Harga
- Prime Basic: **gratis** (upload sendiri sampai 300 MB). Plan berbayar membuka cloud lebih besar (sampai 500 GB), share ke tim (40 anggota), MIDI out, Band Cues, dan pad. **Harga plan Prime [belum terverifikasi], halamannya nggak menampilkan angka.**
- Langganan track: Lite Tracks **$9.99/bln**, Tracks Pro **$55/bln** (15/25/50 unlock per bulan).

## Keluhan user (sinyal yang harus dihindari WPE)
1. **Click/track de-sync** yang muncul sesekali (Reddit r/worshipleaders, 2024). Pola yang dicurigai: track custom yang diunggah sendiri, jumlah track banyak (15–20), CPU mendekati 100%, dan **audio yang punya hening di awal / nggak mulai tepat di beat 1**.
2. **Latency click/track di IEM.** Terjadi di Prime *dan* MultiTracks Playback, jadi penyebabnya sering di routing/mixer, iPad tua, buffer, atau Wi-Fi.
3. **"Migration in progress" freeze** setelah update. App berhenti di loading tanpa prompt dan tanpa opsi skip (review App Store, 2019).
4. **Solusi support = install ulang/hapus container**, jadi nggak ada repair otomatis. Lagu nggak bunyi? Solusinya hapus lalu download ulang.
5. **Click bocor ke PA** karena mode STEREO/PAN salah atau kabel breakout salah.
6. Dulu jarang di-update (MultiTracks mengklaim Prime cuma 1 update di 2022, klaim pesaing). Sekarang update-nya rutin.
7. MIDI: dulu cuma mendukung Note, CC nggak (FAQ resmi).

## Pembanding singkat: MultiTracks Playback
- **Redundancy Mode:** dua device menjalankan setlist yang sama lewat iConnectivity PlayAudio12, dan kalau yang utama gagal, otomatis pindah ke cadangan tanpa putus (audio + MIDI).
- **Offline:** v8.5.0 (Des 2025) bikin akses offline lebih andal, ada tombol "Redownload" kalau muncul error "Audio Files Missing", ada peringatan kalau logout saat offline, dan pesan jelas kapan internet dibutuhkan "daripada mengulang percobaan yang gagal terus".
- Section preroll (count-off sambil fade in), master fader per lagu, tombol mute semua MIDI sekali tap, BPM pecahan, perubahan birama.

## Pelajaran untuk stabilitas WPE

### P0: wajib sebelum dipakai ibadah
1. **Engine yang nggak pernah nunggu disk/jaringan saat Live.** Saat setlist masuk Live, decode semua stem lagu berikutnya ke RAM (atau ring buffer yang di-prefetch thread terpisah). Callback cpal hanya membaca buffer yang sudah siap: tanpa lock, tanpa alokasi, tanpa log. Tampilkan indikator "Siap ✓" per lagu.
2. **Satu jam master (sample clock).** Click, cue, dan semua stem diambil dari counter frame yang sama di callback, bukan timer terpisah. Ini mencegah de-sync ala Prime. Tambahkan tes otomatis: render 5 menit offline lalu cek drift click vs stem = 0 sampel.
3. **Normalisasi offset saat import.** Deteksi hening atau offset sebelum Beat 1, lalu simpan offset-nya (jangan potong file). Tampilkan peringatan kalau stem beda panjang atau sample rate-nya beda. Resample sekali di worker, jangan di real-time.
4. **Pemeriksaan sebelum ibadah:** semua file ada dan checksum-nya cocok, device tersambung, sample rate dan buffer sesuai, tidak ada lagu yang masih "processing". Tombol **Repair**: analisis ulang atau decode ulang satu lagu. Jangan pernah menyuruh user install ulang.
5. **Migrasi data aman.** Sebelum migrasi skema, backup DB/project. Kalau migrasi gagal: rollback, tampilkan pesan, dan app tetap bisa dibuka (read-only) untuk Live. Jangan pernah macet di loading tanpa pesan.
6. **Live = 100% offline.** Live jangan pernah memanggil jaringan, Ollama, atau Python worker. Worker crash ≠ audio berhenti (proses terpisah, sudah benar). Pastikan juga UI Live nggak menunggu respons worker.
7. **Penanganan device putus** (hot-plug sudah ada): device tercabut saat main → otomatis pindah ke output cadangan (misalnya speaker internal) dengan posisi transport tetap, lalu tampil banner besar. Device kembali → tawarkan pindah balik (jangan otomatis di tengah lagu).

### P1: penting dalam waktu dekat
8. **Live Lock / mode aman:** kunci editing, tombol Stop butuh tekan-tahan atau konfirmasi, cegah sleep (`IOPMAssertion`), dan peringatan kalau baterai/charger lepas, Wi-Fi nyala, atau Low Power Mode aktif.
9. **Pemulihan crash:** simpan state Live (lagu, posisi, mix) tiap beberapa detik. Kalau app dibuka ulang setelah crash, tampilkan "Lanjutkan dari Chorus 2, lagu 3?" lalu muat ulang langsung.
10. **Lompat yang terkuantisasi:** lompat/loop section dieksekusi di akhir section / setelah 1 bar / setelah 2 bar (sample-accurate di engine). Transisi lagu: crossfade/auto-advance/overlap. Section preroll (count-in + fade in).
11. **Preset routing dengan default aman:** preset "2 output" (click+cue kiri, track kanan) dan peringatan "Click bisa bocor ke PA" kalau click diroute ke bus utama. Tombol tes per output ("bunyi di output 3/4").
12. **Monitor CPU/dropout:** hitung xrun/underrun di callback (atomic counter), tampilkan di pemeriksaan sebelum ibadah dan sesudah rehearsal ("2 dropout terjadi → naikkan buffer ke 512?").
13. **MIDI in (Note + CC) + panic button:** pedal untuk next/loop/jump, dan satu tombol "mute semua MIDI".
14. **Pesan error yang jelas** (pelajaran dari Playback 8.5): bilang kapan internet dibutuhkan, tombol Redownload/Repair, jangan ada loop retry tanpa batas.

### P2: nanti
15. **Rig cadangan:** dua Mac menjalankan setlist yang sama dengan Ableton Link (offline via Ethernet) atau MIDI Clock/LTC, lalu pakai switcher hardware. WPE cukup mendukung "start bersamaan + sync tempo".
16. **Ekspor/backup setlist** (file `.wpeset` berisi arrangement + routing) supaya mudah dipindah ke Mac cadangan. Sync cloud nanti saja.
17. Pad player yang mengikuti kunci lagu, master fader per lagu, BPM pecahan, perubahan birama.

## Sumber (yang dibaca lewat hasil pencarian/kutipan halaman)
- https://loopcommunity.com/en-US/prime-multitrack-app
- https://apps.apple.com/us/app/prime-multitrack-app/id664688175
- https://apps.apple.com/us/app/prime-multitrack-app/id1289813088?mt=12
- https://support.loopcommunity.com/article/3706-what-are-prime-setlists
- https://support.loopcommunity.com/article/2496-general-issues
- https://support.loopcommunity.com/article/2485-what-is-prime
- https://loopcommunity.com/en-US/become-a-pro/tracks
- https://www.reddit.com/r/worshipleaders/comments/1b5aqic/anyone_ever_had_the_loopcommunity_prime_app/
- https://www.reddit.com/r/worshipleaders/comments/1ggkyzk/audio_latency_for_clicktracks/
- https://www.multitracks.com/products/playback/comparison/primevsplayback/ (tabel dari pesaing, bisa bias)
- https://helpcenter.multitracks.com/en/articles/5413358-how-to-playback-redundancy
- https://helpcenter.multitracks.com/en/articles/12986877-playback-8-5-0-release-notes
- https://help.ableton.com/hc/en-us/articles/360003279779-Using-Link-without-an-active-internet-connection
- https://www.linkedin.com/posts/loop-community_activity-7433562865342611456-iZVc (Prime MD)

Belum terverifikasi: harga plan Prime (Plus/Pro), apakah Prime punya mode offline resmi atau pemulihan crash, dukungan Ableton Link di Prime, dan engine audio internal Prime.
