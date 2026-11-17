# TEMPO-BUG: "Tempo tidak berubah saat diedit di tempo automation lane"

Snapshot 22:40 WIB. File editor/grid identik dengan snapshot RETEST (tidak ada perubahan sejak itu).
Semua analisis read-only. Repro hanya dijalankan di box (`/workspace/wpe3/repro/`), bukan di repo Mac.

## 1. Penyebab utama (sudah direproduksi)

### #1. Titik tempo di bar 1.1 tertutup handle "Drag Beat 1" (kemungkinan sangat tinggi)

**Lokasi di kode:**
- `SongTimeline.tsx:437-443`: handle Beat 1 adalah div `z-30`, `w-3` (12 px), `pointer-events-auto`, `top-0 bottom-0`. Div ini membentang setinggi *semua* lane, termasuk lane Tempo, tepat di x = Beat 1.
- `TempoLane.tsx:43-44, 215-226`: titik pertama selalu digambar di q=0, yaitu tepat di Beat 1. Ukuran dot 14 px dengan `z-20`. Root TempoLane tidak membuat stacking context, jadi `z-30` milik handle yang menang.
- Akibatnya, `pointerdown` di dot masuk ke `beginBeat` (`SongTimeline.tsx:199-219`), bukan ke `begin()` milik TempoLane.
- `beginBeat` hanya membaca sumbu X. Saat pointerdown pun, Beat 1 langsung dipindah ke posisi kursor (`setBeatDrag(apply(e.nativeEvent))`, baris 218).

**Bukti repro (Chrome headless, komponen SongTimeline asli, data Never Lost 6/8, zoom 8.4×):**
- Hit-test di −6…+4 px dari pusat dot semuanya mengenai "Drag Beat 1". Hanya +6 px yang mengenai lane.
- Drag dot ke atas 30 px:
  - **BPM tetap 123.592**.
  - **Beat 1 berpindah 4.162 → 4.271 s** (jitter 1 px ≈ 0,11 s).
  - Event yang muncul hanya `onMoveBeat`, tidak ada `onTempo`.
  - Cursor yang tampil `ew-resize`.
- Sebagai kontrol: double-click untuk menambah titik di bar 8.5, lalu drag ke atas 30 px, hasilnya **123.592 → 126.592**. Jalur lane → writeGrid → state berfungsi.

**Yang dia lihat:**
- Kursor berubah jadi ↔ di atas dot, dan dot tidak bergerak naik/turun.
- Kalau mouse sedikit bergeser horizontal, garis Beat 1 dan semua bar ikut bergeser, angka "BEAT 1" di header berubah, tetapi TEMPO tetap 123.59.
- Karena lagu ini hanya punya 1 titik, *tempo lagu memang tidak bisa diubah dari lane sama sekali*.

### #2. Yang diubah ternyata titik baru, bukan tempo awal (sedang)

- Double-click menambah titik **Step** baru dengan BPM yang sama (`tempoMap.ts:214-228`).
- Jika titik itu dinaikkan, tempo hanya berubah *setelah* titik tersebut. Bar 1 sampai titik itu, header TEMPO (dibaca di playhead/Beat 1, `SongTimeline.tsx:116, 281`), dan klik awal tetap 123.59.
- Lane juga mengabaikan "Snap: Bar" di toolbar dan selalu snap ke beat (`TempoLane.tsx:80-86`). Di repro, titik jatuh di 8.5, bukan 9.1.
- **Yang dia lihat:** garis kuning naik mulai dari titik baru, tapi "tempo tidak berubah" di awal lagu.

### #3. Edit BPM/Beat 1 di laci "Mix" (SongInfo) di-revert saat Save (sedang, kalau dia memakai panel itu)

- Lokasi kontrol: NumInput BPM, Tap, dan ÷2/×2 di `SongEditor.tsx:1033-1040`; Beat 1 di 978, 875-886, dan ~1000. Semua hanya memanggil `set({ bpm })` / `set({ beatOffset })` tanpa mengubah `grid`.
- Saat save, `sanitize_song` (`commands.rs:306-313`) menimpa `bpm` dan `beat_offset` dengan nilai dari `grid`.
- Timeline juga merender dari `grid` (`SongTimeline.tsx:114-115`).
- **Yang dia lihat:** angka di panel berubah dan Save menyala, tetapi lane/header/klik tidak berubah. Setelah Save, angkanya kembali seperti semula.

### Bukan penyebab (sudah dicek)

