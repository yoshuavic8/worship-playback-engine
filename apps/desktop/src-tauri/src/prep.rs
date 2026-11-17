//! Preparation queue. Songs are prepared strictly one at a time (24 GB unified memory target):
//! the Python worker is a short-lived local process per job that talks JSON lines over stdout,
//! so model memory is fully released after every song. Nothing is exposed on the network.

use crate::core::Ctx;
use crate::logs::{self, cat};
use crate::models::*;
use crate::paths::find_tool;
use anyhow::{anyhow, Result};
use crossbeam_channel::{unbounded, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PrepOptions {
    pub separate: bool,
    pub analyze: bool,
    /// Overwrite manually edited BPM / key / sections with detected values.
    pub overwrite_manual: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum WorkerEvent {
    Progress {
        stage: String,
        progress: f32,
        #[serde(default)]
        message: String,
    },
    Log {
        #[serde(default)]
        level: String,
        message: String,
    },
    Result(Box<WorkerResult>),
    Error {
        code: String,
        message: String,
        #[serde(default)]
        details: Option<String>,
    },
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct WorkerResult {
    stems: StemPaths,
    rendered: RenderedPaths,
    analysis: Option<WorkerAnalysis>,
    separator: Option<serde_json::Value>,
    warnings: Vec<String>,
    timings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct WorkerAnalysis {
    bpm: Option<f64>,
    bpm_confidence: Option<f64>,
    beat_offset: Option<f64>,
    key: Option<String>,
    key_confidence: Option<f64>,
    key_pitch_class: Option<u8>,
    key_mode: Option<String>,
    time_signature: Option<String>,
    duration: Option<f64>,
    sections: Vec<WorkerSection>,
    bpm_alternatives: Vec<f64>,
    // New timeline & stability fields
    audio_start: Option<f64>,
    musical_start: Option<f64>,
    tempo_mode: Option<String>,
    stability: Option<f64>,
    downbeat_confidence: Option<f64>,
    tempo_map: Option<serde_json::Value>,
    beat_grid_locked: Option<bool>,
    grid: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct WorkerSection {
    label: String,
    kind: String,
    start: f64,
}

pub struct PrepQueue {
    tx: Mutex<Option<Sender<String>>>,
    options: Mutex<HashMap<String, PrepOptions>>,
    cancelled: Mutex<HashSet<String>>,
    running: Mutex<Option<(String, u32)>>,
    pending: Mutex<VecDeque<String>>,
}

impl Default for PrepQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PrepQueue {
    pub fn new() -> Self {
        Self {
            tx: Mutex::new(None),
            options: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
            running: Mutex::new(None),
            pending: Mutex::new(VecDeque::new()),
        }
    }

    pub fn start(&self, ctx: Arc<Ctx>) {
        let (tx, rx) = unbounded::<String>();
        *self.tx.lock() = Some(tx);
        std::thread::Builder::new()
            .name("wpe-prep-queue".into())
            .spawn(move || {
                while let Ok(job_id) = rx.recv() {
                    ctx.queue.pending.lock().retain(|j| j != &job_id);
                    if ctx.queue.cancelled.lock().remove(&job_id) {
                        continue;
                    }
                    run_job(&ctx, &job_id);
                }
            })
            .expect("spawn prep queue");
    }

    pub fn enqueue(&self, ctx: &Ctx, song_id: &str, opts: PrepOptions) -> Result<PreparationJob> {
        let song = ctx.db.get_song(song_id)?.ok_or_else(|| anyhow!("Song not found"))?;
        if matches!(song.status, SongStatus::Queued | SongStatus::Processing) {
            return Err(anyhow!("\"{}\" is already being prepared.", song.title));
        }
        let job = PreparationJob {
            id: new_id(),
            song_id: song.id.clone(),
            song_title: song.title.clone(),
            status: "queued".into(),
            stage: "queued".into(),
            progress: 0.0,
            message: "Waiting in queue".into(),
            error: None,
            model: None,
            device: None,
            processing_seconds: None,
            created_at: now_iso(),
            started_at: None,
            finished_at: None,
        };
        ctx.db.save_job(&job)?;
        ctx.db.set_song_status(&song.id, SongStatus::Queued, None)?;
        self.options.lock().insert(job.id.clone(), opts);
        self.pending.lock().push_back(job.id.clone());
        self.tx.lock().as_ref().ok_or_else(|| anyhow!("queue not running"))?.send(job.id.clone())?;
        let _ = ctx.app.emit("preparation:queued", &job);
        logs::info(cat::PREPARATION, format!("Queued \"{}\"", song.title));
        Ok(job)
    }

    pub fn cancel(&self, ctx: &Ctx, job_id: &str) -> Result<()> {
        let running = self.running.lock().clone();
        if let Some((id, pid)) = running {
            if id == job_id {
                self.cancelled.lock().insert(job_id.to_string());
                let _ = Command::new("/bin/kill").arg("-TERM").arg(pid.to_string()).status();
                return Ok(());
            }
        }
        self.cancelled.lock().insert(job_id.to_string());
        if let Some(mut j) = ctx.db.list_jobs(500)?.into_iter().find(|j| j.id == job_id) {
            j.status = "cancelled".into();
            j.finished_at = Some(now_iso());
            ctx.db.save_job(&j)?;
            restore_status(ctx, &j.song_id);
            let _ = ctx.app.emit("preparation:failed", &j);
        }
        Ok(())
    }

    pub fn queue_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.running.lock().iter().map(|(id, _)| id.clone()).collect();
        v.extend(self.pending.lock().iter().cloned());
        v
    }
}

fn restore_status(ctx: &Ctx, song_id: &str) {
    if let Ok(Some(s)) = ctx.db.get_song(song_id) {
        let st = if s.rendered.backing.is_some() { SongStatus::Ready } else { SongStatus::Imported };
        let _ = ctx.db.set_song_status(song_id, st, None);
    }
}

pub fn friendly_error(code: &str, fallback: &str) -> String {
    match code {
        "model_missing" => "The AI separation model is not installed. Open Diagnostics → AI Models to download it.".into(),
        "env_missing" => "The audio preparation environment is not installed. Open Diagnostics and click Install.".into(),
        "ffmpeg_missing" => "FFmpeg was not found. Open Diagnostics → Repair setup.".into(),
        "decode_failed" => "This audio file could not be decoded.".into(),
        "separation_failed" => "The separation engine stopped unexpectedly.".into(),
        "out_of_memory" => "The Mac ran out of memory while separating. Close other apps and retry.".into(),
        "cancelled" => "Preparation was cancelled.".into(),
        _ => fallback.to_string(),
    }
}

fn fail(ctx: &Ctx, job: &mut PreparationJob, code: &str, message: &str, details: Option<String>) {
    let friendly = friendly_error(code, message);
    job.status = if code == "cancelled" { "cancelled".into() } else { "failed".into() };
    job.error = Some(match &details {
        Some(d) if !d.is_empty() => format!("{friendly}\n\n--- details ---\n{message}\n{d}"),
        _ => friendly.clone(),
    });
    job.message = friendly.clone();
    job.finished_at = Some(now_iso());
    let _ = ctx.db.save_job(job);
    if code == "cancelled" {
        restore_status(ctx, &job.song_id);
    } else {
        let _ = ctx.db.set_song_status(&job.song_id, SongStatus::Error, Some(&friendly));
    }
    logs::error(cat::PREPARATION, format!("\"{}\" failed: {message}", job.song_title));
    let _ = ctx.app.emit("preparation:failed", &*job);
}

fn run_job(ctx: &Ctx, job_id: &str) {
    let Some(mut job) = ctx.db.list_jobs(500).ok().and_then(|j| j.into_iter().find(|j| j.id == job_id)) else {
        return;
    };
    let opts = ctx.queue.options.lock().remove(job_id).unwrap_or(PrepOptions {
        separate: true,
        analyze: true,
        overwrite_manual: false,
    });
    let started = Instant::now();
    job.status = "running".into();
    job.started_at = Some(now_iso());
    job.stage = "validate".into();
    job.message = "Validating file".into();
    let _ = ctx.db.save_job(&job);
    let _ = ctx.db.set_song_status(&job.song_id, SongStatus::Processing, None);
    let _ = ctx.app.emit("preparation:started", &job);

    let song = match ctx.db.get_song(&job.song_id) {
        Ok(Some(s)) => s,
        _ => return fail(ctx, &mut job, "missing", "The song no longer exists.", None),
    };
    let paths = ctx.paths.read().clone();
    let settings = ctx.settings.read().clone();
    let python = paths.python_bin();
    if !python.exists() {
        return fail(ctx, &mut job, "env_missing", "python env missing", None);
    }
    let Some(ffmpeg) = find_tool("ffmpeg") else {
        return fail(ctx, &mut job, "ffmpeg_missing", "ffmpeg missing", None);
    };
    let model = settings.separation_model.clone();
    if opts.separate && !ctx.resources.model_installed(&paths, &model) {
        return fail(ctx, &mut job, "model_missing", &format!("model {model} not installed"), None);
    }
    job.model = opts.separate.then(|| model.clone());

    let request = serde_json::json!({
        "type": "prepare_song",
        "protocolVersion": 1,
        "songId": song.id,
        "input": crate::library::song_path(&song, &song.source_path),
        "songDir": song.dir,
        "sampleRate": 48000,
        "ffmpeg": ffmpeg,
        "settings": {
            "vocals": song.prep.vocals,
            "drums": song.prep.drums,
            "bass": song.prep.bass,
            "bassDb": song.prep.bass_db,
            "otherDb": song.prep.other_db,
        },
        "separation": {
            "enabled": opts.separate,
            "model": model,
            "engine": settings.separation_engine,
            "installedEngines": ctx.resources.installed_engines(&paths, &model),
            "device": settings.separation_device,
            "modelsDir": paths.demucs_models(),
            "mlxModelsDir": paths.models().join("demucs-mlx"),
            "mdxModelsDir": paths.models().join("mdx"),
            "profile": settings.separation_profile,
        },
        "analysis": {
            "enabled": opts.analyze,
            "timeSignature": song.time_signature,
        },
    });
    let req_path = paths.cache().join(format!("prepare-{}.json", job.id));
    if let Err(e) = std::fs::write(&req_path, request.to_string()) {
        return fail(ctx, &mut job, "io", &format!("cannot write request: {e}"), None);
    }

    let mut child = match Command::new(&python)
        .args(["-m", "wpe_worker", "prepare", "--request"])
        .arg(&req_path)
        .current_dir(&ctx.worker_dir)
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONPATH", ctx.worker_dir.join("src"))
        .env("TORCH_HOME", paths.models().join("torch"))
        .env("HF_HUB_OFFLINE", "1")
        .env("DEMUCS_MLX_NO_DOWNLOAD", "1")
        .env("DEMUCS_MLX_CACHE_DIR", paths.models().join("demucs-mlx"))
        .env("PYTORCH_ENABLE_MPS_FALLBACK", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail(ctx, &mut job, "spawn", &format!("could not start worker: {e}"), None),
    };
    *ctx.queue.running.lock() = Some((job.id.clone(), child.id()));
    let stderr = child.stderr.take();
    let stderr_handle = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut s);
        }
        s
    });

    let mut result: Option<WorkerResult> = None;
    let mut error: Option<(String, String, Option<String>)> = None;
    let mut last_save = Instant::now();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let line = line.trim();
            if !line.starts_with('{') {
                continue;
            }
            match serde_json::from_str::<WorkerEvent>(line) {
                Ok(WorkerEvent::Progress { stage, progress, message }) => {
                    job.stage = stage;
                    job.progress = progress.clamp(0.0, 1.0);
                    job.message = message;
                    let _ = ctx.app.emit("preparation:progress", &job);
                    if last_save.elapsed().as_millis() > 1000 {
                        let _ = ctx.db.save_job(&job);
                        last_save = Instant::now();
                    }
                }
                Ok(WorkerEvent::Log { level, message }) => {
                    if message.starts_with("device=") {
                        job.device = Some(message.trim_start_matches("device=").to_string());
                    }
                    logs::log(if level.is_empty() { "info" } else { &level }, cat::PREPARATION, message);
                }
                Ok(WorkerEvent::Result(r)) => result = Some(*r),
                Ok(WorkerEvent::Error { code, message, details }) => error = Some((code, message, details)),
                Err(e) => logs::warn(cat::PREPARATION, format!("bad worker line ({e}): {line}")),
            }
        }
    }
    let status = child.wait();
    *ctx.queue.running.lock() = None;
    let stderr_text = stderr_handle.join().unwrap_or_default();
    let _ = std::fs::remove_file(&req_path);
    let tail: String = {
        let t = stderr_text.trim();
        let n = t.len();
        let start = (n.saturating_sub(3000)..n).find(|i| t.is_char_boundary(*i)).unwrap_or(n);
        t[start..].to_string()
    };

    if ctx.queue.cancelled.lock().remove(&job.id) {
        return fail(ctx, &mut job, "cancelled", "cancelled by user", None);
    }
    if let Some((code, message, details)) = error {
        return fail(ctx, &mut job, &code, &message, details.or(Some(tail)));
    }
    let Some(result) = result else {
        let code = match status.as_ref().ok().and_then(|s| s.code()) {
            None | Some(137) | Some(-9) => "out_of_memory",
            _ => "separation_failed",
        };
        return fail(ctx, &mut job, code, "worker exited without a result", Some(tail));
    };

    match apply_result(ctx, &song.id, &opts, &result) {
        Ok(updated) => {
            job.status = "completed".into();
            job.stage = "done".into();
            job.progress = 1.0;
            job.message = if result.warnings.is_empty() {
                "Live-ready".into()
            } else {
                result.warnings.join(" · ")
            };
            job.processing_seconds = Some(started.elapsed().as_secs_f64());
            if let Some(sep) = &result.separator {
                job.device = sep.get("device").and_then(|d| d.as_str()).map(String::from).or(job.device.clone());
            }
            job.finished_at = Some(now_iso());
            let _ = ctx.db.save_job(&job);
            logs::info(
                cat::PREPARATION,
                format!("\"{}\" prepared in {:.1}s", updated.title, job.processing_seconds.unwrap_or(0.0)),
            );
            let _ = ctx.app.emit("preparation:completed", &job);
        }
        Err(e) => fail(ctx, &mut job, "apply", &format!("{e:#}"), None),
    }
}

