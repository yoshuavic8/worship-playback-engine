//! Transpose −4…+4 semitones. Stems are pre-rendered OFFLINE by the Python worker
//! (Signalsmith Stretch, MIT) into `<song>/transpose/<p2|m3>/`, one folder per key.
//! Live playback only picks a finished folder when a song is loaded; it never renders.
//!
//! Only tonal stems are pitched (keys/guitars, bass, guide vocal). Drums, the click and
//! the voice cues are never pitched; a rendered backing (which contains drums) is never
//! used for a transposed key.

use crate::core::Ctx;
use crate::logs::{self, cat};
use crate::audio::mixer::TrackRole;
use crate::models::Song;
use anyhow::{anyhow, Result};
use crossbeam_channel::{unbounded, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

pub const MAX: i8 = 4;

pub fn clamp(semitones: i32) -> i8 {
    semitones.clamp(-(MAX as i32), MAX as i32) as i8
}

/// Folder name for a key: +2 → "p2", −3 → "m3".
pub fn tag(semitones: i8) -> String {
    if semitones >= 0 {
        format!("p{semitones}")
    } else {
        format!("m{}", -semitones)
    }
}

/// The stem a track role plays from, when that role is transposable. Drums, backing
/// (contains drums), backup, original, click and cues are never pitched.
pub fn pitched_stem(role: TrackRole) -> Option<&'static str> {
    match role {
        TrackRole::Other => Some("other"),
        TrackRole::Bass => Some("bass"),
        TrackRole::Vocals => Some("vocals"),
        _ => None,
    }
}

pub fn dir(song: &Song, semitones: i8) -> PathBuf {
    PathBuf::from(&song.dir).join("transpose").join(tag(semitones))
}

/// True when every tonal stem of this key has been rendered.
pub fn is_ready(song: &Song, semitones: i8) -> bool {
    semitones == 0 || dir(song, semitones).join("done.json").is_file()
}

/// Keys (≠ 0) with a finished render.
pub fn cached_keys(song: &Song) -> Vec<i8> {
    (-MAX..=MAX).filter(|&s| s != 0 && is_ready(song, s)).collect()
}

/// Path to play for `role` at `semitones`, or None to use the original file.
pub fn track_path(song: &Song, role: TrackRole, semitones: i8) -> Option<PathBuf> {
    if semitones == 0 || !is_ready(song, semitones) {
        return None;
    }
    let p = dir(song, semitones).join(format!("{}.wav", pitched_stem(role)?));
    p.is_file().then_some(p)
}

pub fn has_stems(song: &Song) -> bool {
    crate::library::file_exists(song, &song.stems.other) && crate::library::file_exists(song, &song.stems.bass)
}

/// Remove every cached key (the stems changed, e.g. after preparing again).
pub fn clear(song: &Song) {
    let _ = std::fs::remove_dir_all(PathBuf::from(&song.dir).join("transpose"));
}

// ---------------- queue ----------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransposeJob {
    pub song_id: String,
    pub title: String,
    pub semitones: i8,
    /// queued | running | done | failed
    pub state: String,
    pub progress: f32,
    pub message: String,
}

#[derive(Default)]
pub struct TransposeQueue {
    tx: Mutex<Option<Sender<(String, i8)>>>,
    jobs: Mutex<HashMap<String, TransposeJob>>,
}

