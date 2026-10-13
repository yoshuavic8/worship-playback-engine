# UX-REVIEW: Apakah WPE sekarang "mudah dan manusiawi"? (13 Okt 2026)

**Snapshot:** diambil 15:40 WIB.
- `git status` = 75 entri, belum di-commit.
- File terakhir diubah 15:35 (`settings.rs`, `commands.rs`).
- Repo Mac hanya dibaca. Tarball sudah dihapus.

## 1. Verifikasi klaim agent Cursor

| Klaim | Hasil | Bukti |
|---|---|---|
| Konfirmasi saat keluar Live | ✅ ada | `Live.tsx:104-108, 124, 129`. Di browser, tombol "← Setlist" saat lagu berjalan memunculkan "Playback is still active. End live and stop the music?". Menekan Batal membuat app tetap di Live. ⚠️ Memakai `window.confirm` native. Perlu dicek di WKWebView/Tauri apakah dialognya benar-benar muncul (bisa saja langsung mengembalikan false). Lebih aman diganti dialog in-app. |
| Esc = panic hanya di Live, Shift+Esc di tempat lain | ✅ | `useShortcuts.ts:15-21`. Esc di laci editor tidak memicu panic (status tetap PLAYING). Esc di Live memicu PANIC. |
| Lane placeholder untuk lagu tanpa BPM | ✅ | `SongTimeline.tsx:360`. Screenshot `03-song-6-still-waters.png`: lane sejajar, "Set tempo in the Beat grid panel →". |
| Tempo/Beat 1/meter hanya di Inspector; Mix diganti "Song settings" | ✅ | Laci sekarang berisi Notes, Count-in, Live arrangement, Backing, Monitor, Rehearsal mixer, Assistant (`22-song-settings.png`). Key juga pindah ke Inspector. |
| Deteksi warning basi (`reviewWarnings.ts`) | ✅ sebagian | Banner menampilkan "Superseded after your tempo map edits…" (`03-song-3-open-heaven-skies.png`). Namun header GRID masih "Changing" untuk tempo map 1 titik, dan kata "your edits" muncul padahal user belum mengedit. |
| Lock grid dihapus | ✅ di UI/IPC | Sisa di backend: `prep.rs` (4 ref), `ai.rs` (1 ref), `types.ts:84`. Tidak berbahaya. |
| fmtDate | ✅ | `Live.tsx:442` |
| Toast untuk Rescan/test/dll. | ✅ | Sweep: tombol tanpa efek turun dari 30+ menjadi hampir nol. Sisanya hanya tombol yang memang sudah terpilih (4/4, 256, 1×), TAP (butuh ≥2 ketukan), dan handle drag. |
| Space me-load lalu memutar | ✅ | Space di editor menghasilkan PLAYING. |
| Overlay "Prepare to separate" | ✅ | still-waters / rise-again |
| Display language EN/ID | ⚠️ parsial, dan jujur | Hanya tanggal yang berubah. Label tetap Inggris (`Settings.tsx:83`). |
| Mock "Desktop app only" | ✅ | `dev/mock.ts:13`. Efek samping: toast muncul saat masuk Live di browser (is_fullscreen). |
| Harness dipindah ke `src/dev` | ✅ | ⚠️ Tapi `src/tempoLane.hit.test.ts` memakai path Chrome Mac hardcoded, sehingga **gagal di box/Linux/CI** ("executable doesn't exist at /Applications/Google Chrome.app"). |
| Zoom maks 32× | ✅ | `editorZoom.ts:2` |
| Snap menu, popover shortcuts "?" | ✅ | `SongTimeline.tsx:299, 825-833` |
| IPC yang tidak terpakai dihapus | ✅ | |
| npm test 24 lulus | ⚠️ | Di box: 23 lulus, 1 dilewati, dan 1 file gagal (hit test di atas). Kemungkinan lulus di Mac. |
| typecheck OK | ✅ | tsc di box exit 0 |
| cargo test 60 lulus | ✅ | Dijalankan ulang di Mac: 60/60 |

Tidak ada regresi fungsional yang ditemukan. Console error: 0.

## 2. Verdict

**Lebih mudah, ya. Setara Apple/MultiTracks, belum.**

Mode Live sekarang benar-benar nyaman untuk hari Minggu:
- Satu klik "Go live" dari Dashboard.
- Kartu NOW/NEXT besar.
- Klik section untuk antre, Shift-klik untuk lompat sekarang, L untuk loop.
- Pengaman keluar, dan PANIC yang tidak lagi muncul tidak sengaja.