fn apply_result(ctx: &Ctx, song_id: &str, opts: &PrepOptions, r: &WorkerResult) -> Result<Song> {
    let mut song = ctx.db.get_song(song_id)?.ok_or_else(|| anyhow!("song vanished"))?;
    let dir = Path::new(&song.dir).to_path_buf();
    let exists = |p: &Option<String>| p.as_ref().filter(|rel| dir.join(rel).is_file()).cloned();
    if opts.separate {
        song.stems = StemPaths {
            vocals: exists(&r.stems.vocals),
            drums: exists(&r.stems.drums),
            bass: exists(&r.stems.bass),
            other: exists(&r.stems.other),
        };
    }
    if let Some(b) = exists(&r.rendered.backing) {
        song.rendered.backing = Some(b);
    }
    if let Some(b) = exists(&r.rendered.backup) {
        song.rendered.backup = Some(b);
    }
    let mut analysis = song.analysis.clone().unwrap_or_else(|| serde_json::json!({}));
    if let Some(a) = &r.analysis {
        let ow = opts.overwrite_manual;
        let keep_user_grid = song.grid.as_ref().map(|g| g.source == "user").unwrap_or(false) && !ow;
        if a.bpm.is_some() && (ow || song.bpm.is_none()) && !keep_user_grid {
            song.bpm = a.bpm.map(|b| (b * 1000.0).round() / 1000.0);
            song.beat_offset = a.beat_offset.unwrap_or(0.0);
        }
        if a.key.is_some() && (ow || song.key.is_none()) {
            song.key = a.key.clone();
            song.key_pitch_class = a.key_pitch_class;
            song.key_mode = a.key_mode.clone();
        }
        if let Some(ts) = &a.time_signature {
            if ow {
                song.time_signature = ts.clone();
            }
        }
        if let Some(d) = a.duration {
            song.duration = d;
        }
        if !a.sections.is_empty() && (ow || song.sections.is_empty()) {
            song.sections = a
                .sections
                .iter()
                .map(|s| Section { id: new_id(), label: s.label.clone(), kind: s.kind.clone(), start: s.start })
                .collect();
            song.arrangement.clear();
        }
        analysis["bpmConfidence"] = serde_json::json!(a.bpm_confidence);
        analysis["keyConfidence"] = serde_json::json!(a.key_confidence);
        if let Some(pc) = a.key_pitch_class {
            analysis["keyPitchClass"] = serde_json::json!(pc);
        }
        if let Some(m) = &a.key_mode {
            analysis["keyMode"] = serde_json::json!(m);
        }
        analysis["detected"] = serde_json::json!({
            "bpm": a.bpm, "key": a.key, "beatOffset": a.beat_offset, "sectionCount": a.sections.len(),
            "bpmAlternatives": a.bpm_alternatives,
        });
        // Forward new timeline / stability fields into the analysis blob.
        // These are read by the player (build_beat_grid) and the frontend.
        if let Some(v) = a.audio_start { analysis["audioStart"] = serde_json::json!(v); }
        if let Some(v) = a.musical_start { analysis["musicalStart"] = serde_json::json!(v); }
        if let Some(v) = &a.tempo_mode { analysis["tempoMode"] = serde_json::json!(v); }
        if let Some(v) = a.stability { analysis["stability"] = serde_json::json!(v); }
        if let Some(v) = a.downbeat_confidence { analysis["downbeatConfidence"] = serde_json::json!(v); }
        if let Some(v) = &a.tempo_map { analysis["tempoMap"] = v.clone(); }
        // Preserve beatGridLocked if previously set by user; only reset it on overwrite.
        let already_locked = analysis.get("beatGridLocked").and_then(|v| v.as_bool()).unwrap_or(false);
        if !already_locked {
            analysis["beatGridLocked"] = serde_json::json!(a.beat_grid_locked.unwrap_or(false));
        }
        if !keep_user_grid {
            if let Some(raw) = &a.grid {
                if let Ok(parsed) = serde_json::from_value(raw.clone()) {
                    song.grid = crate::audio::tempo_map::sanitize_grid(parsed, song.beats_per_bar(), song.duration.max(a.duration.unwrap_or(song.duration)));
                }
            }
        }
    }
    if let Some(sep) = &r.separator {
        analysis["separator"] = sep.clone();
    }
    if let Some(t) = &r.timings {
        analysis["timings"] = t.clone();
    }
    if song.rendered.backing.is_some() {
        analysis["renderedWith"] = serde_json::to_value(&song.prep)?;
    }
    analysis["warnings"] = serde_json::json!(r.warnings);
    song.analysis = Some(analysis);
    song.sample_rate = 48000;
    song.prepared_at = Some(now_iso());
    song.updated_at = now_iso();
    song.last_error = None;
    song.status = if song.rendered.backing.is_some() && r.warnings.is_empty() {
        SongStatus::Ready
    } else if song.rendered.backing.is_some() || song.rendered.backup.is_some() {
        SongStatus::Warning
    } else {
        SongStatus::Imported
    };
    // Render cue clips now so live playback never waits for TTS.
    let voice = ctx.settings.read().cue_voice.clone();
    let cue_root = ctx.paths.read().cues();
    for s in &song.sections {
        if let Err(e) = crate::cue::ensure_cue(&cue_root, voice.as_deref(), &s.label) {
            logs::warn(cat::PREPARATION, format!("cue \"{}\" failed: {e}", s.label));
        }
    }
    ctx.db.upsert_song(&song)?;
    crate::library::write_song_json(&song)?;
    ctx.player.invalidate_song(&song);
    Ok(song)
}
