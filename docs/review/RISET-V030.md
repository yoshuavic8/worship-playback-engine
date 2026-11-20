# Riset WPE v0.3.0: Transpose, MIDI Mapping, dan Fitur Unggulan Lain
*Disusun 23 Okt 2026 (WIB), diverifikasi dengan riset web di hari yang sama. Dilengkapi 20 Nov 2026 (WIB) dengan rencana uji Arturia MiniLab MkII dan pilihan backend AI FreeLLMAPI. Hanya riset, belum ada perubahan kode. Klaim yang belum bisa dipastikan ditandai **[belum terkonfirmasi]**.*

---
## Ringkasan singkat (baca ini dulu)

1. **Ganti nada (transpose) -4 sampai +4.** Rekomendasi saya: **siapkan dulu file audionya sebelum manggung (pre-render)**, jangan diubah saat lagu berjalan. Waktu user memilih nada baru, worker Python membuat versi baru dari setiap track di background lalu menyimpannya (cache). Saat lagu diputar, engine cukup memutar file yang sudah jadi. Hasilnya: kualitas paling bagus, CPU Mac Intel tidak terbebani saat live, dan kecil risiko suara patah-patah di tengah ibadah.
   - Mesin pengubah nada yang disarankan: **Signalsmith Stretch** (lisensi MIT, sama dengan lisensi WPE, jadi aman). **Rubber Band** sebaiknya tidak dipakai, karena lisensinya GPL sedangkan WPE berlisensi MIT. Kalau dipakai, WPE harus diubah jadi GPL atau membeli lisensi komersial.
   - **Lisensi WPE sudah dicek: MIT License** (file LICENSE di GitHub, "Copyright (c) 2026 Yoshua Victor and contributors").
   - Track **click, cue/guide, dan drum/perkusi tidak diubah nadanya.** Track ini cukup ditandai "jangan di-transpose".
2. **MIDI mapping ala Ableton** (MultiTracks Playback dan Prime juga memakai cara yang sama). Alurnya: tombol **MIDI** (Learn) → semua kontrol yang bisa dipetakan berwarna → klik kontrol (misalnya tombol "Chorus") → tekan tombol/pedal/knob di controller → selesai. Ada daftar mapping untuk mengecek dan menghapus. Mapping section tersimpan **per lagu**, sedangkan mapping transport (Play/Stop/Next song) dan volume track bersifat **global**. Saran saya, keyboard 1–9 jangan dihapus total. Keyboard dijadikan **salah satu sumber yang bisa di-learn** juga (seperti tombol "KEY" di Ableton), jadi tetap bisa dipakai tanpa controller.
3. **Fitur unggulan lain** yang paling berguna untuk MD gereja dan aman dibuat:
   - (a) **Remote dari iPad/HP lewat Wi-Fi**, seperti AbleSet
   - (b) **Vamp/loop section**, yaitu mengulang bagian sampai dilepas
   - (c) **MIDI Program Change/CC keluar per section** untuk menggerakkan keyboard, lampu, atau ProPresenter
   - (d) **Setlist yang bisa diatur** dengan jeda atau auto-lanjut antar lagu

   Fitur yang lebih berisiko (Ableton Link, MIDI clock, lirik/chord sinkron) sebaiknya ditunda.

4. **Alat uji MIDI:** Arturia **MiniLab MkII** (25 tuts) milik Yoshua. Pad 1–8 untuk pindah section, encoder untuk volume track. Encoder pabrik biasanya mode Absolute, jadi volume bisa “lompat”. Untuk uji yang rapi, encoder diubah ke mode Relative lewat MIDI Control Center. Lampu pad bisa dikendalikan, tetapi lewat pesan SysEx, bukan Note/CC biasa.
5. **Pilihan AI:** di Settings user memilih **Tidak dipakai** (hanya mesin aturan), **Ollama**, atau **FreeLLMAPI**. FreeLLMAPI adalah gerbang lokal yang kompatibel dengan API OpenAI. Alamat bawaannya `http://localhost:31415`, plus satu token terpadu. Token disimpan di Keychain macOS, bukan di berkas pengaturan. Kalau layanan tidak merespons, WPE kembali ke mesin aturan. Mode Live tidak pernah menunggu AI.

**Usulan isi v0.3.0:** transpose (pre-render) + MIDI Learn (diuji di MiniLab MkII, termasuk encoder relatif) + vamp/loop + MIDI out Program Change per section + pilihan AI Ollama atau FreeLLMAPI. Remote iPad masuk v0.4. Rinciannya ada di bagian **ROADMAP v0.3.0**.

---
## 1. Transpose -4..+4 semitone

### Penjelasan sederhana
Mengubah nada tanpa mengubah tempo disebut *pitch shifting*. Makin jauh nadanya digeser, makin terdengar "aneh". Vokal bisa terdengar seperti chipmunk atau seperti suara berat robot, dan bass bisa jadi "berlumpur". Batas ±4 yang Yoshua usulkan masuk akal. Angka "±3–4 masih aman" adalah pengalaman umum, bukan standar resmi **[belum terkonfirmasi]**. MultiTracks.com memang menyediakan dua cara: mengunduh file dalam nada yang dipilih dari database mereka, atau men-transpose di dalam app setelah diunduh.

Untuk **vokal** ada yang namanya *formant preservation*: warna suara (karakter mulut/tenggorokan) dijaga supaya tidak ikut naik-turun. Dengan fitur ini, vokal yang naik 3 nada tetap terdengar seperti orang yang sama. Fitur ini perlu dinyalakan untuk track vokal/BGV dan dimatikan untuk instrumen.