fn job_key(song_id: &str, s: i8) -> String {
    format!("{song_id}:{s}")
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum Ev {
    Progress {
        progress: f32,
        #[serde(default)]
        message: String,
    },
    Log {
        message: String,
    },
    Result {},
    Error {
        message: String,
        #[serde(default)]
        details: Option<String>,
    },
}

impl TransposeQueue {
    pub fn start(&self, ctx: Arc<Ctx>) {
        let (tx, rx) = unbounded::<(String, i8)>();
        *self.tx.lock() = Some(tx);
        std::thread::Builder::new()
            .name("wpe-transpose".into())
            .spawn(move || {
                while let Ok((song_id, s)) = rx.recv() {
                    // Never compete with live playback for CPU: wait until the music stops.
                    while ctx.player.is_rolling(&ctx) {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    run(&ctx, &song_id, s);
                }
            })
            .expect("spawn transpose queue");
    }

    pub fn jobs(&self) -> Vec<TransposeJob> {
        let mut v: Vec<_> = self.jobs.lock().values().cloned().collect();
        v.sort_by(|a, b| a.title.cmp(&b.title).then(a.semitones.cmp(&b.semitones)));
        v
    }

    /// Queue a render of `semitones` for a song (no-op when cached or already queued).
    pub fn enqueue(&self, ctx: &Ctx, song: &Song, semitones: i8) -> Result<bool> {
        if semitones == 0 || is_ready(song, semitones) {
            return Ok(false);
        }
        if !has_stems(song) {
            return Err(anyhow!("\"{}\" has no separated stems yet. Prepare it first.", song.title));
        }
        let key = job_key(&song.id, semitones);
        {
            let mut jobs = self.jobs.lock();
            if jobs.get(&key).is_some_and(|j| j.state == "queued" || j.state == "running") {
                return Ok(false);
            }
            let job = TransposeJob {
                song_id: song.id.clone(),
                title: song.title.clone(),
                semitones,
                state: "queued".into(),
                progress: 0.0,
                message: "Waiting".into(),
            };
            let _ = ctx.app.emit("transpose:progress", &job);
            jobs.insert(key, job);
        }
        self.tx
            .lock()
            .as_ref()
            .ok_or_else(|| anyhow!("transpose queue not running"))?
            .send((song.id.clone(), semitones))?;
        Ok(true)
    }

    fn update(&self, ctx: &Ctx, song_id: &str, s: i8, f: impl FnOnce(&mut TransposeJob)) {
        let mut jobs = self.jobs.lock();
        if let Some(j) = jobs.get_mut(&job_key(song_id, s)) {
            f(j);
            let _ = ctx.app.emit("transpose:progress", &*j);
        }
    }
}

fn run(ctx: &Ctx, song_id: &str, s: i8) {
    let q = &ctx.transpose;
    let fail = |msg: String| {
        logs::warn(cat::PREPARATION, format!("transpose {s:+} failed: {msg}"));
        q.update(ctx, song_id, s, |j| {
            j.state = "failed".into();
            j.message = msg;
        });
    };
    let song = match ctx.db.get_song(song_id) {
        Ok(Some(s)) => s,
        _ => return fail("The song no longer exists.".into()),
    };
    if is_ready(&song, s) {
        return q.update(ctx, song_id, s, |j| {
            j.state = "done".into();
            j.progress = 1.0;
        });
    }
    q.update(ctx, song_id, s, |j| {
        j.state = "running".into();
        j.message = "Starting".into();
    });
    if let Err(e) = ensure_stretch(ctx) {
        return fail(format!("{e:#}"));
    }
    let paths = ctx.paths.read().clone();
    let stems: HashMap<&str, &Option<String>> = [
        ("other", &song.stems.other),
        ("bass", &song.stems.bass),
        ("vocals", &song.stems.vocals),
        ("drums", &song.stems.drums),
    ]
    .into_iter()
    .filter(|(_, p)| crate::library::file_exists(&song, p))
    .collect();
    let request = serde_json::json!({
        "type": "transpose_song",
        "songId": song.id,
        "songDir": song.dir,
        "stems": stems,
        "semitones": [s],
    });
    let req_path = paths.cache().join(format!("transpose-{}-{}.json", song.id, tag(s)));
    if let Err(e) = std::fs::write(&req_path, request.to_string()) {
        return fail(format!("cannot write request: {e}"));
    }
    let started = Instant::now();
    let child = Command::new(paths.python_bin())
        .args(["-m", "wpe_worker", "transpose", "--request"])
        .arg(&req_path)
        .current_dir(&ctx.worker_dir)
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONPATH", ctx.worker_dir.join("src"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return fail(format!("could not start worker: {e}")),
    };
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let mut ok = false;
    let mut error: Option<String> = None;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let line = line.trim();
            if !line.starts_with('{') {
                continue;
            }
            match serde_json::from_str::<Ev>(line) {
                Ok(Ev::Progress { progress, message }) => q.update(ctx, song_id, s, |j| {
                    j.progress = progress.clamp(0.0, 1.0);
                    j.message = message;
                }),
                Ok(Ev::Log { message }) => logs::info(cat::PREPARATION, message),
                Ok(Ev::Result {}) => ok = true,
                Ok(Ev::Error { message, details }) => {
                    if let Some(d) = details {
                        logs::warn(cat::PREPARATION, format!("transpose details: {d}"));
                    }
                    error = Some(message)
                }
                Err(_) => {}
            }
        }
    }
    let _ = child.wait();
    let stderr_text = err_thread.join().unwrap_or_default();
    let _ = std::fs::remove_file(&req_path);
    if let Some(e) = error {
        return fail(e);
    }
    if !ok || !is_ready(&song, s) {
        let tail: String = stderr_text.chars().rev().take(600).collect::<Vec<_>>().into_iter().rev().collect();
        return fail(format!("worker exited without a result. {tail}"));
    }
    logs::info(
        cat::PREPARATION,
        format!("\"{}\" rendered in {} ({:.1}s)", song.title, crate::keys::transposed_name(song.key.as_deref(), s), started.elapsed().as_secs_f64()),
    );
    q.update(ctx, song_id, s, |j| {
        j.state = "done".into();
        j.progress = 1.0;
        j.message = "Ready".into();
    });
    // The loaded song switches to the new key only while it sits at the start.
    let wanted = ctx.db.get_song(song_id).ok().flatten().map(|x| x.transpose) == Some(s);
    if wanted && ctx.player.loaded().is_some_and(|l| l.song_id == song_id) && ctx.player.is_idle(ctx) {
        let _ = ctx.player.reload(ctx);
    }
    let _ = ctx.app.emit("library:changed", ());
}