Untuk worship leader biasa, alur Minggu pagi sudah lebih sederhana daripada banyak tool. Masalahnya ada di dua tempat:
1. **Editor masih DAW.** Ada tempo lane, Snap, Step/Ramp, zoom 32×, dan 10 lane. Di sinilah music director tanpa pengalaman DAW akan tersesat.
2. **Fitur inti yang membuat MultiTracks Playback jadi standar gereja belum ada:**
   - transpose/key change dan tempo change,
   - transisi antar lagu (crossfade/pad),
   - Pad Player,
   - kontrol MIDI/foot pedal,
   - kirim cue ke lirik (ProPresenter),
   - remote dari iPad.

WPE unggul di hal yang MultiTracks tidak punya:
- pemisahan stem lokal dari lagu apa pun (MultiTracks butuh katalog berbayar),
- analisis BPM/Beat 1 otomatis,
- AI lokal,
- privasi offline.

## 3. Walkthrough berbasis tugas (music director tanpa pengalaman DAW)

### (a) Minggu pagi: semua di Live

| Langkah | Aksi | Klik/tombol | Friksi |
|---|---|---|---|
| Buka app, mulai setlist | Dashboard menampilkan kartu "Next service", tekan **Go live** | 1 | Bagus. Ibadah berikutnya otomatis tampil. |
| Putar lagu 1 | Space / tombol ▶ besar | 1 | Count-in 2 bar terlihat. |
| Lompat ke chorus | Klik tile Chorus (antre di akhir section) atau Shift-klik (sekarang) | 1 | Ini konsep "Live ReOrder" MultiTracks, dan mudah dipahami. Shift-klik tidak bisa ditemukan sendiri tanpa membaca teks kecil. |
| Loop bridge | Shift-klik Bridge, lalu L | 2 | Belum ada "infinite loop preset" per section. Label kartu NEXT menjadi ambigu bila loop aktif bersamaan dengan antrean: kartu menulis "LOOPING" + nama section antrean (`Live.tsx:86, 227`). |
| Lagu berikutnya | N **ditolak saat lagu berjalan** ("Stop or fade the current song first", `transport.ts:76-80`). Harus F (fade) / S, lalu N, lalu Space. | 3 | Aman, tapi kaku. MultiTracks punya AutoLink/Crossfade/Cue Next Song, dan transisi "Auto" WPE hanya bekerja di akhir lagu. |
| Salah: ulang section | Shift-klik section yang sedang diputar | 1 | Tidak ada tombol "Restart section" yang terlihat. |
| Darurat | Esc = PANIC; tombol PANIC merah selalu terlihat | 1 | Bagus. |

Total untuk satu lagu lengkap dengan variasi sekitar 10 aksi. **Tidak ada dukungan foot pedal/MIDI**, padahal di panggung tangan pemain sedang memegang gitar atau keyboard.

### (b) Persiapan hari kerja

| Langkah | Aksi | Klik | Friksi |
|---|---|---|---|
| Tambah lagu | Import songs, lalu dialog file (atau drag & drop) | 2–3 | **Tidak ada impor dari YouTube/URL** (grep yt-dlp kosong). Banyak gereja mulai dari YouTube. |
| Prepare | Toast "Imported… Prepare", atau tombol Prepare / Prepare all | 1 | Tidak otomatis. Antrean berjalan dengan tahapan yang jelas. |
| Cek Beat 1 / tempo | Buka lagu, dengarkan, perbaiki di Inspector Beat grid (Tempo, TAP, ÷2/×2, Beat 1 ±10 ms / ±beat / Playhead) | 3–8 | Sudah satu tempat. Masih banyak istilah: "Beat 1", "Step", "GRID Changing". |
| Perbaiki section | Double-click lane Arrangement / M di playhead, drag tepi clip, rename di Inspector | 3–6 per section | Pola DAW. Tidak ada "tap section saat lagu berjalan" yang lebih manusiawi. |
| Key / transpose | Key hanya **label**. **Tidak ada transpose audio.** | — | Gap MVP besar (MultiTracks: semua 12 kunci). |
| Tambah ke setlist | Setlist, lalu Add songs (dialog multi-pilih) | 3 | Bagus. Pre-service check otomatis. |

### (c) Latihan