### Perbandingan library
| Library | Lisensi | Kualitas | CPU | Formant | Bisa dari Rust? | Catatan |
|---|---|---|---|---|---|---|
| **Rubber Band (v4.0, Okt 2024; engine R3 "finer")** | GPL **atau** komersial (berbayar). **Tidak cocok dengan WPE (MIT)** tanpa lisensi komersial | Sangat tinggi | Berat (R3); R2 lebih ringan | Ya | Ada binding (`rubberband` crate / FFI C API), juga CLI & Python `pyrubberband` | Dipakai di banyak DAW. Kalau WPE memakai GPL, seluruh app wajib GPL saat didistribusikan. |
| **Signalsmith Stretch** | **MIT** (terkonfirmasi di LICENSE.txt) | Tinggi, polyphonic bagus | Sedang (ada `presetCheaper`) | Ada (`setFormantSemitones`/`setFormantFactor` + `setFormantBase`). README menyebut koreksinya tidak setajam algoritma khusus vokal tunggal | Header-only C++; perlu wrapper kecil via `cc`/`cxx`. Binding Rust komunitas **[belum terkonfirmasi]** | Paling cocok untuk WPE. |
| **SoundTouch** | LGPL 2.1 (lisensi komersial non-LGPL tersedia atas permintaan) | Sedang; artefak di stem rapat **[belum terkonfirmasi]** | Sangat ringan (latency maks ~100 ms menurut situs resmi) | Tidak | Ada crate FFI **[belum terkonfirmasi]** | Bagus untuk preview cepat, kurang bagus untuk hasil final. |
| **Bungee** | MPL-2.0 (terkonfirmasi; copyleft per-file, aman dipakai bersama MIT). Bungee Pro komersial **[belum terkonfirmasi]** | Baik | Sedang | **[belum terkonfirmasi]** | C++; perlu FFI | Bagus untuk real-time/scrub, masih relatif muda. |
| **zplane élastique** | Komersial (biaya lisensi) | Sangat tinggi (dipakai Ableton) | Sedang (mode Efficient ~2x lebih ringan dari Pro) | Ya (mode Pro, juga untuk audio polyphonic) | SDK C++ | Tidak praktis untuk proyek open source gratis. |

**CPU di Mac Intel:** perkiraan ini belum diukur **[belum terkonfirmasi]**. Pitch shift real-time untuk 8–16 stem sekaligus kemungkinan berat untuk MacBook Intel lama. Ableton sendiri mengakui mode Complex/Complex Pro boros CPU dan bisa menyebabkan dropout. R2, Signalsmith, atau SoundTouch lebih ringan, tetapi tetap menambah beban dan latency (sekitar 50–120 ms sekali proses). Inilah alasan utama memilih pre-render.

### Bagaimana aplikasi lain melakukannya
- **Ableton Live:** warp mode *Complex/Complex Pro*. CEO Ableton menyebut teknologi ini berasal dari zplane élastique. Complex Pro punya kontrol **Formants** (100% = warna suara asli dijaga). Prosesnya real-time, dan manual Ableton menyarankan *freeze* atau *resample* untuk menghemat CPU.
- **Loop Community Prime:** nada bisa diubah lewat tombol key di toolbar (gratis di paket Basic). Tempo juga bisa diubah. Mesin pitch yang dipakai, dan apakah prosesnya real-time atau dirender dulu, tidak dipublikasikan **[belum terkonfirmasi]**.
- **MultiTracks.com Playback (terkonfirmasi di Help Center):**
  - Ubah nada dilakukan **offline di perangkat** (Edit → lagu → Key/Tempo → Update), atau dengan mengunduh versi nada itu dari server mereka.
  - Stem drum dan perkusi yang tidak bernada **tidak pernah di-transpose**. Pengecualiannya, "Drums (Live)" tetap di-transpose karena ada bocoran suara alat bernada.
  - Tempo hanya bisa diubah ±20 BPM.
- **Kesimpulan:** pemain terbesar (Playback) memakai pendekatan **pre-render**, sama dengan rekomendasi kita.

### Rekomendasi
1. **Offline/pre-render di worker Python.** Pakai Signalsmith Stretch, lewat wrapper Python atau binary kecil. Rubber Band tidak dipakai karena WPE berlisensi MIT. Hasilnya di-cache di folder seperti `cache/<song>/<key>/<stem>.wav` (atau FLAC supaya hemat disk).
2. Per track ada pilihan **Transpose: Ya / Tidak / Formant (vokal)**. Nilai otomatisnya:
   - **Tidak:** click, cue, guide, drums, perc.
   - **Formant:** vocal, BGV.
   - **Ya:** sisanya.

   Sistem memakai nama stem yang sudah terdeteksi.