/// Environments installed before v0.3.0 lack python-stretch: add it once (uv sync is incremental).
fn ensure_stretch(ctx: &Ctx) -> Result<()> {
    let py = ctx.paths.read().python_bin();
    if !py.exists() {
        return Err(anyhow!("The audio engine (Python) is not installed. Open Diagnostics → Setup."));
    }
    let probe = |py: &PathBuf| {
        Command::new(py)
            .args(["-c", "import python_stretch"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if probe(&py) {
        return Ok(());
    }
    logs::info(cat::SYSTEM, "python-stretch missing, updating the Python environment");
    crate::resources::sync_python_env(ctx)?;
    if probe(&py) {
        Ok(())
    } else {
        Err(anyhow!("Could not install Signalsmith Stretch. Check the internet connection and try again."))
    }
}

/// Save a song's key and queue its render. Returns the updated song.
pub fn set_song_key(ctx: &Ctx, song_id: &str, semitones: i32) -> Result<Song> {
    let mut song = ctx.db.get_song(song_id)?.ok_or_else(|| anyhow!("Song not found"))?;
    let s = clamp(semitones);
    if s != 0 && !has_stems(&song) {
        return Err(anyhow!("Transpose needs separated stems. Prepare \"{}\" first.", song.title));
    }
    song.transpose = s;
    ctx.db.set_song_transpose(&song.id, s)?;
    song.transposed = cached_keys(&song);
    if s != 0 && !is_ready(&song, s) {
        ctx.transpose.enqueue(ctx, &song, s)?;
    }
    // Apply now only when this song is loaded and NOT playing (start of the song).
    if ctx.player.loaded().is_some_and(|l| l.song_id == song.id) && ctx.player.is_idle(ctx) {
        let _ = ctx.player.reload(ctx);
    }
    let _ = ctx.app.emit("library:changed", ());
    Ok(song)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_selection_never_pitches_drums_or_mixes() {
        assert_eq!(pitched_stem(TrackRole::Other), Some("other"));
        assert_eq!(pitched_stem(TrackRole::Bass), Some("bass"));
        assert_eq!(pitched_stem(TrackRole::Vocals), Some("vocals"));
        for r in [TrackRole::Drums, TrackRole::Backing, TrackRole::Backup, TrackRole::Original] {
            assert_eq!(pitched_stem(r), None, "{r:?} must never be pitched");
        }
    }

    #[test]
    fn tags_and_clamp() {
        assert_eq!(tag(2), "p2");
        assert_eq!(tag(-3), "m3");
        assert_eq!(clamp(9), 4);
        assert_eq!(clamp(-7), -4);
    }

    #[test]
    fn uses_cache_only_when_complete() {
        let tmp = std::env::temp_dir().join(format!("wpe-tr-{}", crate::models::new_id()));
        let mut song = crate::db::tests::sample_song("t");
        song.dir = tmp.to_string_lossy().into();
        let d = dir(&song, -2);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("other.wav"), b"x").unwrap();
        // Not finished yet: no done.json → original file.
        assert!(track_path(&song, TrackRole::Other, -2).is_none());
        std::fs::write(d.join("done.json"), b"{}").unwrap();
        assert_eq!(track_path(&song, TrackRole::Other, -2), Some(d.join("other.wav")));
        assert!(track_path(&song, TrackRole::Drums, -2).is_none(), "drums stay original");
        assert!(track_path(&song, TrackRole::Other, 0).is_none());
        assert_eq!(cached_keys(&song), vec![-2]);
        clear(&song);
        assert!(cached_keys(&song).is_empty());
        let _ = std::fs::remove_dir_all(tmp);
    }
}