- **Live reload engine:** `set_grid` (`commands.rs:565-584`) menulis DB, lalu `apply_grid` (`player.rs:197-214`) mengirim `SetBeatTable`, lalu `mixer.rs:503-511` menjalankan `sync_beat`. Klik berubah tanpa reload. Jika tidak ada lagu yang di-load ("Nothing loaded" di screenshot), lagu akan di-load dari DB yang sudah baru.
- **Clamp:** range 30–300 (`tempoMap.ts:247`). Angka 110/130 hanya skala tampilan otomatis (`yScale`, `tempoMap.ts:283-290`), bukan batas.
- **Save abu-abu dan Undo abu-abu di screenshot:** berarti belum ada edit sama sekali di sesi itu. Edit apa pun memanggil `remember()`, yang menyalakan Undo. Kemungkinan screenshot diambil sebelum dia mencoba.
- **Meter 6/8:** rumusnya konsisten, BPM = klik/menit, `beatsPerBar` 6 di TS dan Rust (kecuali kasus di temuan audit #6).

## 2. Cek manual 30 detik

1. Arahkan mouse ke dot kuning di bar 1.1.
   - Jika kursor jadi **↔** dan tooltip **"Drag Beat 1"**, itu penyebab #1.
   - Coba drag ke atas, lalu lihat angka BEAT 1 di header. Kalau angka itu berubah dan TEMPO tidak, terkonfirmasi. Tekan ⌘Z.
2. Di Inspector, klik baris **"1 · 1.1 · 123.59"** pada TEMPO MAP. Tekan **↑** 10 kali, atau ketik 125 di field Tempo lalu Enter.
   - Header TEMPO seharusnya menjadi 124.59/125 dan Save menyala kuning. Artinya data path sehat dan masalahnya hanya hit-test.
3. Tekan Play dan dengarkan klik. Jika klik ikut berubah, engine aman. Jika tidak, laporkan kembali (artinya ada penyebab lain).

## 3. Audit editor + grid (prioritas)

1. **[P0] Handle Beat 1 menutupi titik tempo 1.** Lokasi: `SongTimeline.tsx:437-443` vs `TempoLane.tsx:215-226`. Lihat #1.
2. **[P1] Dua set kontrol BPM/Beat 1 yang saling bertabrakan.** Inspector `BpmField`/÷2/×2/nudge (`SongTimeline.tsx:688-700`) sudah benar lewat `commitBpm` → `writeGrid`. Laci Mix/SongInfo (`SongEditor.tsx:875-886, 978, ~1000, 1033-1040`) hanya mengubah bpm/offset, lalu di-revert oleh `commands.rs:306-313`. Hapus panel SongInfo atau arahkan ke `writeGrid`.
3. **[P1] Revert tidak mengembalikan grid yang sudah tersimpan otomatis.** `writeGrid` → `pushGrid` → `set_grid` langsung menyimpan ke DB dan engine (`SongEditor.tsx:123-139`). Tombol Revert hanya memanggil `setDraft(stored)` (`SongEditor.tsx:767`), sedangkan `stored` masih yang lama karena `set_grid` tidak meng-emit `library:changed` dan hasilnya diabaikan. Akibatnya UI terlihat "kembali", padahal klik dan DB tetap tempo hasil edit.
4. **[P1] Separuh tersimpan otomatis, separuh manual.** Grid tersimpan otomatis, sedangkan section yang di-resnap oleh `writeGrid` (`resnapSections`) hanya ada di draft. Tombol Back (`SongEditor.tsx:723` → `back`) tidak menyimpan, sehingga DB bisa berisi grid baru dengan section lama.
5. **[P1] "Lock grid" tidak berpengaruh apa pun.** Lokasi: `SongEditor.tsx:860-870, 1135-1160` dan `commands.rs:~505-520`. Player/mixer tidak pernah membaca `beatGridLocked` (grep: hanya ada di prep.rs/ai.rs). Klik selalu mengikuti tempo map. Teks "Click follows the music's actual tempo… Reload song" menyesatkan, dan dokumentasi di `click.rs:9-11` sudah usang. Efeknya hanya mengubah warna pill header (`SongTimeline.tsx:281`).
6. **[P2] Ganti meter tidak memetakan ulang titik tempo, dan tidak tersimpan otomatis.** `applyMeter` (`SongEditor.tsx:489-500`) hanya mengganti `beatsPerBar`, padahal titik disimpan sebagai bar.beat, sehingga posisinya bergeser dalam waktu. Fungsi ini juga tidak memanggil `pushGrid`. Edit berikutnya lewat `set_grid` di-sanitize memakai meter *lama* dari DB (`commands.rs:570`, `tempo_map.rs:194-241`), dengan beat di-clamp ke bpb lama. Akibatnya engine memasang perubahan tempo di posisi yang berbeda dari yang terlihat.
7. **[P2] Header masih membaca analisis lama.** "GRID Changing", confidence 18%/0% (`SongTimeline.tsx:247-250`) diambil dari `analysis.tempoMode` lama (false positive, lihat RETEST #4). Nilainya tidak berubah walau user sudah menetapkan grid 1 titik.
8. **[P2] Kode mati / fitur yang tidak benar-benar ada.**
   - `components/Waveform.tsx` (301 baris) tidak diimport di mana pun.
   - Teks "Tempo regions · flagged on waveform" (`SongEditor.tsx:1079`) dan `analysis.tempoMap` (853) merujuk ke tampilan yang sudah tidak ada. Region lama berdampingan dengan tempo map baru.
   - Docstring `click.rs:1-11` masih menyebut mode explicit beat dari Python.
9. **[P2] Banjir `set_grid` saat drag.** Setiap 40 ms (`SongEditor.tsx:123-128`) terjadi upsert DB, tulis song.json, `invalidate_song` (cache audio dibuang, `player.rs:181-184`), dan `SetBeatTable`. Panggilan `spawn_blocking` paralel bisa selesai tidak berurutan, sehingga DB/engine berakhir dengan nilai di tengah drag. Sebaiknya kirim hanya saat `onEnd`, atau beri nomor urut.
10. **[P3] Lane tidak konsisten dengan toolbar dan UX lain.**
    - Lane mengabaikan Snap: Bar/Beat/Off (`TempoLane.tsx:80-86`). "Snap every marker to the bar" (`SongEditor.tsx:444-456`) tumpang tindih dengan resnap otomatis.
    - ⌘ "free" snap tidak berfungsi karena `barBeatOf` membulatkan (`tempoMap.ts:49-62`; Rust `beat: u32`, `models.rs:135`).
    - Skala lane melompat saat BPM melewati kelipatan 10 (`yScale`).
    - Panah ↑↓ tidak auto-repeat karena `e.repeat` diabaikan (`SongEditor.tsx:229`).

## 4. Cara sistematis menemukan bug, fitur setengah jadi, dan konflik

1. **Peta sumber kebenaran.** Untuk setiap nilai (BPM, Beat 1, meter, tempo map, section), catat siapa yang *menulis* (semua `set({…})`, `ipc.*`, dan command Rust) dan siapa yang *membaca*. Nilai dengan lebih dari satu penulis yang tidak lewat satu fungsi adalah kandidat konflik. Contoh: grep `set({ bpm` vs `writeGrid`.
2. **Grep "ditulis tapi tidak dibaca".** Field yang ditulis tapi tidak pernah dibaca oleh player/mixer menandakan fitur yang tidak berpengaruh (contoh: `beatGridLocked`). Komponen yang tidak diimport menandakan kode mati. Contoh: `rg -l "from \"@/components/X\""`; cek juga `knip` atau `ts-prune`.
3. **Test interaksi nyata.** Gunakan harness seperti `/workspace/wpe3/repro/` (render komponen asli + Chrome/puppeteer, `elementFromPoint`) untuk setiap kontrol drag. Test unit tidak bisa menangkap overlay/z-index.
4. **Test round-trip.** Edit lewat setiap kontrol, lalu save (`save_song`), lalu baca ulang. Nilai harus sama. Test ini akan langsung menangkap #3 dan audit #2.
5. **Satu jalur autosave.** Pilih autosave semua atau manual semua. Campuran keduanya menghasilkan bug Revert/Back.

## 5. Instruksi siap-tempel untuk agent Cursor

> Perbaiki bug "tempo tidak berubah di tempo lane" dan konflik grid. Jangan ubah algoritma analisis.
> 1. `SongTimeline.tsx:437-443`: batasi handle "Drag Beat 1" supaya tidak menutupi lane Tempo. Pilihannya: mulai handle di bawah lane tempo (top = H.ruler + H.tempo), atau beri root `TempoLane` `relative z-[35]` agar dot (`TempoLane.tsx:215`) berada di atas handle. Pastikan dot titik 1 bisa di-drag vertikal, dan klik tanpa gerakan di handle Beat 1 tidak memindah Beat 1 (hapus `setBeatDrag(apply(e.nativeEvent))` di baris 218; mulai drag setelah >3 px).
> 2. Tambahkan test puppeteer/Playwright: `elementFromPoint` di pusat dot bar 1.1 harus dot itu sendiri, dan drag ke atas 30 px harus mengubah BPM sebesar +3.0 (lihat contoh di repro).
> 3. Satu sumber kebenaran: semua kontrol BPM/Beat 1 di SongInfo (`SongEditor.tsx:875-886, 978, ~1000, 1033-1040`) harus lewat `commitBpm` / `commitBeat` → `writeGrid`, atau hapus duplikatnya dan sisakan Inspector.
> 4. Autosave grid: sesudah `set_grid` sukses, perbarui store (`upsertSong` dengan hasil `set_grid`) atau emit `library:changed`. Revert harus mengirim ulang grid `stored` ke `set_grid`. Back harus menyimpan atau menanyakan jika masih dirty. Kirim `set_grid` hanya saat `onEnd` (atau beri sequence number), bukan setiap 40 ms.
> 5. `applyMeter`: petakan ulang titik tempo dengan `q → barBeatOf(q, bpbBaru)` (pertahankan waktu), lalu `pushGrid`. `set_grid` harus memakai `grid.beatsPerBar` dari klien, bukan meter lama dari DB.
> 6. Lane memakai snap mode toolbar (Bar/Beat/Off). Header "GRID" diambil dari grid (Fixed jika 1 titik, Tempo map N titik), bukan dari `analysis.tempoMode`, jika `grid.source === "user"`.
> 7. Hapus atau sambungkan: "Lock grid" (tidak dibaca engine), `components/Waveform.tsx` (tidak dipakai), teks "flagged on waveform", docstring `click.rs:1-11`.
> Jalankan `npm test`, `npm run typecheck`, dan `cargo test` sesudahnya.