3. UI menampilkan nada asli dan nada baru (misalnya **E → F#**), progress render, dan tanda ✓ "siap" sebelum lagu bisa diputar dalam nada itu. Kalau render belum selesai, lagu diputar di nada asli dan muncul peringatan.
4. Tombol **"Siapkan semua lagu di setlist"** untuk dijalankan sebelum ibadah.
5. Hasil deteksi nada (`key.py`) dipakai untuk menampilkan nama nada.
6. *(Opsional nanti)* Mode preview real-time memakai SoundTouch untuk latihan cepat, tidak untuk live.

Perkiraan waktu render: lagu 5 menit dengan 10 stem di Mac Intel kira-kira 1–4 menit per nada (perlu diukur).

---
## 2. MIDI Mapping ala Ableton

### Cara kerja di Ableton (yang ditiru)
- Tekan tombol **MIDI** di pojok kanan atas. Semua kontrol yang bisa dipetakan diberi highlight biru, dan Mapping Browser muncul.
- Klik kontrol, lalu gerakkan knob atau tekan tombol di controller. Mapping langsung jadi.
- Panel **Mapping Browser** menampilkan daftar mapping (sumber, tujuan, Min, Max). Min dan Max bisa diubah, dan mapping bisa dihapus dengan Delete.
- Ada pengaturan **Takeover Mode**. Di Ableton ini satu pengaturan global di Settings, bukan per mapping:
  - **None:** nilai langsung lompat.
  - **Pick-Up:** baru bergerak setelah knob "menjemput" posisi nilai sekarang.
  - **Value Scaling:** pindah perlahan menyesuaikan.
- Ada juga **Key Map Mode** (tombol KEY, highlight merah) untuk keyboard komputer, dengan mekanisme yang sama.
- Perilaku pesan:
  - Note On ke switch = toggle.
  - Satu note ke parameter = lompat antara Min dan Max.
  - CC ke switch transport: nilai ≥64 = on.
- Jenis encoder relatif: **Signed Bit, Signed Bit 2, Bin Offset, Twos Complement**, masing-masing ada versi linear/akselerasi. Ableton mencoba mendeteksi jenisnya otomatis.
- Ableton juga mendukung pitch bend dan CC 14-bit.

### Bagaimana app worship melakukannya (terkonfirmasi)
- **MultiTracks Playback:**
  - Edit → MIDI Mapping. Kontrol diberi overlay biru, lalu ketuk kontrol dan tekan tombol controller.
  - Mendukung **Note, CC, dan Program Change**.
  - Mapping tersimpan **per perangkat**. Transport dan master bersifat **global**, sedangkan section, lagu, dan fader track bersifat **per setlist**. Ini sangat mirip usulan kita.
  - Ada juga **Keyboard Mapping** di macOS/iPadOS dan **Infinite Loop** (vamp).
- **Prime:**
  - Tombol MIDI, lalu kotak putih di kontrol yang bisa dipetakan, lalu tekan tombol controller.
  - Ada mapping bawaan untuk Looptimus, dan pedal ekspresi bisa dipakai untuk master volume.
  - Menurut FAQ resmi, Prime **hanya menerima MIDI Note, tidak menerima CC**. WPE bisa unggul di sini dengan mendukung Note, CC, dan PC.

### Desain untuk WPE
- **Tombol "MIDI Learn"** di toolbar. Saat aktif, tombol section, fader volume, Mute, Solo, Play/Stop, Next/Prev song, dan Transpose ±1 diberi highlight.
- **Alur Learn:** klik target → tunggu pesan MIDI → simpan. Kalau pesan yang sama sudah dipakai di tempat lain, muncul peringatan "pindahkan ke sini?".
- **Jenis pesan:**
  - **Note On/Off:** pad atau keyboard.
  - **CC:** knob, fader, tombol, footswitch. Banyak footswitch mengirim CC 64 (sustain) atau CC bernilai 127/0.
  - **Program Change:** pilih lagu, misalnya PC 1 = lagu ke-1 di setlist.
- **Mode tombol:**
  - **Trigger:** untuk section, Next, dan Play.
  - **Toggle:** untuk Mute/Solo (tekan = ganti status).
  - **Momentary:** aktif hanya selama ditekan (berguna untuk vamp/hold).
- **Klik sekali/dua kali dari MIDI:** sama dengan mouse. Tekan sekali artinya pindah di akhir section, tekan dua kali dalam ~400 ms artinya pindah di akhir bar. Footswitch tunggal tetap bisa memakai kedua mode ini.
- **Knob:**
  - **Absolute** (0–127) dengan Min/Max dan takeover Pick-Up sebagai default supaya volume tidak lompat.
  - **Relative** untuk encoder tanpa batas. Formatnya mengikuti Ableton: Signed Bit / Signed Bit 2 / Bin Offset / Twos Complement.
- **Lingkup mapping:**
  - **Per lagu:** tombol section. Contohnya, pad 1 = "Chorus" di lagu A dan "Bridge" di lagu B. Bisa juga dipetakan berdasarkan *urutan* (pad 1 = section ke-1), sehingga tidak perlu di-learn ulang di setiap lagu. Saran saya, sediakan dua pilihan: "Slot section 1..16" (global, otomatis) **dan** override per lagu.
  - **Global** (seperti Playback): transport, pilih lagu, master volume, dan volume/mute/solo berdasarkan *nama/jenis track* (misalnya "Click", "Guide", "Keys"), supaya berlaku di semua lagu.
- **Feedback LED:** kirim Note/CC balik ke controller supaya lampu pad menyala sesuai section yang aktif, berkedip pelan saat antre, dan berkedip cepat saat menunggu akhir bar (bisa memakai pesan berulang). Mute dan Solo juga menyalakan lampu.
- **Keyboard tetap ada sebagai sumber yang bisa di-learn.** Angka 1–9 bisa dijadikan preset bawaan yang bisa dimatikan atau diubah.
- **Hot-plug:** daftar device diperbarui otomatis. Mapping disimpan dengan nama device + channel, sehingga tetap jalan setelah controller dicabut lalu dicolok lagi.

### Crate Rust
- **`midir`:** lintas-platform dan paling umum dipakai. Untuk input dan output di CoreMIDI.
- **`coremidi`:** binding langsung ke CoreMIDI Mac.
- Kedua crate ada di crates.io. Detail notifikasi hot-plug dan cara scan ulang **[belum terkonfirmasi]** dan perlu dicek di dokumentasi crate saat implementasi.
- Saran: pakai `midir` untuk I/O, ditambah polling ringan atau `coremidi` notification untuk hot-plug. Pesan MIDI diteruskan ke thread audio lewat antrean lock-free (`rtrb`/`crossbeam`) supaya timing tidak terganggu.
- Bluetooth MIDI di Mac muncul sebagai device CoreMIDI setelah dipasangkan lewat Audio MIDI Setup **[belum terkonfirmasi untuk merk tertentu]**.

---
## 3. Fitur unggulan lain (diurutkan)

| # | Fitur | Dari mana | Manfaat untuk MD gereja | Risiko stabilitas | Saran |
|---|---|---|---|---|---|
| 1 | **Vamp/Loop section**: ulangi section sampai dilepas, lalu lanjut di akhir bar/section | Prime ("Loop Sections on Demand"), Playback (Infinite Loop), AbleSet (quantized section looping) | Sangat tinggi (worship spontan, doa, ulang chorus) | Rendah, karena sistem jump quantized sudah ada | **v0.3.0** |
| 2 | **MIDI out per section/lagu**: Program Change/CC/Note dikirim otomatis saat section mulai | Playback (MIDI Cues + Automation, paket Premium), Prime (MIDI Cues, misalnya Program Change ke MainStage, ProPresenter, LightKey) | Tinggi: ganti patch keyboard, scene lampu, slide lirik | Rendah–sedang | **v0.3.0** (Program Change & CC saja) |
| 3 | **Remote dari iPad/HP via Wi-Fi** (web app lokal: setlist, section, transport) | AbleSet (web app di browser, tanpa internet, bisa banyak perangkat sekaligus). Remote Playback/Prime **[belum terkonfirmasi]** | Tinggi: WL/drummer bisa memegang kendali | Sedang (server lokal, keamanan jaringan) | v0.4 |
| 4 | **Setlist lebih matang**: urutan, jeda/auto-lanjut, transisi antar lagu, nada & tempo per lagu tersimpan | Semua | Tinggi | Rendah | Sebagian di v0.3.0 (simpan nada per lagu) |
| 5 | **Guide cue kustom** (suara sendiri, bahasa Indonesia "Reff", "Ulang") | Prime (Custom Band Cues: ketik teks, audio dibuat otomatis), AbleSet Cues (suara generated atau rekaman sendiri, dengan hitungan). AbleSet juga punya *dynamic guide*: guide otomatis mute kalau section yang dipilih bukan section berikutnya | Tinggi | Rendah | v0.4 |
| 6 | **Bunyi click per lagu** (pilih suara/aksen, subdivision 1/8) | Playback (toggle subdivision 1/8 via footswitch) | Sedang | Rendah | v0.3.x |
| 7 | **Stop/End dengan fade**, "Stop di akhir section" | Prime (atur start, end & fade), Playback (auto fade-out 5 detik) | Sudah ada sebagian (fade-out) | Rendah | Lanjutkan |
| 8 | **Lirik/chord sinkron** (tampil di layar/iPad) | AbleSet (lirik & chord sinkron), Prime (lewat WorshipTools Presenter/Charts), Playback (terhubung ke ChartBuilder) | Sedang–tinggi | Sedang | v0.5 |
| 9 | **Ableton Link** (sinkron tempo dengan app lain, misalnya drummer di Ableton) | Ableton, AbleSet | Sedang | Sedang (crate `rusty_link`) | Nanti |
| 10 | **MIDI Clock out** (sinkron synth/arpeggiator, delay) | MainStage, Ableton | Sedang | Sedang (jitter perlu diuji) | Nanti |
| 11 | **Redundansi (2 Mac sinkron)** | AbleSet (AbleNet). Redundansi di Playback **[belum terkonfirmasi]** | Rendah untuk gereja kecil | Tinggi | Jauh nanti |
| 12 | **Integrasi Planning Center** (ambil setlist & nada dari PCO) | Planning Center Services / Music Stand | Sedang (kalau gereja memakai PCO) | Sedang (API & login) | Nanti |

---
## 4. Usulan isi v0.3.0

| Item | Wajib/Nanti | Usaha | Risiko |
|---|---|---|---|
| Transpose -4..+4 pre-render (Signalsmith), cache per nada, flag per track (click/guide/drum tidak di-transpose), formant untuk vokal | **Wajib** | Besar (±1–2 minggu) | Sedang: waktu render dan disk di Mac Intel |
| Nada per lagu tersimpan di setlist + tombol "Siapkan semua" | **Wajib** | Kecil | Rendah |
| MIDI Learn: section (per lagu + slot global), transport, next/prev song, volume/mute/solo, Program Change pilih lagu | **Wajib** | Besar (±1–2 minggu) | Sedang |
| Keyboard sebagai sumber yang bisa di-learn (1–9 jadi preset yang bisa dimatikan) | **Wajib** | Kecil | Rendah |
| Hot-plug MIDI device | **Wajib** | Kecil–sedang | Rendah |
| Vamp/loop section | **Wajib** | Sedang | Rendah |
| MIDI out Program Change/CC per section | Wajib-ringan / boleh v0.3.x | Sedang | Rendah–sedang |
| Encoder relatif (Relative #1/#2) dan takeover Pick-Up untuk mode Absolute | **Wajib** (dibutuhkan MiniLab MkII) | Sedang | Rendah–sedang |
| Profil bawaan “MiniLab MkII” + rencana uji di alat Yoshua | **Wajib** | Sedang | Rendah |
| Pilihan AI: Tidak dipakai / Ollama / FreeLLMAPI (URL + token, tombol Uji koneksi, pemilih model), token di Keychain, fallback ke mesin aturan | **Wajib** | Sedang (±3–5 hari) | Sedang: kuota gratis bisa habis atau layanan sedang mati |
| Feedback lampu pad MiniLab (SysEx) | Nanti (v0.3.x) | Sedang | Sedang: formatnya dari komunitas, bukan dokumen resmi Arturia |
| Takeover Value Scaling | Nanti | Kecil | Rendah |
| Remote iPad/HP | v0.4 | Besar | Sedang |
| Guide cue kustom, lirik/chord, Link, MIDI clock | v0.4+ | Besar | Sedang–tinggi |

### ROADMAP v0.3.0

Ini yang wajib masuk rilis 0.3.0, dalam urutan pengerjaan yang disarankan:

1. **Transpose -4 sampai +4, disiapkan sebelum ibadah.** Worker Python merender ulang stem bernada (Signalsmith Stretch), lalu hasilnya disimpan per nada. Click, guide, dan drum tidak diubah. Vokal memakai penjagaan formant. Nada per lagu tersimpan di setlist, plus tombol “Siapkan semua”.
2. **MIDI Learn ala Ableton.** Tombol Learn, daftar mapping, Note/CC/Program Change, plus keyboard komputer sebagai sumber yang bisa dipetakan (angka 1–9 menjadi preset yang bisa dimatikan). Mapping section per lagu (dan slot global 1–16). Transport, volume, mute, dan solo bersifat global. Aturan klik tetap berlaku: sekali = pindah di akhir section, dua kali = pindah di akhir bar. Perangkat MIDI bisa dicabut dan dicolok lagi.
3. **Dukungan encoder yang benar.** Mode Relative #1 dan #2 (plus deteksi otomatis bila mungkin). Kalau encoder masih Absolute, pakai takeover Pick-Up agar volume tidak lompat.
4. **Uji di Arturia MiniLab MkII.** Profil mapping bawaan (pad untuk section, encoder untuk volume, pad bank 2 dan klik encoder untuk transport/mute). Rencana ujinya ada di bagian 5.
5. **Vamp atau loop section**, diulang sampai dilepas, lalu lanjut mengikuti aturan lompat yang sama.
6. **Pilihan backend AI.** Settings: Tidak dipakai, Ollama, atau FreeLLMAPI (URL bawaan `http://localhost:31415` + token terpadu). Tombol Uji koneksi dan pemilih model. Token hanya di Keychain. Kalau gagal atau lewat batas waktu, kembali ke mesin aturan. Mode Live tidak bergantung pada AI.

Yang boleh menyusul di 0.3.x: lampu pad MiniLab, MIDI keluar per section, dan bunyi click per lagu. Remote iPad ditunda ke 0.4.

**Pertanyaan yang sudah terjawab:**
1. Lisensi WPE sudah dicek: **MIT**, jadi pakai Signalsmith Stretch.
2. Alat uji MIDI: **Arturia MiniLab MkII, 25 tuts.**

Tidak ada pertanyaan terbuka yang menunda dimulainya 0.3.0.


---
## 5. Alat uji: Arturia MiniLab MkII (25 tuts)

Ini model MiniLab MkII, bukan MiniLab 3 yang lebih baru. Nama resmi di manual Arturia adalah MiniLab MkII.

### Yang ada di alat ini
- 25 tuts peka tekanan (velocity).
- 16 encoder putar. Encoder 1 dan 9 bisa ditekan (klik), dan kombinasi Shift + putar encoder 1/9 mengirim nomor CC cadangan.
- 8 pad peka tekanan, dengan dua bank (tombol Pad 1–8 / 9–16), jadi total 16 pad. Pad menyala dengan lampu berwarna.
- Strip Pitch dan Mod, tombol Oktaf −/+, tombol Shift, plus jack pedal sakelar (bukan pedal volume/ekspresi analog).
- Delapan memori di dalam alat. **Memory 1 dikunci** untuk Analog Lab Lite dan tidak bisa diubah. Memory 2 sampai 8 boleh diatur. Memori dipanggil dengan **Shift + pad**.
- Pengaturan dilakukan di aplikasi **MIDI Control Center** (MCC) milik Arturia, lalu dikirim ke alat dengan **Store To**.

### Mode encoder (penting untuk volume)
Menurut manual resmi:

| Mode MCC | Pesan yang dikirim | Implikasi di WPE |
|---|---|---|
| **Absolute** (bawaan hampir semua encoder) | Nilai 0–127 | Kalau posisi knob tidak sama dengan volume di app, suara bisa lompat. WPE wajib memakai takeover **Pick-Up** (knob baru bekerja setelah “menjemput” nilai yang ada). Lampu atau motor tidak bisa merapikan ini: MiniLab **tidak menerima CC balik** untuk menyamakan posisi knob. |
| **Relative #1** | Turun: 61–63; naik: 65–67 (makin cepat diputar, makin besar angkanya) | Cocok dengan pola Bin Offset di Ableton (sekitar angka 64). Volume bergerak dari nilai sekarang, jadi tidak lompat. Ini yang sering disarankan komunitas (misalnya ReaLearn). |
| **Relative #2** | Turun: 125–127; naik: 1–3 | Cocok dengan Two’s Complement di Ableton. Juga umum dipakai (Ardour, beberapa panduan MPC). |
| **Relative #3** | Turun: 13–15; naik: 17–19 | Mirip Signed Bit (sekitar angka 16). Lebih jarang. |

Pencocokan nama Ableton di tabel itu adalah tafsiran yang lazim, dan tetap perlu dicek di alat Yoshua **[sebagian belum terkonfirmasi di perangkat]**. Manual juga menyebut, pada mode relatif, ada pesan “posisi netral” (nilai 00) di antara gerakan. WPE harus mengabaikan nilai 0 agar volume tidak tersentak.

**Saran:** di MCC, salin templat ke Memory 2 (beri nama WPE), ubah ke-16 encoder ke **Relative #1**, lalu Store To, dan panggil dengan Shift + pad 2. Kalau Relative #2 ternyata lebih nyaman, ganti saja. WPE mengenali ketiga mode itu, dan tetap aman jika belum ada yang diubah (mode Absolute + Pick-Up).

### Pesan pabrik yang sering dilaporkan
Angka ini berasal dari manual, dukungan Arturia, dan catatan komunitas (padmon, ReaLearn). Tetap dikonfirmasi dengan monitor MIDI di alat Yoshua, karena Memory 1 tidak bisa diubah dan isi memori user bisa berbeda.

- Bank pad 1–8: Note On/Off, channel 10, nada 36–43 (pad 1 = 36). Ini ada di manual.
- Bank pad 9–16 (Memory Analog Lab): CC 22–29 di channel 1, nilai 127 saat ditekan dan 0 saat dilepas (dukungan Arturia dan padmon). Untuk Learn yang rapi, di Memory 2 sebaiknya ke-16 pad dikirim sebagai **Note**, misalnya 36–43 dan 44–51.
- Encoder, menurut pemetaan yang sering dipakai ReaLearn: 1=CC 112, 2=74, 3=71, 4=76, 5=77, 6=93, 7=73, 8=75, 9=114, 10=18, 11=19, 12=16, lalu 13–16 kemungkinan 17, 91, 79, 72 di channel 1 **[urutan 13–16 belum terkonfirmasi di perangkat]**. Padmon menyebutkan kumpulan CC yang sama: 16–19, 71–79, 91, 93, 112, dan 114.
- Klik encoder 1 dan 9: ReaLearn mencatat CC 113 dan 115 **[belum terkonfirmasi sebagai nilai pabrik]**.
- Pedal sustain biasanya CC 64 **[belum terkonfirmasi di perangkat]**.

### Lampu pad
Pad memiliki lampu berwarna. Mengubah warna dari aplikasi **bukan** lewat Note atau CC biasa, melainkan lewat **SysEx** Arturia. ReaLearn dan padmon sudah memakainya. Format yang dicatat padmon (diverifikasi komunitas, September 2026):

`F0 00 20 6B 7F 42 02 00 10 {pad} {warna} F7`

Nomor pad 1–16 memakai `0x70` sampai `0x7F`. Warna yang dicatat: mati `00`, merah `01`, hijau `04`, kuning `05`, biru `10`, magenta `11`, sian `14`, putih `7F`.

Ini belum ada di manual resmi Arturia (Arturia memang jarang merilis bagan MIDI lengkap), jadi tetap **[belum terkonfirmasi secara resmi]**. Karena itu lampu pad masuk v0.3.x, bukan syarat rilis 0.3.0. Encoder tidak punya umpan balik visual dari DAW.

### Mapping bawaan yang disarankan untuk WPE
Setelah Memory 2 diatur (encoder Relatif, pad 9–16 jadi Note), WPE boleh menawar “Pakai profil MiniLab MkII”:

| Kontrol | Fungsi | Lingkup |
|---|---|---|
| Pad 1–8 (bank 1) | Slot section 1–8 (aturan klik sekali/dua kali tetap berlaku) | Per lagu, lewat slot global |
| Pad 9 | Putar / berhenti | Global |
| Pad 10 | Lagu berikutnya | Global |
| Pad 11 | Lagu sebelumnya | Global |
| Pad 12 | Vamp/loop section yang sedang jalan (toggle) | Global |
| Pad 13 | Mute click | Global (berdasar jenis track) |
| Pad 14 | Mute guide | Global |
| Pad 15 | Transpose −1 (dalam batas −4) | Per lagu |
| Pad 16 | Transpose +1 (dalam batas +4) | Per lagu |
| Encoder 1 | Volume master | Global |
| Klik encoder 1 | Putar / berhenti (cadangan, kalau pad 9 terasa jauh) | Global |
| Encoder 2–8 | Volume track menurut jenis: click, guide, drum, bass, keys, gitar, vokal | Global |
| Encoder 9–16 | Volume track lain, urut seperti mixer | Global |
| Klik encoder 9 | Solo guide, atau vamp **[pilih saat uji]** | Global |
| Pedal sakelar | Putar / berhenti, atau tahan vamp (momentary) | Global |
| 25 tuts keyboard | Tidak dipakai WPE secara bawaan, supaya tetap bisa dipakai bermain | Tidak ada |

User tetap bisa mengubah semua ini lewat MIDI Learn. Hati-hati: **Shift + pad** memanggil memori alat, dan bisa ikut mengirim nada pad. Saat uji, pastikan gerakan itu tidak merusak antrean section.

### Rencana uji di MiniLab Yoshua
1. Pastikan alat muncul di Audio MIDI Setup dan di daftar perangkat WPE, lalu cabut-colok (hot-plug).
2. Dengan Memory 1, rekam semua pesan di monitor MIDI: 16 pad, 16 encoder, klik encoder 1/9, pedal, Pitch, dan Mod. Catat channel, nomor, dan nilai. Ini menjadi acuan pabrik.
3. Di MCC, salin ke Memory 2, ubah encoder ke Relative #1, ubah pad 9–16 menjadi Note, Store To, lalu panggil Shift + pad 2. Rekam lagi, bandingkan.
4. Learn: pad 1 ke section 1. Cek klik sekali (pindah di akhir section) dan dua kali (pindah di akhir bar), plus klik ketiga untuk batal.
5. Biarkan encoder di Absolute, pindahkan volume di app jauh dari posisi knob, lalu putar pelan. Tanpa Pick-Up, volume harus terasa lompat. Dengan Pick-Up, belum berubah sampai knob “ketemu” nilainya.
6. Ubah ke Relative #1, lalu #2. Volume harus bergeser halus ke dua arah, termasuk saat diputar cepat (akselerasi).
7. Uji klik encoder 1/9, pedal, dan pindah bank 1–8 / 9–16 (mapping tidak boleh hilang).
8. Uji Shift + pad (panggil memori) supaya tidak ikut memicu section.
9. Cabut USB di tengah lagu, colok lagi, pastikan mapping kembali.
10. (v0.3.x) Kirim SysEx warna ke satu pad, pastikan lampunya sesuai, dan tidak mengganggu Memory 1 / Analog Lab.

---
## 6. Pilihan backend AI: Ollama atau FreeLLMAPI

### Apa itu FreeLLMAPI
FreeLLMAPI adalah aplikasi sumber terbuka (lisensi **MIT**, sama dengan WPE) buatan Tashfeen Ahmed. Program ini berjalan di komputer user, lalu menyatukan kuota gratis banyak penyedia model (Gemini, Groq, Mistral, OpenRouter, dan lainnya) di belakang **satu alamat** dan **satu token terpadu** yang biasanya berawalan `freellmapi-…`.

- Repositori: https://github.com/tashfeenahmed/freellmapi
- Dokumentasi API: https://github.com/tashfeenahmed/freellmapi/blob/main/docs/en/api/01-rest-api.md

Ada dua cara menjalankannya, dan portnya berbeda:

- **Aplikasi desktop** (ikon di bilah menu Mac): mendengar di `127.0.0.1`, port bawaan **31415**. Kalau port itu sudah dipakai, aplikasi mencari port berikutnya dan mengingatnya. Token disalin dari ikon bilah menu (“Copy Key”) atau dari dasbor Keys.
- **Pemasangan server/Docker**: biasanya `http://localhost:3001`.

Proyek ini menyatakan dirinya untuk **uji coba pribadi**, bukan untuk produksi. Kuota gratis tiap penyedia bisa berubah atau habis. Karena itu WPE tidak boleh bergantung padanya saat ibadah berlangsung.

### Format API (terkonfirmasi di dokumentasi resmi)
Kompatibel dengan OpenAI:

- Chat: `POST {asal}/v1/chat/completions`
- Daftar model: `GET {asal}/v1/models` (boleh ditambah `?available=true` agar hanya model yang siap dipakai)
- Otorisasi: header `Authorization: Bearer <token-terpadu>`
- Tubuh chat mengikuti kebiasaan OpenAI: `{ "model": "auto", "messages": [ { "role": "user", "content": "…" } ] }`

Nama model yang berguna:

- `auto`: router yang memilih model bebas yang masih ada kuotanya, lalu pindah sendiri kalau ditolak
- `auto:fast`, `auto:smart`, `auto:reliable`, `auto:balanced`
- atau id model tertentu

Dokumentasi desktop memakai base URL `http://127.0.0.1:31415/v1`. Kalau user hanya mengisi `http://localhost:31415` (tanpa `/v1`), WPE harus menambahkan `/v1` sendiri, dan sebaliknya tidak menuliskannya dua kali.

FreeLLMAPI juga bisa meniru API Ollama (`/api/tags`, `/api/chat`), tetapi fitur itu **mati secara bawaan**. User harus menyalakannya dulu di Keys → Agents (`open-loopback` atau `key-required`). Karena itu WPE berbicara ke FreeLLMAPI lewat API OpenAI, bukan menganggapnya sebagai Ollama.

### Desain di WPE
Di **Settings → AI**, user memilih salah satu:

1. **Tidak dipakai.** Hanya mesin aturan (seperti cadangan yang sudah ada). Ini tetap pilihan yang sah, dan wajib tersedia sebagai cadangan.
2. **Ollama.** Alur yang sudah ada: pasang baru, atau pakai Ollama yang sudah terpasang.
3. **FreeLLMAPI.** Dua isian:
   - URL dasar, bawaan `http://localhost:31415` (boleh diubah, misalnya ke `http://127.0.0.1:31415` atau `http://localhost:3001`)
   - Token terpadu
   - Tombol **Uji koneksi**: memanggil `GET /v1/models` dengan token itu. Kalau berhasil, tampilkan daftar model yang siap, plus `auto`. Kalau gagal, tampilkan alasan yang jelas (tidak tersambung, port salah, token ditolak, dan sebagainya)
   - Pemilih model, bawaan `auto`

Aturan keamanan dan keandalan:

- Token **hanya disimpan di Keychain macOS**, bukan di berkas pengaturan, log, atau cadangan yang mudah tersalin. URL dan nama model boleh disimpan biasa, karena bukan rahasia.
- Token tidak pernah dicetak di log.
- Batas waktu: sambungan sekitar 5–10 detik, jawaban lengkap sekitar 30–60 detik (perlu disesuaikan dengan pemakaian aktual, misalnya merancang arrangement).
- Kalau timeout, koneksi ditolak, 401, 429, atau 5xx: kembali ke mesin aturan, dan beri tahu user di luar mode Live.
- **Mode Live tidak pernah menunggu atau membutuhkan AI.** Arrangement dan deteksi boleh memakai AI hanya saat persiapan.

WPE tidak memasang FreeLLMAPI. User menjalankannya sendiri, lalu menempelkan URL dan token. Ini selaras dengan permintaan Yoshua: pilihan antara memasang Ollama atau memakai FreeLLMAPI yang sudah jalan di localhost:31415.

---
## Lampiran Teknis

**Pipeline transpose:**
- Worker Python membuat job `transpose(song_id, semitones, stems[], mode)`.
- Untuk tiap stem dengan flag `pitch=on`, worker menjalankan Signalsmith Stretch: `setTransposeSemitones(n)`, plus untuk vokal `setFormantSemitones(0, true)` dan `setFormantBase(~200/sampleRate)`.
- Hasil ditulis ke `cache/<song_hash>/<+n>/<stem>.flac`. Stem dengan `pitch=off` di-symlink atau langsung memakai file asli.
- Job memakai checksum source + versi algoritma untuk invalidasi cache, dan mengirim progress lewat kanal worker yang sudah ada.
- Engine Rust memilih set file sesuai `song.key_offset`, dan harus memverifikasi panjang sampel sama persis supaya sinkron beat/section (time-stretch rasio 1.0 harus menjaga panjang; kalau berbeda, trim/pad).
- Latency offline tidak masalah, tetapi perlu kompensasi delay awal: algoritma menambahkan pre-roll, jadi output digeser sebesar `inputLatency()+outputLatency()` (nama method Signalsmith).

**MIDI:**
- `midir::MidiInput::connect` per port, dengan callback yang mem-parse dan push ke `rtrb` ring buffer. Thread kontrol (bukan audio callback) mengeksekusi aksi lewat API engine yang sudah ada (queue jump, set gain).
- Mapping disimpan sebagai JSON:
  `{source:{device, channel, type: note|cc|pc|key, number}, target:{scope: global|song:<id>, kind: section_slot|section_id|transport|track_gain|track_mute|track_solo|song_select|transpose, ref}, behavior:{mode: trigger|toggle|momentary, min, max, takeover: none|pickup|scale, relative: null|twos|offset|signmag}}`
- Double-press detection berjalan di sisi kontrol (jendela 350–400 ms). Kelemahannya, single press tertunda selama jendela itu. Alternatif tanpa delay: tekan pertama langsung antre di akhir section, tekan kedua dalam jendela itu meng-upgrade ke akhir bar. Ini sama dengan perilaku UI sekarang.
- LED feedback umum: `MidiOutput` ke port dengan nama sama, mengirim nilai 127/0 (atau velocity warna untuk Launchpad/APC).
- MiniLab MkII tidak merespons CC balik pada encoder. Lampu pad memakai SysEx (lihat bagian 5). Decoder encoder: Absolute + pickup; Relative #1 (offset di sekitar 64); Relative #2 (two’s complement); Relative #3 (signed bit di sekitar 16). Abaikan CC bernilai 0 pada mode relatif, sesuai catatan “posisi netral” di manual.
- FreeLLMAPI: klien HTTP yang kompatibel dengan OpenAI, di worker atau di sisi Tauri. Base URL dinormalisasi (tambah `/v1` bila belum ada). `GET /v1/models?available=true` untuk uji dan daftar model; `POST /v1/chat/completions` untuk kerja AI. Header `Authorization: Bearer …`. Token dibaca dari Keychain saat permintaan, tidak disimpan ke disk. Timeout dan semua galat jaringan/HTTP merosot ke mesin aturan; pemutar Live tidak memanggil klien ini.
- Hot-plug: polling `MidiInput::ports()` tiap 1–2 detik lalu diff, atau memakai notifikasi CoreMIDI lewat crate `coremidi` (nama API **[belum terkonfirmasi]**).

**Vamp:** status `loop_section=true` membuat engine mengulang `[section.start, section.end)` pada boundary bar yang sudah dihitung oleh quantizer jump. Saat dilepas, jump ke section berikutnya mengikuti aturan klik yang sama.

---
## Sumber (dicek 23 Okt 2026; MiniLab dan FreeLLMAPI dicek 20 Nov 2026)
- Lisensi WPE (MIT): https://github.com/yoshuavic8/worship-playback-engine/blob/main/LICENSE
- Rubber Band (v4.0, lisensi GPL/komersial): https://breakfastquay.com/rubberband/ , https://breakfastquay.com/rubberband/license.html
- Signalsmith Stretch (MIT, API formant): https://github.com/Signalsmith-Audio/signalsmith-stretch , https://signalsmith-audio.co.uk/writing/2023/stretch-design/
- SoundTouch (LGPL 2.1): https://www.surina.net/soundtouch/
- Bungee (MPL-2.0): https://github.com/bungee-audio-stretch/bungee
- zplane élastique (teknologi & lisensi SDK): https://licensing.zplane.de/technology ; wawancara CEO Ableton: https://products.zplane.de/blog/gerhard-behles-interview-ableton
- Ableton warp modes (Complex Pro, Formants, CPU): https://www.ableton.com/en/manual/audio-clips-tempo-and-warping/
- Ableton MIDI & Key Remote (MIDI Map Mode, Takeover, relatif): https://www.ableton.com/en/manual/midi-and-key-remote-control/
- Prime (fitur & paket): https://loopcommunity.com/en-US/prime-multitrack-app ; FAQ (hanya MIDI Note, pedal ekspresi): https://loopcommunity.com/en-us/faq/4/48 ; transpose: https://support.loopcommunity.com/article/2492-how-do-i-transpose-a-track-in-prime ; MIDI mapping & MIDI Out: https://loopcommunity.com/blog/2019/12/5-features-in-prime-you-didnt-know-about/ ; Program Change ke MainStage: https://loopcommunity.com/blog/2020/06/how-to-trigger-program-changes-in-mainstage-with-the-prime-app/
- MultiTracks Playback, key/tempo: https://helpcenter.multitracks.com/en/articles/4520655-how-to-change-the-key-and-tempo-of-a-song-in-playback ; MIDI mapping: https://helpcenter.multitracks.com/en/articles/4490527-how-to-setup-a-midi-controller-to-control-playback ; User Guide: https://helpcenter.multitracks.com/en/articles/4944485-playback-user-guide ; contoh footswitch (Morningstar): https://www.morningstar.io/post/multitracks-playback-midi-foot-control
- AbleSet: https://ableset.app/
- MainStage: https://www.apple.com/mainstage/ ; Planning Center Services: https://www.planningcenter.com/services
- Crates: https://crates.io/crates/midir , https://crates.io/crates/coremidi , https://crates.io/crates/rusty_link

- Arturia MiniLab MkII, manual: https://downloads.arturia.net/products/minilab-mkII/manual/minilab-mkii_Manual_1_1_EN.pdf
- FAQ MiniLab MkII (Memory 1 terkunci, encoder 1 dan 9 punya klik dan Shift+putar): https://support.arturia.com/hc/en-us/articles/4405748007570-MiniLab-MkII-General-Questions
- ReaLearn/Helgobox (encoder relatif, lampu pad lewat SysEx, pemetaan CC): https://github.com/helgoboss/helgobox/wiki/Arturia-MiniLab-Mk2
- Padmon, templat pabrik pad/LED SysEx (diverifikasi komunitas, 26 Sep 2026): https://tangled.org/faz.ms/padmon/blob/0d526fddbcf14ac7719fd5329fa4a901723c8100/controllers/arturia-minilab-mk2.toml
- Forum Arturia: encoder tidak menerima CC balik dari DAW: https://legacy-forum.arturia.com/index.php?topic=103830.0
- FreeLLMAPI (MIT): https://github.com/tashfeenahmed/freellmapi
- Dokumentasi API FreeLLMAPI: https://github.com/tashfeenahmed/freellmapi/blob/main/docs/en/api/01-rest-api.md
- Pemasangan dan token terpadu: https://github.com/tashfeenahmed/freellmapi/blob/main/docs/en/install/01-install.md
- Aplikasi desktop (port 31415): https://github.com/tashfeenahmed/freellmapi/blob/main/desktop/README.md

*Catatan:*
- Dihapus: link zplane élastique-pro lama (404) dan link umum support ProPresenter. Dukungan MIDI di ProPresenter didasarkan pada blog Prime yang menyebutnya, bukan pada dokumentasi ProPresenter **[belum terkonfirmasi langsung]**.
- Mesin pitch internal Prime tidak dipublikasikan.
- Angka CC encoder MiniLab dan format SysEx lampu berasal dari komunitas, bukan bagan resmi Arturia. Perlu dikonfirmasi di alat Yoshua.
- Jumlah penyedia dan model di halaman depan FreeLLMAPI bisa berubah. Yang dipakai WPE adalah format API-nya, bukan angkanya.
