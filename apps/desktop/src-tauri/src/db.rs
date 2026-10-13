//! SQLite persistence with versioned migrations (PRAGMA user_version).
//! Large audio files are never stored here — only paths relative to the song package.

use crate::models::*;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

const MIGRATIONS: &[&str] = &[
    // v1 — initial schema
    r#"
    CREATE TABLE songs (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        artist TEXT NOT NULL DEFAULT '',
        bpm REAL,
        song_key TEXT,
        time_signature TEXT NOT NULL DEFAULT '4/4',
        beat_offset REAL NOT NULL DEFAULT 0,
        duration REAL NOT NULL DEFAULT 0,
        sample_rate INTEGER NOT NULL DEFAULT 0,
        channels INTEGER NOT NULL DEFAULT 2,
        dir TEXT NOT NULL,
        source_path TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'imported',
        prep_json TEXT NOT NULL,
        routing_json TEXT NOT NULL,
        arrangement_json TEXT NOT NULL DEFAULT '[]',
        count_in_bars INTEGER NOT NULL DEFAULT 1,
        cue_lead_beats INTEGER NOT NULL DEFAULT 4,
        notes TEXT NOT NULL DEFAULT '',
        last_error TEXT,
        analysis_json TEXT,
        rendered_backing TEXT,
        rendered_backup TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        prepared_at TEXT
    );
    CREATE TABLE song_stems (
        song_id TEXT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
        kind TEXT NOT NULL,
        path TEXT NOT NULL,
        PRIMARY KEY (song_id, kind)
    );
    CREATE TABLE song_sections (
        id TEXT PRIMARY KEY,
        song_id TEXT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        label TEXT NOT NULL,
        kind TEXT NOT NULL,
        start REAL NOT NULL
    );
    CREATE INDEX idx_sections_song ON song_sections(song_id, position);
    CREATE TABLE setlists (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        date TEXT,
        notes TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE setlist_items (
        id TEXT PRIMARY KEY,
        setlist_id TEXT NOT NULL REFERENCES setlists(id) ON DELETE CASCADE,
        song_id TEXT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        preset_id TEXT,
        click_enabled INTEGER,
        cue_enabled INTEGER,
        foh_enabled INTEGER,
        monitor_backing INTEGER,
        start_section_id TEXT,
        arrangement_json TEXT,
        transition TEXT NOT NULL DEFAULT 'stop',
        notes TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX idx_items_setlist ON setlist_items(setlist_id, position);
    CREATE TABLE presets (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        builtin INTEGER NOT NULL DEFAULT 0,
        settings_json TEXT NOT NULL
    );
    CREATE TABLE audio_devices (
        name TEXT PRIMARY KEY,
        channels INTEGER NOT NULL,
        sample_rates_json TEXT NOT NULL,
        routing_profile_id TEXT,
        last_seen TEXT NOT NULL
    );
    CREATE TABLE routing_profiles (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        mode TEXT NOT NULL,
        foh_json TEXT NOT NULL,
        monitor_json TEXT NOT NULL
    );
    CREATE TABLE resources (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        version TEXT NOT NULL,
        status TEXT NOT NULL,
        size INTEGER NOT NULL DEFAULT 0,
        path TEXT,
        installed_at TEXT,
        meta_json TEXT
    );
    CREATE TABLE preparation_jobs (
        id TEXT PRIMARY KEY,
        song_id TEXT NOT NULL,
        song_title TEXT NOT NULL,
        status TEXT NOT NULL,
        stage TEXT NOT NULL DEFAULT '',
        progress REAL NOT NULL DEFAULT 0,
        message TEXT NOT NULL DEFAULT '',
        error TEXT,
        model TEXT,
        device TEXT,
        processing_seconds REAL,
        created_at TEXT NOT NULL,
        started_at TEXT,
        finished_at TEXT
    );
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value_json TEXT NOT NULL
    );
    "#,
];

pub struct Db {
    pub conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrate(&conn)?;
        let db = Db { conn: Mutex::new(conn) };
        db.seed_presets()?;
        Ok(db)
    }

    fn seed_presets(&self) -> Result<()> {
        let c = self.conn.lock();
        for p in crate::presets::builtin_presets() {
            c.execute(
                "INSERT INTO presets (id, name, description, builtin, settings_json) VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
                 settings_json = excluded.settings_json, builtin = 1",
                params![p.id, p.name, p.description, serde_json::to_string(&p.settings)?],
            )?;
        }
        Ok(())
    }

    // ---------- settings ----------

    pub fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let c = self.conn.lock();
        let v: Option<String> = c
            .query_row("SELECT value_json FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        v.and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn set_setting<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    pub fn all_settings(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let c = self.conn.lock();
        let mut stmt = c.prepare("SELECT key, value_json FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut map = serde_json::Map::new();
        for r in rows {
            let (k, v) = r?;
            if let Ok(val) = serde_json::from_str(&v) {
                map.insert(k, val);
            }
        }
        Ok(map)
    }

    // ---------- songs ----------

    pub fn list_songs(&self) -> Result<Vec<Song>> {
        let c = self.conn.lock();
        let mut stmt = c.prepare("SELECT * FROM songs ORDER BY title COLLATE NOCASE")?;
        let mut songs: Vec<Song> = stmt.query_map([], row_to_song)?.collect::<Result<_, _>>()?;
        for s in songs.iter_mut() {
            load_song_children(&c, s)?;
        }
        Ok(songs)
    }

    pub fn get_song(&self, id: &str) -> Result<Option<Song>> {
        let c = self.conn.lock();
        let song = c.query_row("SELECT * FROM songs WHERE id = ?1", [id], row_to_song).optional()?;
        match song {
            Some(mut s) => {
                load_song_children(&c, &mut s)?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    pub fn song_id_exists(&self, id: &str) -> Result<bool> {
        let c = self.conn.lock();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM songs WHERE id = ?1", [id], |r| r.get(0))?;
        Ok(n > 0)
    }

    pub fn upsert_song(&self, s: &Song) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute(
            "INSERT INTO songs (id, title, artist, bpm, song_key, time_signature, beat_offset, duration,
                sample_rate, channels, dir, source_path, status, prep_json, routing_json, arrangement_json,
                count_in_bars, cue_lead_beats, notes, last_error, analysis_json, rendered_backing,
                rendered_backup, created_at, updated_at, prepared_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, artist=excluded.artist, bpm=excluded.bpm,
                song_key=excluded.song_key, time_signature=excluded.time_signature, beat_offset=excluded.beat_offset,
                duration=excluded.duration, sample_rate=excluded.sample_rate, channels=excluded.channels,
                dir=excluded.dir, source_path=excluded.source_path, status=excluded.status,
                prep_json=excluded.prep_json, routing_json=excluded.routing_json,
                arrangement_json=excluded.arrangement_json, count_in_bars=excluded.count_in_bars,
                cue_lead_beats=excluded.cue_lead_beats, notes=excluded.notes, last_error=excluded.last_error,
                analysis_json=excluded.analysis_json, rendered_backing=excluded.rendered_backing,
                rendered_backup=excluded.rendered_backup, updated_at=excluded.updated_at,
                prepared_at=excluded.prepared_at",
            params![
                s.id,
                s.title,
                s.artist,
                s.bpm,
                s.key,
                s.time_signature,
                s.beat_offset,
                s.duration,
                s.sample_rate,
                s.channels,
                s.dir,
                s.source_path,
                s.status.as_str(),
                serde_json::to_string(&s.prep)?,
                serde_json::to_string(&s.routing)?,
                serde_json::to_string(&s.arrangement)?,
                s.count_in_bars,
                s.cue_lead_beats,
                s.notes,
                s.last_error,
                analysis_blob(s),
                s.rendered.backing,
                s.rendered.backup,
                s.created_at,
                s.updated_at,
                s.prepared_at,
            ],
        )?;
        tx.execute("DELETE FROM song_stems WHERE song_id = ?1", [&s.id])?;
        for (kind, path) in [
            ("vocals", &s.stems.vocals),
            ("drums", &s.stems.drums),
            ("bass", &s.stems.bass),
            ("other", &s.stems.other),
        ] {
            if let Some(p) = path {
                tx.execute(
                    "INSERT INTO song_stems (song_id, kind, path) VALUES (?1, ?2, ?3)",
                    params![s.id, kind, p],
                )?;
            }
        }
        tx.execute("DELETE FROM song_sections WHERE song_id = ?1", [&s.id])?;
        for (i, sec) in s.sections.iter().enumerate() {
            tx.execute(
                "INSERT INTO song_sections (id, song_id, position, label, kind, start) VALUES (?1,?2,?3,?4,?5,?6)",
                params![sec.id, s.id, i as i64, sec.label, sec.kind, sec.start],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_song_status(&self, id: &str, status: SongStatus, error: Option<&str>) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "UPDATE songs SET status = ?2, last_error = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, status.as_str(), error, now_iso()],
        )?;
        Ok(())
    }

    pub fn delete_song(&self, id: &str) -> Result<()> {
        let c = self.conn.lock();
        c.execute("DELETE FROM songs WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---------- setlists ----------

    pub fn list_setlists(&self) -> Result<Vec<Setlist>> {
        let ids: Vec<String> = {
            let c = self.conn.lock();
            let mut stmt =
                c.prepare("SELECT id FROM setlists ORDER BY COALESCE(date, created_at) DESC, name")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut out = vec![];
        for id in ids {
            if let Some(s) = self.get_setlist(&id)? {
                out.push(s);
            }
        }
        Ok(out)
    }

    pub fn get_setlist(&self, id: &str) -> Result<Option<Setlist>> {
        let c = self.conn.lock();
        let sl = c
            .query_row(
                "SELECT id, name, date, notes, created_at, updated_at FROM setlists WHERE id = ?1",
                [id],
                |r| {
                    Ok(Setlist {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        date: r.get(2)?,
                        notes: r.get(3)?,
                        items: vec![],
                        created_at: r.get(4)?,
                        updated_at: r.get(5)?,
                    })
                },
            )
            .optional()?;
        let Some(mut sl) = sl else { return Ok(None) };
        let mut stmt = c.prepare(
            "SELECT id, song_id, position, preset_id, click_enabled, cue_enabled, foh_enabled, monitor_backing,
                    start_section_id, arrangement_json, transition, notes
             FROM setlist_items WHERE setlist_id = ?1 ORDER BY position",
        )?;
        let items = stmt.query_map([id], |r| {
            let arr: Option<String> = r.get(9)?;
            let tr: String = r.get(10)?;
            Ok(SetlistItem {
                id: r.get(0)?,
                song_id: r.get(1)?,
                position: r.get(2)?,
                preset_id: r.get(3)?,
                click_enabled: r.get(4)?,
                cue_enabled: r.get(5)?,
                foh_enabled: r.get(6)?,
                monitor_backing: r.get(7)?,
                start_section_id: r.get(8)?,
                arrangement: arr.and_then(|a| serde_json::from_str(&a).ok()),
                transition: if tr == "auto" { Transition::Auto } else { Transition::Stop },
                notes: r.get(11)?,
            })
        })?;
        sl.items = items.collect::<Result<_, _>>()?;
        Ok(Some(sl))
    }

    pub fn save_setlist(&self, s: &Setlist) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction()?;
        tx.execute(
            "INSERT INTO setlists (id, name, date, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, date=excluded.date, notes=excluded.notes,
             updated_at=excluded.updated_at",
            params![s.id, s.name, s.date, s.notes, s.created_at, s.updated_at],
        )?;
        tx.execute("DELETE FROM setlist_items WHERE setlist_id = ?1", [&s.id])?;
        for (i, it) in s.items.iter().enumerate() {
            tx.execute(
                "INSERT INTO setlist_items (id, setlist_id, song_id, position, preset_id, click_enabled, cue_enabled,
                    foh_enabled, monitor_backing, start_section_id, arrangement_json, transition, notes)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    it.id,
                    s.id,
                    it.song_id,
                    i as i64,
                    it.preset_id,
                    it.click_enabled,
                    it.cue_enabled,
                    it.foh_enabled,
                    it.monitor_backing,
                    it.start_section_id,
                    it.arrangement.as_ref().map(|a| serde_json::to_string(a).unwrap_or_default()),
                    match it.transition {
                        Transition::Auto => "auto",
                        Transition::Stop => "stop",
                    },
                    it.notes,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_setlist(&self, id: &str) -> Result<()> {
        let c = self.conn.lock();
        c.execute("DELETE FROM setlists WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---------- presets ----------

    pub fn list_presets(&self) -> Result<Vec<Preset>> {
        let c = self.conn.lock();
        let mut stmt = c.prepare(
            "SELECT id, name, description, builtin, settings_json FROM presets ORDER BY builtin DESC, name",
        )?;
        let rows = stmt.query_map([], |r| {
            let sj: String = r.get(4)?;
            Ok(Preset {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                builtin: r.get::<_, i64>(3)? == 1,
                settings: serde_json::from_str(&sj).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_preset(&self, p: &Preset) -> Result<()> {
        let c = self.conn.lock();
        let builtin: Option<i64> = c
            .query_row("SELECT builtin FROM presets WHERE id = ?1", [&p.id], |r| r.get(0))
            .optional()?;
        if builtin == Some(1) {
            anyhow::bail!("Built-in presets cannot be modified. Duplicate it instead.");
        }
        c.execute(
            "INSERT INTO presets (id, name, description, builtin, settings_json) VALUES (?1,?2,?3,0,?4)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description,
             settings_json=excluded.settings_json",
            params![p.id, p.name, p.description, serde_json::to_string(&p.settings)?],
        )?;
        Ok(())
    }

    pub fn delete_preset(&self, id: &str) -> Result<()> {
        let c = self.conn.lock();
        c.execute("DELETE FROM presets WHERE id = ?1 AND builtin = 0", [id])?;
        Ok(())
    }

    // ---------- routing profiles & devices ----------

    pub fn save_routing_profile(&self, p: &RoutingProfile) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO routing_profiles (id, name, mode, foh_json, monitor_json) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, mode=excluded.mode, foh_json=excluded.foh_json,
             monitor_json=excluded.monitor_json",
            params![
                p.id,
                p.name,
                p.mode,
                serde_json::to_string(&p.foh)?,
                serde_json::to_string(&p.monitor)?
            ],
        )?;
        Ok(())
    }

    pub fn list_routing_profiles(&self) -> Result<Vec<RoutingProfile>> {
        let c = self.conn.lock();
        let mut stmt = c.prepare("SELECT id, name, mode, foh_json, monitor_json FROM routing_profiles")?;
        let rows = stmt.query_map([], |r| {
            let f: String = r.get(3)?;
            let m: String = r.get(4)?;
            Ok(RoutingProfile {
                id: r.get(0)?,
                name: r.get(1)?,
                mode: r.get(2)?,
                foh: serde_json::from_str(&f).unwrap_or(OutputTarget::None),
                monitor: serde_json::from_str(&m).unwrap_or(OutputTarget::None),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remember_device(&self, name: &str, channels: u16, rates: &[u32], profile: Option<&str>) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO audio_devices (name, channels, sample_rates_json, routing_profile_id, last_seen)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(name) DO UPDATE SET channels=excluded.channels, sample_rates_json=excluded.sample_rates_json,
             routing_profile_id=COALESCE(excluded.routing_profile_id, audio_devices.routing_profile_id),
             last_seen=excluded.last_seen",
            params![name, channels, serde_json::to_string(rates)?, profile, now_iso()],
        )?;
        Ok(())
    }

    // ---------- resources ----------

    pub fn set_resource(
        &self,
        id: &str,
        kind: &str,
        name: &str,
        version: &str,
        status: &str,
        size: u64,
        path: Option<&str>,
        meta: Option<&serde_json::Value>,
    ) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO resources (id, kind, name, version, status, size, path, installed_at, meta_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status, size=excluded.size, path=excluded.path,
             installed_at=excluded.installed_at, meta_json=excluded.meta_json, version=excluded.version",
            params![
                id,
                kind,
                name,
                version,
                status,
                size as i64,
                path,
                now_iso(),
                meta.map(|m| m.to_string())
            ],
        )?;
        Ok(())
    }

    pub fn remove_resource(&self, id: &str) -> Result<()> {
        let c = self.conn.lock();
        c.execute("DELETE FROM resources WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---------- jobs ----------

    pub fn save_job(&self, j: &PreparationJob) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "INSERT INTO preparation_jobs (id, song_id, song_title, status, stage, progress, message, error, model,
                device, processing_seconds, created_at, started_at, finished_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status, stage=excluded.stage, progress=excluded.progress,
                message=excluded.message, error=excluded.error, model=excluded.model, device=excluded.device,
                processing_seconds=excluded.processing_seconds, started_at=excluded.started_at,
                finished_at=excluded.finished_at",
            params![
                j.id,
                j.song_id,
                j.song_title,
                j.status,
                j.stage,
                j.progress,
                j.message,
                j.error,
                j.model,
                j.device,
                j.processing_seconds,
                j.created_at,
                j.started_at,
                j.finished_at
            ],
        )?;
        Ok(())
    }

    pub fn list_jobs(&self, limit: u32) -> Result<Vec<PreparationJob>> {
        let c = self.conn.lock();
        let mut stmt = c.prepare(
            "SELECT id, song_id, song_title, status, stage, progress, message, error, model, device,
                    processing_seconds, created_at, started_at, finished_at
             FROM preparation_jobs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(PreparationJob {
                id: r.get(0)?,
                song_id: r.get(1)?,
                song_title: r.get(2)?,
                status: r.get(3)?,
                stage: r.get(4)?,
                progress: r.get(5)?,
                message: r.get(6)?,
                error: r.get(7)?,
                model: r.get(8)?,
                device: r.get(9)?,
                processing_seconds: r.get(10)?,
                created_at: r.get(11)?,
                started_at: r.get(12)?,
                finished_at: r.get(13)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Jobs left running when the app quit can never finish; mark them failed on startup.
    pub fn recover_interrupted_jobs(&self) -> Result<()> {
        let c = self.conn.lock();
        c.execute(
            "UPDATE preparation_jobs SET status='failed', error='Interrupted — the app was closed during preparation.'
             WHERE status IN ('queued','running')",
            [],
        )?;
        c.execute(
            "UPDATE songs SET status = CASE WHEN rendered_backing IS NOT NULL THEN 'ready' ELSE 'imported' END
             WHERE status IN ('queued','processing')",
            [],
        )?;
        Ok(())
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let v = (i + 1) as i64;
        if v > version {
            conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {v}; COMMIT;"))
                .with_context(|| format!("migration v{v}"))?;
        }
    }
    Ok(())
}

pub fn schema_version(db: &Db) -> i64 {
    db.conn.lock().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0)
}

/// Persist the tempo map inside `analysis_json` so older databases need no new column.
fn analysis_blob(song: &Song) -> Option<String> {
    if song.analysis.is_none() && song.grid.is_none() {
        return None;
    }
    let mut v = song.analysis.clone().unwrap_or_else(|| serde_json::json!({}));
    if !v.is_object() {
        v = serde_json::json!({});
    }
    if let Some(g) = &song.grid {
        if let Ok(value) = serde_json::to_value(g) {
            v["grid"] = value;
        }
    }
    Some(v.to_string())
}

fn row_to_song(r: &Row) -> rusqlite::Result<Song> {
    let prep: String = r.get("prep_json")?;
    let routing: String = r.get("routing_json")?;
    let arr: String = r.get("arrangement_json")?;
    let analysis: Option<String> = r.get("analysis_json")?;
    let status: String = r.get("status")?;
    let mut song = Song {
        id: r.get("id")?,
        title: r.get("title")?,
        artist: r.get("artist")?,
        bpm: r.get("bpm")?,
        key: r.get("song_key")?,
        time_signature: r.get("time_signature")?,
        beat_offset: r.get("beat_offset")?,
        duration: r.get("duration")?,
        sample_rate: r.get("sample_rate")?,
        channels: r.get("channels")?,
        dir: r.get("dir")?,
        source_path: r.get("source_path")?,
        stems: StemPaths::default(),
        rendered: RenderedPaths {
            backing: r.get("rendered_backing")?,
            backup: r.get("rendered_backup")?,
        },
        status: SongStatus::parse(&status),
        prep: serde_json::from_str(&prep).unwrap_or_default(),
        routing: serde_json::from_str(&routing).unwrap_or_default(),
        sections: vec![],
        arrangement: serde_json::from_str(&arr).unwrap_or_default(),
        count_in_bars: r.get("count_in_bars")?,
        cue_lead_beats: r.get("cue_lead_beats")?,
        notes: r.get("notes")?,
        last_error: r.get("last_error")?,
        analysis: analysis.and_then(|a| serde_json::from_str(&a).ok()),
        grid: None,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        prepared_at: r.get("prepared_at")?,
    };
    song.grid = song
        .analysis
        .as_ref()
        .and_then(|a| a.get("grid"))
        .and_then(|g| serde_json::from_value(g.clone()).ok());
    Ok(song)
}

fn load_song_children(c: &Connection, s: &mut Song) -> Result<()> {
    let mut stmt = c.prepare("SELECT kind, path FROM song_stems WHERE song_id = ?1")?;
    let rows = stmt.query_map([&s.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (k, p) = row?;
        match k.as_str() {
            "vocals" => s.stems.vocals = Some(p),
            "drums" => s.stems.drums = Some(p),
            "bass" => s.stems.bass = Some(p),
            "other" => s.stems.other = Some(p),
            _ => {}
        }
    }
    let mut stmt =
        c.prepare("SELECT id, label, kind, start FROM song_sections WHERE song_id = ?1 ORDER BY position")?;
    let rows = stmt.query_map([&s.id], |r| {
        Ok(Section { id: r.get(0)?, label: r.get(1)?, kind: r.get(2)?, start: r.get(3)? })
    })?;
    s.sections = rows.collect::<Result<_, _>>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample_song(id: &str) -> Song {
        Song {
            id: id.into(),
            title: format!("Song {id}"),
            artist: "Test".into(),
            bpm: Some(72.0),
            key: Some("A".into()),
            time_signature: "4/4".into(),
            beat_offset: 0.0,
            duration: 200.0,
            sample_rate: 48000,
            channels: 2,
            dir: "/tmp/x".into(),
            source_path: "source/original.wav".into(),
            stems: StemPaths { vocals: Some("stems/vocals.wav".into()), ..Default::default() },
            rendered: RenderedPaths::default(),
            status: SongStatus::Imported,
            prep: PrepSettings::default(),
            routing: SongRouting::default(),
            sections: vec![
                Section { id: format!("{id}-s1"), label: "Intro".into(), kind: "intro".into(), start: 0.0 },
                Section { id: format!("{id}-s2"), label: "Verse 1".into(), kind: "verse".into(), start: 10.0 },
            ],
            arrangement: vec![],
            count_in_bars: 1,
            cue_lead_beats: 4,
            notes: String::new(),
            last_error: None,
            analysis: None,
            grid: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            prepared_at: None,
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(schema_version(&db), MIGRATIONS.len() as i64);
        migrate(&db.conn.lock()).unwrap();
        assert_eq!(schema_version(&db), MIGRATIONS.len() as i64);
        assert_eq!(db.list_presets().unwrap().len(), 4);
    }

    #[test]
    fn song_roundtrip_with_children() {
        let db = Db::open_in_memory().unwrap();
        let s = sample_song("a");
        db.upsert_song(&s).unwrap();
        let back = db.get_song("a").unwrap().unwrap();
        assert_eq!(back.sections.len(), 2);
        assert_eq!(back.stems.vocals.as_deref(), Some("stems/vocals.wav"));
        assert_eq!(back.sections[1].label, "Verse 1");
    }

    #[test]
    fn setlist_order_is_preserved() {
        let db = Db::open_in_memory().unwrap();
        for id in ["a", "b", "c"] {
            db.upsert_song(&sample_song(id)).unwrap();
        }
        let item = |id: &str, song: &str| SetlistItem {
            id: id.into(),
            song_id: song.into(),
            position: 0,
            preset_id: None,
            click_enabled: None,
            cue_enabled: None,
            foh_enabled: None,
            monitor_backing: None,
            start_section_id: None,
            arrangement: None,
            transition: Transition::Stop,
            notes: String::new(),
        };
        let mut sl = Setlist {
            id: "sl".into(),
            name: "Sunday".into(),
            date: None,
            notes: String::new(),
            items: vec![item("1", "c"), item("2", "a"), item("3", "b")],
            created_at: now_iso(),
            updated_at: now_iso(),
        };
        db.save_setlist(&sl).unwrap();
        let back = db.get_setlist("sl").unwrap().unwrap();
        let order: Vec<_> = back.items.iter().map(|i| i.song_id.as_str()).collect();
        assert_eq!(order, vec!["c", "a", "b"]);
        // reorder
        sl.items.swap(0, 2);
        db.save_setlist(&sl).unwrap();
        let back = db.get_setlist("sl").unwrap().unwrap();
        let order: Vec<_> = back.items.iter().map(|i| i.song_id.as_str()).collect();
        assert_eq!(order, vec!["b", "a", "c"]);
        // deleting a song removes it from setlists
        db.delete_song("a").unwrap();
        assert_eq!(db.get_setlist("sl").unwrap().unwrap().items.len(), 2);
    }

    #[test]
    fn builtin_presets_are_protected() {
        let db = Db::open_in_memory().unwrap();
        let mut p = db.list_presets().unwrap().into_iter().find(|p| p.id == "FULL_BAND").unwrap();
        p.name = "Hacked".into();
        assert!(db.save_preset(&p).is_err());
    }
}