| Tugas | Aksi | Friksi |
|---|---|---|
| Mute/solo stem | M/S di timeline editor, mute di Live, slider di "Rehearsal mixer" | **Ada tiga tempat** untuk mix yang sama (timeline, Song settings → Rehearsal mixer, Live Tracks). Live hanya punya Mute, tanpa Solo. |
| Mix per lagu | Preset Backing (Full Band/No Drummer/…), Bass dB di Song settings | Preset bagus dan bahasanya manusiawi ("No Drummer"). |
| Click/cue ke in-ear | Devices: profil Multi-output / L/R split / Rehearsal. Live: toggle Click/Voice cues. | Routing sudah sangat matang. Tidak ada subdivisi click atau pilihan click per lagu. |

## 4. Matriks gap vs MultiTracks Playback

| Fitur | MultiTracks Playback | WPE sekarang | Prioritas MVP |
|---|---|---|---|
| Setlist + urutan + notes | Ya | **Ya** (`SetlistEditor.tsx`) | — |
| Peta section + jump live (Live ReOrder) | Ya (Premium) | **Ya**: klik = antre, Shift = sekarang (`Live.tsx`, `mixer.rs:440-455`) | — |
| Loop / Infinite Loop preset per section | Ya | **Parsial**: loop live (L). Belum ada preset per section. | P2 |
| Edit aransemen (tambah/hapus/ulang section) | Ya | **Ya**: Live arrangement (`Arrangement.tsx`) | — |
| Guide cue suara ("Verse", "Chorus") | Ya, dinamis | **Ya**: TTS macOS, lead-beat bisa diatur (`cue.rs`) | — |
| Click + count-in | Ya, dengan subdivisi | **Parsial**: click, 4 suara, count-in. Tanpa subdivisi. | P3 |
| Transpose / key change | Ya, 12 kunci | **Tidak** (key hanya label) | **P0** |
| Tempo change (time-stretch) | Ya | **Tidak** (tempo map hanya untuk click) | **P1** |
| Transisi lagu (AutoLink/Overlap/Crossfade/Cue Next) | Ya | **Parsial**: hanya Stop/Auto (`models.rs:284-289`). N diblokir saat lagu berjalan. | **P0** |
| Pad Player (pad ambient mengikuti kunci) | Ya | **Tidak** | **P1** |
| Fade out | Ya (Auto Fade) | **Ya**: F (`transport.ts`) | — |
| Mixer: mute/solo/level per track | Ya | **Ya** (4 stem). Live tanpa solo. | P3 |
| Routing multi-output / bus in-ear | Ya (Buses, SmartRouting, MultiOuts) | **Ya**: FOH/MON, profil, test (`Devices.tsx`, `routing.rs`) | — |
| Kontrol MIDI / foot pedal | Ya | **Tidak** | **P0** |
| MIDI out ke lirik/lampu (ProPresenter) | Ya (Live Bundle) | **Tidak** | P2 |
| Remote (iPad/iPhone) | Ya | **Tidak** | P2 |
| Sinkronisasi cloud antar perangkat | Ya (Cloud Pro) | **Tidak** (ada export/import library JSON) | P3 |
| Konten / katalog | Katalog multitrack berbayar | **Pemisahan stem lokal dari file apa pun** (keunggulan WPE) | — |
| Impor dari YouTube/URL | — | **Tidak** | P1 (kebutuhan nyata gereja lokal) |
| Atur titik awal/akhir lagu + fade akhir | Ya | **Parsial**: `startSectionId` per item setlist. Belum ada trim akhir. | P2 |
| Otomasi (mute/volume per section) | Ya (Premium) | **Tidak** | P3 |
| Lokalisasi | ES/PT/FR | **Parsial**: hanya tanggal (`Settings.tsx:83`) | P1 (Bahasa Indonesia) |
| Redundansi (backup player) | PlayAudio12 | **Parsial**: render backup (`rendered.backup`), fallback "original" | P3 |

## 5. UX vs standar Apple (MainStage/Logic)

**Sudah terasa seperti Apple**
- Pemisahan mode mirip MainStage: Live adalah layar penuh tanpa sidebar, dengan kontrol besar (MainStage Perform mode: "screen controls as large as possible"). Dashboard, Songs, dan editor adalah "Edit mode".
- Hierarki tipografi, warna status yang konsisten, empty state yang jelas, toast yang informatif.
- Preset dengan bahasa manusia ("No Drummer", "Full Track") mirip Smart Controls: satu kontrol berlabel yang mengatur banyak parameter.
- Diagnostics "Is this Mac ready for Sunday?" dan pre-service check.

