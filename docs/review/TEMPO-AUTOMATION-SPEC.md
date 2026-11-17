# Spec: Tempo Automation Lane (Tempo Map) — Worship Playback Engine

> **Untuk:** Cursor coding agent yang bekerja di repo `Worship Playback Engine` (Tauri + Rust audio engine + React UI + Python worker).
> **Dari:** review arsitektur (lihat `REVIEW.md`, terutama **Roadmap #4 "Satu model `grid` end-to-end"** dan bagian B).
> **Mockup:** `editor-mockup.png` (sumber: `mockup/editor-mockup.html`) — lane "Tempo" tepat di bawah ruler.
>
> ⚠️ **Repo sudah bergerak sejak review.** Nama file / baris di bawah adalah *perkiraan* dari snapshot 9 Okt 2026. Agent lain juga sudah menambah zoom timeline (1×–256×, tombol Beat 1) dan mixer per-track di Live. **Verifikasi kondisi kode terbaru dulu, pakai ulang yang sudah ada, jangan tulis ulang.**

---

## 0. Ringkasan & prinsip

1. **Tempo map adalah deskripsi rekaman, bukan perintah untuk mengubah rekaman.** Audio tidak pernah di-time-stretch. Tempo map hanya menentukan *di mana* beat/bar/click/cue jatuh di atas audio yang sudah tetap. (Warping/ubah tempo playback = di luar scope, defer.)
2. **Satu sumber kebenaran** (`REVIEW.md` Roadmap #4): click, cue, count-in, ruler bar.beat, snap section, dan tampilan UI **semuanya** membaca tempo map yang sama. Rumus `bpm + beatOffset` tetap (`BeatGrid::new`) **dihapus** sebagai jalur terpisah — lagu ber-tempo konstan hanyalah tempo map dengan **1 titik**.
3. **Default = 1 titik.** Analisis hanya menambah titik kalau ada bukti kuat perubahan tempo nyata. Map yang "berisik" (puluhan region palsu seperti `_classify_stability` sekarang) lebih buruk daripada tidak ada map.
4. **Sumbu X timeline = waktu (detik), linear.** Karena audio tetap, waveform tidak boleh "melar". Konsekuensinya: **bar melebar saat tempo melambat dan menyempit saat tempo naik** — click lane terlihat lebih jarang/rapat. Ini justru yang membuat hubungan tempo ↔ click jelas buat MD (lihat mockup: ♩76 → 66, bar 3.16 s → 3.64 s).

---

## 1. Interaksi (Tempo lane)

Lane "Tempo" tinggi ±96 px, di bawah ruler, di atas Arrangement. Sumbu Y = BPM dengan label skala (default otomatis: `floor(min−5)`..`ceil(max+5)`, dibulatkan ke kelipatan 10; contoh 60–90). Garis kuning menghubungkan titik; area di bawah garis diberi gradient tipis.

| Aksi | Perilaku |
|---|---|
| **Hover** di garis | Kursor jadi pensil (mode Draw) / pointer (mode Select). Tampilkan ghost dot + hint "Double-click to add point". |
| **Double-click di garis / lane** | Tambah titik di posisi **ter-snap** (default: beat) dengan BPM = **nilai garis saat itu** (interpolasi), sehingga bentuk kurva **tidak berubah** saat titik ditambah. Curve titik baru = curve segmen yang dipotong. |
| **Klik titik** | Select (ring putih + tooltip `Bar 45.1 · 66.00 BPM`). Inspector "Selected point" terisi. |
| **Drag titik vertikal** | Ubah BPM. Normal 0.1 BPM/px (snap ke 0.1); **Shift = fine** 0.01 BPM/px. Clamp 30–300. Tooltip live. Click & ruler update live (lihat §4.4). |
| **Drag titik horizontal** | Pindah posisi, snap ke **beat** (default), ⌥ = snap ke **bar**, ⌘ = tanpa snap (1/16 beat). Tidak boleh melewati titik tetangga; jarak minimal 1 beat. Drag ditahan di satu sumbu setelah 4 px pertama (axis lock) kecuali Shift+⌘. |
| **Delete / Backspace** | Hapus titik terpilih. **Titik #1 tidak bisa dihapus** (dia adalah tempo di Beat 1; posisinya = Beat 1 dan dipindah lewat handle Beat 1). |
| **Klik kanan segmen** / tombol **R** / toggle lane header | Ganti curve segmen *keluar* dari titik terpilih: **Step** (tahan nilai sampai titik berikut, lompat tegak) ↔ **Ramp** (linear ke titik berikut). |
| **⌥-drag segmen** (opsional, v2) | Lengkungkan ramp (ease-in/out). v1 cukup `linear`; siapkan field `curve` agar bisa diperluas. |
| **Drag segmen Step vertikal** (opsional) | Geser BPM segmen itu (= titik awalnya). |
| **Toolbar lane header** | `↖ Select` / `✎ Draw (B)` · toggle `Step | Ramp` (curve default untuk titik baru & titik terpilih) · menu ⋯: "Reset to constant" (kembali ke 1 titik hasil fit), "Re-detect from audio". |
| **Undo/Redo** | ⌘Z / ⇧⌘Z. Satu drag = satu entry (commit di `pointerup`). Undo stack **sama** dengan stack editor (section, Beat 1, cue). |

Feedback visual wajib:
- Garis putus-putus vertikal tipis dari titik turun melewati Arrangement/Click lane → hubungan titik ↔ bar terlihat.
- Chip di click lane: `♩ 71.80 · bar 3.34s`, `♩ 76 → 66 · bars widen`.
- Header pill "Tempo" menampilkan BPM **di playhead** + status (`● ramp` / `● step`).
- Inspector "Beat grid": `Tempo map · N points` + tabel titik (#, Bar, BPM, Curve); kartu "Selected point" (Bar, Time, BPM dengan −/+, Curve, info segmen masuk).

### Keyboard shortcuts (saat tempo lane fokus / titik terpilih)

| Key | Aksi |
|---|---|
| `B` | Toggle Draw / Select |
| `↑ / ↓` | BPM ±0.1 · `⇧` ±0.01 · `⌥` ±1 |
| `← / →` | Geser titik 1 beat · `⇧` 1 bar |
| `Tab / ⇧Tab` | Titik berikut / sebelumnya |
| `R` | Toggle Step / Ramp |
| `⌫` | Hapus titik |
| `Esc` | Deselect |
| `⌘Z / ⇧⌘Z` | Undo / Redo |
| `Space`, `L` | Play/pause, loop seleksi (global) |

Shortcut **tidak** aktif saat fokus di input teks.

---

## 2. Data model

### 2.1 Bentuk (TS `lib/types.ts`, Rust `models.rs`, Python dict — nama field identik, camelCase di JSON)

```ts
type Curve = "step" | "linear";            // "linear" = Ramp di UI. Siapkan union untuk "ease" nanti.

interface TempoPoint {
  id: string;
  bar: number;        // 1-based, relatif ke Beat 1 (bar 1 = bar yang dimulai di Beat 1)
  beat: number;       // 1-based dalam bar, boleh pecahan (1, 2, 2.5 …)
  bpm: number;        // float, JANGAN dibulatkan (simpan ≥3 desimal)
  curve: Curve;       // bentuk segmen DARI titik ini ke titik berikutnya
}

interface Grid {
  beat1Sec: number;            // waktu Beat 1 di audio (detik) — jangkar absolut, tanpa batas 30 s
  beatsPerBar: number;         // dari time signature (4/4 → 4, 6/8 → 6 atau 2 sesuai keputusan produk)
  points: TempoPoint[];        // terurut, points[0] wajib bar=1 beat=1
  source: "analysis" | "user"; // titik dari analisis vs diedit user
  confidence?: number;         // dari analisis
  residualMs?: number;
  version: 1;
}
```

Internal semua kalkulasi pakai **posisi beat absolut** `q = (bar − 1) · beatsPerBar + (beat − 1)` (q = 0 di Beat 1, negatif = pre-roll/count-in).

### 2.2 Matematika (implementasi identik di TS, Rust, Python)

Untuk segmen dari titik i (`q0, v0`) ke titik i+1 (`q1, v1`), `L = q1 − q0`, `t0` = waktu titik i:

**Step** (atau segmen terakhir, atau pre-roll sebelum titik #1 — pakai v0 titik #1):
```
bpm(q) = v0
t(q)   = t0 + 60 · (q − q0) / v0
q(t)   = q0 + (t − t0) · v0 / 60
```

**Linear ramp** (BPM linear terhadap *beat*, seperti Ableton/Logic):
```
bpm(q) = v0 + (v1 − v0) · (q − q0) / L
t(q)   = t0 + 60 · L / (v1 − v0) · ln( bpm(q) / v0 )            (jika |v1 − v0| < 1e-9 → pakai rumus step)
q(t)   = q0 + L · v0 / (v1 − v0) · ( exp( (t − t0) · (v1 − v0) / (60 · L) ) − 1 )
```

`t0` untuk tiap titik di-akumulasi dari `beat1Sec` (titik #1 → `t0 = beat1Sec`). Precompute array `segments[{q0, t0, v0, v1, L, curve}]` sekali per perubahan map; lookup pakai binary search.

Fungsi wajib (nama sama di tiga bahasa): `beatToTime(q)`, `timeToBeat(t)`, `bpmAtBeat(q)`, `bpmAtTime(t)`, `beatTable(fromSec, toSec, sr) → [{frame, q, bar, beatInBar, isAccent}]`.

Contoh angka (dipakai di mockup & golden test): Beat 1 = 2.320 s, 4/4, titik `{1.1, 71.80, step}`, `{37.1, 76.00, linear}`, `{45.1, 66.00, step}` → bar 40.5 di **2:14.04** (71.63 BPM), bar 45 di **2:29.74**.

### 2.3 Migrasi

- Lagu lama `bpm` + `beatOffset` → `Grid{beat1Sec: beatOffset, points:[{bar:1,beat:1,bpm,curve:"step"}]}`.
- `analysis.tempoMap` format lama (`startBeat, startTime, bpm`) → titik step (konversi `startBeat` ke bar/beat), tandai `source:"analysis"`.
- `beatGridLocked` + array `beats[]` mentah → **tidak dipakai lagi** untuk click (lihat REVIEW A1: array itu bisa setengah tempo). Simpan sebagai data diagnostik saja.
- Field `song.bpm` boleh tetap ada sebagai *derived* (BPM titik #1) untuk list/setlist, tapi **jangan** dipakai oleh engine.

---

## 3. Seeding dari analisis (Python worker, `analysis/tempo.py` + `pipeline.py`)

1. **Selalu mulai dari constant fit** (lihat REVIEW B2 / `experiments/grid_fit.py`): 1 titik, `bpm` float, `beat1Sec`, `residualMs`, `confidence`.
2. **Tambah titik hanya bila terbukti** — hitung tempo lokal per bar dari beat/onset ter-assign (index integer!), lalu:
   - **Step:** change-point detection (mis. dua-segmen least squares / PELT) pada tempo lokal. Terima jika: |Δbpm| ≥ 1.0, tiap region ≥ 4 bar, residual turun ≥ 40% vs 1 titik, dan `confidence ≥ 0.7`. Snap posisi titik ke **bar line** terdekat (lebih disukai boundary section).
   - **Ramp (ritardando/accelerando, sering di 4–8 bar terakhir):** fit linear tempo lokal pada jendela kandidat; terima jika slope signifikan (|Δbpm| ≥ 2 selama jendela) dan residual turun nyata. Hasilkan 2 titik: awal ramp (`linear`) + akhir (`step`).
   - Batas wajar: maks 8 titik untuk lagu click-recorded. Kalau lebih → mode **live/drift**: hasilkan titik step per 2–4 bar, `source:"analysis"`, dan UI tampilkan badge "Live tempo · N points" + tombol "Simplify" (Douglas-Peucker pada kurva tempo, toleransi 5 ms posisi beat) — boleh defer.
3. Output ke `analysis.json` sebagai objek `grid` (bentuk §2.1) + `explain: string[]` (mis. "Ritardando 76→66 terdeteksi di Bridge (bar 37–45)").
4. **Re-analyze** dengan constraint: kalau user sudah mengedit (`source:"user"`), analisis **tidak boleh** menimpa map kecuali user menekan "Re-detect from audio" (konfirmasi + undo-able).

---

## 4. Konsumsi di Rust engine (wajib pakai map yang sama)

### 4.1 Struktur
- `models.rs`: tambah `Grid`, `TempoPoint`, `Curve` (serde camelCase) di `Song` (`song.grid`). Simpan di DB `analysis_json`/kolom baru sesuai pola repo; tulis juga ke `song.json`.
- Modul baru `audio/tempo_map.rs` (atau di `click.rs`): `TempoMap::from_grid(&Grid)` + fungsi §2.2 + `beat_table(sr, from_frame, to_frame)`.
- `click.rs`: `BeatGrid` dibangun **hanya** dari `TempoMap` (tabel beat eksplisit — jalur `explicit_beats` yang sudah ada bisa dipakai ulang). Aksen = `beatInBar == 0` dari tabel, **bukan** "beat terdekat ke offset" (`from_beats` sekarang). Hapus `BeatGrid::new(bpm, offset)` sebagai jalur produksi (boleh tinggal sebagai helper test yang membuat map 1 titik).
- `player.rs::build_beat_grid` (±baris 193–247): ganti seluruh logika lock/explicit/fixed dengan `TempoMap::from_grid(&song.grid)`.

### 4.2 Click
Render/mix click dari `beat_table`. Tidak ada `frames_per_beat` global untuk posisi beat.

### 4.3 Count-in
- Mulai dari awal lagu: count-in N bar memakai **BPM titik #1**, dan **beat terakhir count-in jatuh tepat 1 beat sebelum Beat 1**, sehingga click berikutnya = Beat 1 (frame identik dengan `beat_table`).
- Mulai dari section di tengah lagu: count-in memakai `bpmAtBeat(q_section)` (tempo lokal; kalau di tengah ramp pakai nilai di titik itu) dan berakhir tepat di downbeat section.

### 4.4 Cue
- Lead cue = **N beat sebelum bar line start section**, dihitung di ruang beat: `t_cue = beatToTime(q_section − N)`, bukan `start − N·frames_per_beat` (`player.rs` ±baris 509 sekarang). Di area ramp ini otomatis benar.
- Event cue custom (lihat REVIEW roadmap #6) disimpan sebagai posisi musikal (`bar/beat` atau relatif section), di-resolve via map.

### 4.5 Arrangement jumps
Plan segmen memetakan frame sumber → output. Click/cue tetap diambil dari `beat_table` **di koordinat sumber**, jadi lompatan sample-accurate tetap benar. Pastikan beat yang jatuh tepat di boundary tidak dobel/hilang (uji §7).

### 4.6 Live-apply tanpa reload
- Command baru `set_grid(id, grid)` (menggantikan `set_downbeat` + `lock_beat_grid` di `commands.rs`; pertahankan yang lama sebagai wrapper sementara bila UI lain masih memanggil).
- Bangun `beat_table` di thread non-realtime, kirim `MixerCmd::SetBeatTable(Arc<…>)` ke mixer (`audio/mixer.rs`), swap atomik. **Tidak ada alokasi di audio thread.**
- Debounce dari UI saat drag (mis. 30–50 ms), commit final di `pointerup`.

---

## 5. Re-layout UI saat tempo berubah

- **Ruler** (bar.beat) dan **click lane** dihitung dari `beatTable` di sisi TS (`lib/tempoMap.ts`) untuk range yang terlihat. Kepadatan label adaptif (tiap bar / 2 / 4 bar sesuai px per bar).
- **Waveform & stem lanes tidak bergerak** (sumbu waktu).
- **Section**: kebenaran tetap di audio. Simpan `startSec` + flag `snapped: "bar" | "beat" | null`.
  - Saat map berubah: section yang `snapped` di-**re-snap** ke bar/beat line terdekat dari grid baru (biasanya geser beberapa ms karena grid jadi lebih akurat). Tampilkan toast "8 section disesuaikan ke grid baru · Undo".
  - Section yang tidak di-snap (posisi bebas) tidak disentuh.
  - Label "8 bars · bar 37" di clip dihitung ulang dari map.
- **Cue lane** dihitung ulang dari section + map (§4.4).
- **fmtBarBeat** (`SongEditor.tsx`) dan semua tampilan posisi pakai `timeToBeat` dari map, bukan `60/bpm`.
- Header pill Tempo = `bpmAtTime(playhead)`.

---

## 6. File yang kemungkinan disentuh (VERIFIKASI dulu, repo sudah berubah)

| Area | File | Perubahan |
|---|---|---|
| UI | `apps/desktop/src/pages/SongEditor.tsx` | Tambah Tempo lane di timeline, inspector Beat grid (tabel titik + selected point), hapus kontrol BPM/offset terpisah yang bertentangan, wiring undo, shortcut. |
| UI | `apps/desktop/src/components/Waveform.tsx` | Grid lines & beat labels dari `beatTable` (bukan `60/bpm` konstan, baris ±87–108); snap dari map. Pertimbangkan pecah jadi `Timeline/Ruler.tsx`, `Timeline/TempoLane.tsx`, `Timeline/ClickLane.tsx`. **Pakai ulang zoom yang baru dibuat agent lain.** |
| UI | `apps/desktop/src/lib/tempoMap.ts` (baru) | Fungsi §2.2 + `beatTable` + unit test (vitest). |
| UI | `apps/desktop/src/lib/types.ts`, `lib/ipc.ts` | Tipe `Grid`/`TempoPoint`, `ipc.setGrid`. |
| Rust | `src-tauri/src/models.rs` | `Grid`, `TempoPoint`, `Curve`; migrasi saat load song. |
| Rust | `src-tauri/src/audio/click.rs` (+ `audio/tempo_map.rs` baru) | TempoMap, beat table, aksen dari bar. |
| Rust | `src-tauri/src/player.rs` | `build_beat_grid` → map; count-in & cue lead di ruang beat. |
| Rust | `src-tauri/src/audio/mixer.rs` | `SetBeatTable` live swap. |
| Rust | `src-tauri/src/commands.rs`, `prep.rs` | `set_grid`; `prep.rs` simpan `grid` dari analisis tanpa pembulatan BPM (±baris 438–441). |
| Python | `audio-worker/src/wpe_worker/analysis/tempo.py`, `pipeline.py`, `analysis/sections.py` | Output `grid` (§3); sections pakai bar grid dari map. |
| Test | `audio-worker/tests/`, `src-tauri` unit tests, `src/lib/*.test.ts` | §7. |

---

## 7. Acceptance tests

**Golden vectors lintas bahasa** — buat `fixtures/tempo_map_vectors.json` berisi beberapa map (1 titik; step; ramp naik; ramp turun; map mockup §2.2; 6/8) + daftar `(q, t, bpm)` yang diharapkan. **Python, TS, dan Rust wajib lulus file yang sama** (toleransi 0.1 ms / 1 sample @48 kHz).

Matematika
- [ ] Map 1 titik = rumus konstan, error < 0.1 ms setelah 10 menit.
- [ ] Ramp closed-form vs integrasi numerik (1/1000 beat) < 0.1 ms; `timeToBeat(beatToTime(q)) == q` (±1e-9).
- [ ] Map contoh: bar 40.5 → 134.04 s (71.63 BPM), bar 45 → 149.74 s.

Rust engine
- [ ] `beat_table` frame cocok dengan golden ±1 sample; aksen tepat di beat 1 tiap bar di dalam ramp.
- [ ] Count-in dari awal: click terakhir count-in = Beat 1 − 1 beat (tempo titik #1); click berikutnya = frame Beat 1.
- [ ] Count-in dari section di tengah ramp memakai tempo lokal dan berakhir di downbeat section.
- [ ] Cue dengan lead 4 beat di region ramp jatuh tepat di `beatToTime(q_section − 4)`.
- [ ] Arrangement jump (Chorus → Bridge → Chorus): tidak ada click dobel/hilang di boundary.
- [ ] `SetBeatTable` saat playback: tidak ada glitch/xrun, tanpa alokasi di audio thread (cek dengan test/assert yang sudah ada di mixer bila ada).

Analisis (Python, pakai generator sintetis di `experiments/repro.py` sebagai basis)
- [ ] Lagu konstan (termasuk 71.8 & 72.15, lead-in, intro tanpa drum) → **tepat 1 titik**, BPM ±0.01.
- [ ] Step 72 → 76 di bar 33 → 2 titik, posisi ±1 beat, BPM ±0.1.
- [ ] Ritardando 76 → 66 di 8 bar terakhir → titik `linear` + titik akhir, BPM ±0.5.
- [ ] Rekaman konstan dengan jitter onset ±15 ms → tetap 1 titik (tidak ada region palsu).

UI (vitest + cek manual)
- [ ] Double-click menambah titik di beat ter-snap dengan BPM interpolasi; kurva tidak berubah bentuk.
- [ ] Drag vertikal: tooltip & click lane update live; Shift = fine; drag horizontal snap beat/bar/bebas sesuai modifier; titik tidak bisa melewati tetangga.
- [ ] Delete titik #1 ditolak (dengan hint); Delete titik lain berhasil; ⌘Z mengembalikan persis.
- [ ] Toggle Step/Ramp mengubah segmen keluar; ruler, click lane, label section, dan cue re-layout.
- [ ] Section yang ter-snap ikut re-snap + toast Undo; section bebas tidak bergerak.
- [ ] Waveform tidak bergeser sama sekali saat map diedit.
- [ ] Performa: `beatTable` untuk lagu 10 menit < 5 ms di TS; drag 60 fps di MacBook Air.

Migrasi
- [ ] Lagu lama (bpm + beatOffset) terbuka dengan map 1 titik, click identik dengan sebelumnya (kecuali bug pembulatan BPM, yang memang diperbaiki).

---

## 8. Di luar scope v1 (defer)
- Kurva ease/bezier (⌥-drag), perubahan time signature di tengah lagu (siapkan field, belum UI).
- Warping / mengubah tempo playback audio.
- "Simplify" otomatis untuk map live yang padat (boleh v1.1).
- Tempo map di Live mode (cukup tampil read-only kalau perlu).