**Belum terasa Apple, dan perbaikannya**
1. **Editor = DAW, bukan "Smart".** Jadikan mode *Simple* sebagai default. Isinya hanya tiga kartu:
   - "Tempo & Beat 1" dengan tombol besar: *Dengarkan* / *Terlalu cepat ÷2* / *Geser ke ketukan berikutnya*.
   - "Section" sebagai daftar yang bisa diketuk saat lagu diputar: *Tandai Chorus sekarang*.
   - "Mix" berupa preset.

   Tempo lane, Snap, dan Step/Ramp dipindah ke "Advanced", seperti Smart Controls vs Mixer di Logic.
2. **Satu tempat untuk mix.** Saat ini ada 3 tempat (timeline M/S, Rehearsal mixer, Live Tracks). Pertahankan mixer yang sama persis di Live dan editor, termasuk Solo di Live.
3. **Transisi Live yang mengalir.** Ganti blokir "Stop or fade first" (`transport.ts:78-80`) dengan "N = Cue next song: fade 4 detik lalu siap". Tambahkan pilihan crossfade/overlap per item setlist.
4. **Bahasa.** Pilih Bahasa Indonesia penuh (atau toggle yang benar-benar menerjemahkan label). Saat ini UI Inggris dengan contoh AI berbahasa Indonesia, dan "Display language" hanya mengubah tanggal. Hindari jargon: "Beat 1" → "Ketukan pertama", "GRID Changing" → "Tempo berubah-ubah — cek".
5. **Dialog native dan pesan teknis.**
   - Ganti `window.confirm` dengan dialog in-app bertombol besar ("Tetap di Live" / "Akhiri").
   - Header GRID sebaiknya mengikuti tempo map, bukan `analysis.tempoMode`.
   - Kartu NEXT harus jelas saat loop dan antrean aktif bersamaan.
   - Test hit (`tempoLane.hit.test.ts`) jangan bergantung pada path Chrome di Mac.

## 6. Screenshot terbaik (`/workspace/wpe5/shots/`)

- `21-live-loop-bridge.png`: Live (NOW/NEXT, tile section, loop, tracks). Juga memperlihatkan ambiguitas "LOOPING Chorus".
- `22-song-settings.png`: laci "Song settings" baru, tanpa duplikat tempo.
- `03-song-6-still-waters.png`: placeholder lane tanpa BPM dan overlay "Prepare to separate".
- `03-song-3-open-heaven-skies.png`: warning basi ("Superseded…") vs header "GRID Changing".

## Sumber

- MultiTracks Playback User Guide (transisi AutoLink/Overlap/Crossfade/Cue Next Song/Stay In Song, Live ReOrder, Section Loop, Infinite Loop, Pad Player): https://helpcenter.multitracks.com/en/articles/4944485-playback-user-guide
- Halaman fitur Playback (Live Crossfade, Built-in Click & subdivision, Dynamic Guide Cues, Auto Fade, Live Loop, MIDI Navigation, lokalisasi): https://www.multitracks.com/products/playback/features/ dan https://www.multitracks.com/products/playback/
- App Store Playback (transpose 12 kunci, tempo change, Buses, SmartRouting, MultiOuts, MIDI mapping, MIDI cues, Remote, Cloud Pro): https://apps.apple.com/us/app/playback/id751755884
- Pad Player: https://www.multitracks.com/products/padplayer
- MIDI controller / foot pedal: https://helpcenter.multitracks.com/en/articles/4490527-how-to-setup-a-midi-controller-to-control-playback dan https://helpcenter.multitracks.com/en/articles/8280193-midi-mapping-controls-explained
- ProPresenter MIDI cues: https://helpcenter.multitracks.com/en/articles/5354284-midi-out-series-2-connect-playback-and-propresenter-via-a-midi-network
- MainStage Perform mode (layar penuh, kontrol sebesar mungkin, inspector disembunyikan): https://help.apple.com/mainstage/mac/2.2/help/English.lproj/en/mainstage/usermanual/chapter_3_section_4.html
- MainStage Layout/Edit/Perform: https://help.apple.com/pdf/mainstage/en_US/mainstage-user-guide.pdf
- Logic Pro Smart Controls ("adjust the most important parameters… without opening the Mixer"): https://help.apple.com/pdf/logicpromac/en_US/logic-pro-mac-user-guide.pdf
