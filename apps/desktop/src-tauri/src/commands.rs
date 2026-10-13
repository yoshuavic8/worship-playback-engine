//! Strongly typed IPC surface between React and Rust. Every payload is a typed struct that
//! serde validates on the way in; nothing arbitrary is accepted.

use crate::audio::buffer::AudioBuffer;
use crate::audio::click::{render_click_track, BeatGrid, ClickSound, ClickVoices};
use crate::audio::engine::{list_devices, DeviceInfo, RunningInfo};
use crate::audio::mixer::{MixerCmd, TestSignal, TrackRole};
use crate::audio::routing::{self, RoutingIssue};
use crate::core::{self, Ctx};
use crate::logs::{self, cat, LogEntry};
use crate::models::*;
use crate::player::{LiveInfo, LoadedInfo, PlayContext};
use crate::prep::PrepOptions;
use crate::presets::snap_bass_db;
use crate::settings::AppSettings;
use crate::{ai, library, preflight, resources, system};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

type R<T> = Result<T, String>;
type S<'a> = State<'a, Arc<Ctx>>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(e)?.map_err(|err| format!("{err:#}"))
}

// ---------------- bootstrap & settings ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: AppSettings,
    pub audio: AudioSettings,
    pub presets: Vec<Preset>,
    pub version: String,
    pub data_dir: String,
    pub click_sounds: Vec<(String, String)>,
}

#[tauri::command]
pub fn get_bootstrap(ctx: S) -> R<Bootstrap> {
    Ok(Bootstrap {
        settings: ctx.settings.read().clone(),
        audio: ctx.audio.read().clone(),
        presets: ctx.db.list_presets().map_err(e)?,
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: ctx.paths.read().root.to_string_lossy().into(),
        click_sounds: ClickSound::all().iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
    })
}

#[tauri::command]
pub fn save_settings(ctx: S, mut settings: AppSettings) -> R<AppSettings> {
    settings.ui_scale = settings.ui_scale.clamp(0.8, 1.5);
    settings.default_count_in_bars = settings.default_count_in_bars.min(4);
    if !["htdemucs", "htdemucs_ft"].contains(&settings.separation_model.as_str()) {
        settings.separation_model = "htdemucs".into();
    }
    if !["auto", "demucs-mlx", "demucs"].contains(&settings.separation_engine.as_str()) {
        settings.separation_engine = "auto".into();
    }
    if !["auto", "mps", "cpu"].contains(&settings.separation_device.as_str()) {
        settings.separation_device = "auto".into();
    }
    if !["worship", "fast"].contains(&settings.separation_profile.as_str()) {
        settings.separation_profile = "worship".into();
    }
    let old = ctx.settings.read().clone();
    if settings.library_path != old.library_path {
        if let Some(p) = &settings.library_path {
            std::fs::create_dir_all(p).map_err(|err| format!("Cannot use that folder: {err}"))?;
        }
        let root = ctx.paths.read().root.clone();
        *ctx.paths.write() = crate::paths::AppPaths::new(root, settings.library_path.clone().map(PathBuf::from));
        logs::info(cat::SYSTEM, format!("Library location changed to {:?}", settings.library_path));
    }
    ctx.db.set_setting("app", &settings).map_err(e)?;
    *ctx.settings.write() = settings.clone();
    Ok(settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub running: Option<RunningInfo>,
    pub settings: AudioSettings,
    pub issues: Vec<RoutingIssue>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn audio_status(ctx: S) -> AudioStatus {
    let running = ctx.engine.running();
    let settings = ctx.audio.read().clone();
    let channels = running.as_ref().map(|r| r.channels).unwrap_or(2);
    AudioStatus {
        issues: routing::validate(&settings.routing, channels),
        running,
        settings,
        error: ctx.engine.last_error(),
    }
}

#[tauri::command]
pub async fn list_audio_devices() -> R<Vec<DeviceInfo>> {
    blocking(|| Ok(list_devices())).await
}

#[tauri::command]
pub async fn save_audio_settings(ctx: S<'_>, audio: AudioSettings) -> R<AudioStatus> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let mut audio = audio;
        if ![44100, 48000, 88200, 96000].contains(&audio.sample_rate) {
            audio.sample_rate = 48000;
        }
        if ![64, 128, 256, 512, 1024].contains(&audio.buffer_size) {
            audio.buffer_size = 256;
        }
        audio.click_volume_db = audio.click_volume_db.clamp(-60.0, 6.0);
        audio.cue_volume_db = audio.cue_volume_db.clamp(-60.0, 6.0);
        audio.monitor_backing_db = audio.monitor_backing_db.clamp(-60.0, 6.0);
        let old = ctx.audio.read().clone();
        let reopen = old.device_name != audio.device_name
            || old.sample_rate != audio.sample_rate
            || old.buffer_size != audio.buffer_size
            || ctx.engine.running().is_none();
        let channels = ctx.engine.running().map(|r| r.channels).unwrap_or(2);
        if !reopen && !routing::is_live_safe(&audio.routing, channels) {
            let msgs: Vec<_> = routing::validate(&audio.routing, channels)
                .into_iter()
                .filter(|i| i.level == "error")
                .map(|i| i.message)
                .collect();
            anyhow::bail!("{}", msgs.join(" "));
        }
        *ctx.audio.write() = audio.clone();
        ctx.db.set_setting("audio", &audio)?;
        ctx.db.save_routing_profile(&audio.routing)?;
        if reopen {
            let reload = ctx.player.loaded().map(|l| (l.song_id, l.context, l.live_index));
            ctx.player.unload(&ctx);
            if old.sample_rate != audio.sample_rate {
                ctx.player.clear_cache();
            }
            core::open_audio(&ctx)?;
            if let Some((id, context, live_index)) = reload {
                if let Some(i) = live_index {
                    let _ = ctx.player.live_goto(&ctx, i);
                } else {
                    let _ = ctx.player.load(&ctx, &id, context, None, None);
                }
            }
        } else {
            ctx.engine.set_bus(routing::to_bus_map(&audio.routing));
            ctx.engine.set_levels(core::levels_for(&audio));
            ctx.engine.set_click_sound(ClickSound::parse(&audio.click_sound));
        }
        logs::info(cat::ROUTING, format!("Routing: {} (FOH {}, Monitor {})", audio.routing.name,
            preflight::describe(&audio.routing.foh), preflight::describe(&audio.routing.monitor)));
        let _ = tauri::Emitter::emit(&ctx.app, "audio:deviceChanged", ());
        let running = ctx.engine.running();
        let channels = running.as_ref().map(|r| r.channels).unwrap_or(2);
        let settings = ctx.audio.read().clone();
        Ok(AudioStatus { issues: routing::validate(&settings.routing, channels), running, settings, error: ctx.engine.last_error() })
    })
    .await
}

#[tauri::command]
pub async fn test_signal(ctx: S<'_>, kind: String) -> R<()> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let sr = ctx.engine.sample_rate();
        let tone = |freq: f32, secs: f32| {
            let n = (sr as f32 * secs) as usize;
            let mono: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (t * 20.0).min(1.0) * ((secs - t) * 20.0).clamp(0.0, 1.0);
                    (t * freq * std::f32::consts::TAU).sin() * 0.125 * env
                })
                .collect();
            AudioBuffer::from_mono(&mono, sr)
        };
        let (buf, to_foh, to_mon) = match kind.as_str() {
            "stop" => {
                ctx.engine.send(MixerCmd::StopTest);
                return Ok(());
            }
            "foh" => (tone(440.0, 2.0), true, false),
            "monitor" => (tone(880.0, 2.0), false, true),
            "click" => {
                let sound = ClickSound::parse(&ctx.audio.read().click_sound);
                let g = BeatGrid::new(100.0, 0.0, 4, sr).unwrap();
                let lv = ctx.engine.levels();
                let mut b = render_click_track(&g, (sr as f64 * 4.8) as u64, &ClickVoices::new(sound, sr), sr);
                b.data.iter_mut().for_each(|s| *s *= lv.click);
                (b, false, true)
            }
            "cue" => {
                let voice = ctx.settings.read().cue_voice.clone();
                let p = crate::cue::ensure_test_cue(&ctx.paths.read().cues(), voice.as_deref())?;
                let mut b = crate::audio::decode::load_at_rate(&p, sr)?;
                let lv = ctx.engine.levels();
                b.data.iter_mut().for_each(|s| *s *= lv.cue);
                (b, false, true)
            }
            _ => anyhow::bail!("unknown test"),
        };
        if !ctx.engine.send(MixerCmd::Test(Box::new(TestSignal { buf: Arc::new(buf), to_foh, to_mon }))) {
            anyhow::bail!("Audio engine is not running");
        }
        logs::info(cat::AUDIO, format!("Test signal: {kind}"));
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_voices() -> R<Vec<crate::cue::Voice>> {
    blocking(|| Ok(crate::cue::list_voices())).await
}

#[tauri::command]
pub async fn preview_cue(ctx: S<'_>, text: String, voice: Option<String>) -> R<()> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let sr = ctx.engine.sample_rate();
        let p = crate::cue::ensure_cue(&ctx.paths.read().cues(), voice.as_deref(), &text)?;
        let b = crate::audio::decode::load_at_rate(&p, sr)?;
        ctx.engine.send(MixerCmd::Test(Box::new(TestSignal { buf: Arc::new(b), to_foh: false, to_mon: true })));
        Ok(())
    })
    .await
}

// ---------------- songs ----------------

#[tauri::command]
pub fn list_songs(ctx: S) -> R<Vec<Song>> {
    ctx.db.list_songs().map_err(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: Vec<Song>,
    pub errors: Vec<ImportError>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub path: String,
    pub error: String,
}

#[tauri::command]
pub async fn import_songs(ctx: S<'_>, paths: Vec<String>) -> R<ImportResult> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let songs_dir = ctx.paths.read().songs();
        let mut defaults = PrepSettings::default();
        defaults.preset_id = Some("FULL_BAND".into());
        let mut imported = vec![];
        let mut errors = vec![];
        for p in paths {
            match library::import_file(&ctx.db, &songs_dir, Path::new(&p), &defaults) {
                Ok(s) => imported.push(s),
                Err(err) => errors.push(ImportError { path: p, error: format!("{err:#}") }),
            }
        }
        let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
        Ok(ImportResult { imported, errors })
    })
    .await
}

/// Validate & normalise user edits before persisting (the rules apply to UI and AI alike).
pub fn sanitize_song(mut s: Song) -> Song {
    s.title = s.title.trim().to_string();
    if s.title.is_empty() {
        s.title = "Untitled".into();
    }
    s.artist = s.artist.trim().to_string();
    s.bpm = s.bpm.filter(|b| (20.0..=300.0).contains(b)).map(|b| (b * 1000.0).round() / 1000.0);
    let (n, d) = parse_time_signature(&s.time_signature);
    s.time_signature = format!("{n}/{d}");
    s.key = s.key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty() && k.len() <= 6);
    s.beat_offset = s.beat_offset.max(0.0);
    if s.duration > 0.0 {
        s.beat_offset = s.beat_offset.min(s.duration);
    }
    if let Some(g) = s.grid.take() {
        if let Some(mut g) = crate::audio::tempo_map::sanitize_grid(g, s.beats_per_bar(), s.duration) {
            g.beats_per_bar = s.beats_per_bar();
            if let Some(p) = g.points.first() {
                s.bpm = Some(p.bpm);
            }
            s.beat_offset = g.beat1_sec;
            s.grid = Some(g);
        }
    }
    s.count_in_bars = s.count_in_bars.min(4);
    s.cue_lead_beats = s.cue_lead_beats.clamp(1, 16);
    s.prep.bass_db = snap_bass_db(s.prep.bass_db);
    s.prep.other_db = s.prep.other_db.clamp(-24.0, 6.0);
    if s.prep.vocals == StemMode::Optional {
        s.prep.vocals = StemMode::Remove;
    }
    if s.prep.bass == StemMode::Optional {
        s.prep.bass = StemMode::Keep;
    }
    s.sections.retain(|x| x.start >= 0.0 && x.start < s.duration.max(1.0));
    for x in s.sections.iter_mut() {
        x.label = x.label.trim().to_string();
        if x.label.is_empty() {
            x.label = "Section".into();
        }
        if x.id.is_empty() {
            x.id = new_id();
        }
    }
    s.sections.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(first) = s.sections.first_mut() {
        first.start = 0.0;
    }
    let ids: Vec<String> = s.sections.iter().map(|x| x.id.clone()).collect();
    s.arrangement.retain(|a| ids.contains(a));
    s.updated_at = now_iso();
    s
}

fn warning_text(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or("").to_ascii_lowercase()
}

/// Soft notes (uncertain BPM) are things the user can confirm. Hard notes (silent
/// backing, wrong sample rate) stay until the audio itself is fixed.
fn is_advisory(text: &str) -> bool {
    text.contains("bpm") || text.contains("tempo") || text.contains("downbeat") || text.contains("uncertain")
}

fn warning_count(analysis: &serde_json::Value) -> usize {
    analysis.get("warnings").and_then(|w| w.as_array()).map(|a| a.len()).unwrap_or(0)
}

fn promote_if_clear(song: &mut Song) {
    let left = song.analysis.as_ref().map(warning_count).unwrap_or(0);
    if left == 0 && song.rendered.backing.is_some() && matches!(song.status, SongStatus::Warning) {
        song.status = SongStatus::Ready;
    }
}

#[tauri::command]
pub fn save_song(ctx: S, song: Song) -> R<Song> {
    let existing = ctx.db.get_song(&song.id).map_err(e)?.ok_or("Song not found")?;
    let tempo_edited = existing.bpm != song.bpm || (existing.beat_offset - song.beat_offset).abs() > 5e-4;
    let key_edited = existing.key != song.key;
    let mut s = sanitize_song(song);
    // Paths, status and analysis are owned by the backend.
    s.dir = existing.dir.clone();
    s.source_path = existing.source_path.clone();
    s.stems = existing.stems.clone();
    s.rendered = existing.rendered.clone();
    s.status = existing.status;
    s.analysis = existing.analysis.clone();
    s.created_at = existing.created_at.clone();
    s.prepared_at = existing.prepared_at.clone();
    s.duration = existing.duration;
    if matches!(s.status, SongStatus::Warning) && (tempo_edited || key_edited) {
        if let Some(analysis) = s.analysis.as_mut() {
            if let Some(arr) = analysis.get_mut("warnings").and_then(|w| w.as_array_mut()) {
                arr.retain(|w| {
                    let l = warning_text(w);
                    let about_tempo = l.contains("bpm") || l.contains("tempo") || l.contains("downbeat") || l.contains("uncertain");
                    let about_key = l.contains("key");
                    !((tempo_edited && about_tempo) || (key_edited && about_key))
                });
            }
        }
        promote_if_clear(&mut s);
    }
    ctx.db.upsert_song(&s).map_err(e)?;
    library::write_song_json(&s).map_err(e)?;
    // Pre-render cue clips for any new labels in the background.
    let labels: Vec<String> = s.sections.iter().map(|x| x.label.clone()).collect();
    let cues = ctx.paths.read().cues();
    let voice = ctx.settings.read().cue_voice.clone();
    std::thread::spawn(move || {
        for l in labels {
            let _ = crate::cue::ensure_cue(&cues, voice.as_deref(), &l);
        }
    });
    let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
    Ok(s)
}

/// The user listened and the tempo / key are fine. Clears the "needs review" note.
#[tauri::command]
pub fn accept_song_review(ctx: S, id: String) -> R<Song> {
    let mut song = ctx.db.get_song(&id).map_err(e)?.ok_or("Song not found")?;
    if let Some(analysis) = song.analysis.as_mut() {
        if let Some(arr) = analysis.get_mut("warnings").and_then(|w| w.as_array_mut()) {
            arr.retain(|w| !is_advisory(&warning_text(w)));
        }
    }
    promote_if_clear(&mut song);
    song.updated_at = now_iso();
    ctx.db.upsert_song(&song).map_err(e)?;
    library::write_song_json(&song).map_err(e)?;
    let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
    Ok(song)
}

#[tauri::command]
pub fn delete_song(ctx: S, id: String) -> R<()> {
    let song = ctx.db.get_song(&id).map_err(e)?.ok_or("Song not found")?;
    if ctx.player.loaded().map(|l| l.song_id == id).unwrap_or(false) {
        ctx.player.unload(&ctx);
    }
    ctx.db.delete_song(&id).map_err(e)?;
    let songs_dir = ctx.paths.read().songs();
    library::delete_song_files(&song, &songs_dir).map_err(e)?;
    ctx.player.invalidate_song(&song);
    logs::info(cat::SYSTEM, format!("Deleted \"{}\"", song.title));
    let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn duplicate_song(ctx: S<'_>, id: String) -> R<Song> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let song = ctx.db.get_song(&id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        let songs_dir = ctx.paths.read().songs();
        let s = library::duplicate_song(&ctx.db, &songs_dir, &song)?;
        let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
        Ok(s)
    })
    .await
}

#[tauri::command]
pub async fn get_waveform(ctx: S<'_>, id: String) -> R<serde_json::Value> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let song = ctx.db.get_song(&id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        let p = Path::new(&song.dir).join("analysis/waveform.json");
        if p.exists() {
            return Ok(serde_json::from_slice(&std::fs::read(&p)?)?);
        }
        let (buf, _) = crate::audio::decode::decode_file(&library::song_path(&song, &song.source_path))?;
        let wf = crate::audio::decode::waveform_peaks(&buf, 50);
        std::fs::write(&p, serde_json::to_vec(&wf)?)?;
        Ok(wf)
    })
    .await
}

#[tauri::command]
pub async fn rerender_backing(ctx: S<'_>, id: String) -> R<Song> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let mut song = ctx.db.get_song(&id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        library::render_backing_from_stems(&song)?;
        song.rendered.backing = Some("rendered/backing.wav".into());
        song.rendered.backup = Some("rendered/backup.wav".into());
        let mut a = song.analysis.clone().unwrap_or_else(|| serde_json::json!({}));
        a["renderedWith"] = serde_json::to_value(&song.prep)?;
        song.analysis = Some(a);
        song.status = SongStatus::Ready;
        song.updated_at = now_iso();
        ctx.db.upsert_song(&song)?;
        library::write_song_json(&song)?;
        ctx.player.invalidate_song(&song);
        logs::info(cat::PREPARATION, format!("Re-rendered backing for \"{}\"", song.title));
        let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
        Ok(song)
    })
    .await
}

// ---------------- beat grid ----------------

/// Update the first downbeat position for a song ("Set Downbeat" action).
#[tauri::command]
pub async fn set_downbeat(ctx: S<'_>, id: String, offset_secs: f64) -> R<Song> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let mut song = ctx.db.get_song(&id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        song.beat_offset = offset_secs.max(0.0);
        if song.duration > 0.0 {
            song.beat_offset = song.beat_offset.min(song.duration);
        }
        if let Some(g) = song.grid.as_mut() {
            g.beat1_sec = song.beat_offset;
        }
        song.updated_at = now_iso();
        let analysis_path = std::path::Path::new(&song.dir).join("analysis").join("analysis.json");
        if analysis_path.is_file() {
            let raw = std::fs::read_to_string(&analysis_path).unwrap_or_else(|_| "{}".into());
            let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
            json["beatOffset"] = serde_json::json!(offset_secs);
            let _ = std::fs::write(&analysis_path, json.to_string());
        }
        ctx.db.upsert_song(&song)?;
        library::write_song_json(&song)?;
        ctx.player.invalidate_song(&song);
        ctx.player.apply_grid(&ctx, &song);
        Ok(song)
    })
    .await
}

/// Replace the tempo map and rebuild the click, cue, and count-in from it.
#[tauri::command]
pub async fn set_grid(ctx: S<'_>, id: String, grid: crate::models::Grid) -> R<Song> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let mut song = ctx.db.get_song(&id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        // The client's map owns the meter. The song row may still have the previous signature.
        let bpb = grid.beats_per_bar.max(1);
        song.time_signature = meter_for_beats(&song.time_signature, bpb);
        let Some(g) = crate::audio::tempo_map::sanitize_grid(grid, bpb, song.duration) else {
            return Err(anyhow::anyhow!("Tempo map needs at least one point"));
        };
        if let Some(p) = g.points.first() {
            song.bpm = Some(p.bpm);
        }
        song.beat_offset = g.beat1_sec;
        song.grid = Some(g);
        song.updated_at = now_iso();
        ctx.db.upsert_song(&song)?;
        library::write_song_json(&song)?;
        ctx.player.invalidate_song(&song);
        ctx.player.apply_grid(&ctx, &song);
        Ok(song)
    })
    .await
}

// ---------------- preparation ----------------

#[tauri::command]
pub fn prepare_songs(ctx: S, ids: Vec<String>, options: PrepOptions) -> R<Vec<PreparationJob>> {
    let mut jobs = vec![];
    let mut errors = vec![];
    for id in ids {
        match ctx.queue.enqueue(&ctx, &id, options.clone()) {
            Ok(j) => jobs.push(j),
            Err(err) => errors.push(err.to_string()),
        }
    }
    let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
    if jobs.is_empty() && !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(jobs)
}

#[tauri::command]
pub fn cancel_job(ctx: S, id: String) -> R<()> {
    ctx.queue.cancel(&ctx, &id).map_err(e)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobList {
    pub jobs: Vec<PreparationJob>,
    pub queue: Vec<String>,
}

#[tauri::command]
pub fn list_jobs(ctx: S) -> R<JobList> {
    Ok(JobList { jobs: ctx.db.list_jobs(100).map_err(e)?, queue: ctx.queue.queue_ids() })
}

// ---------------- presets ----------------

#[tauri::command]
pub fn list_presets(ctx: S) -> R<Vec<Preset>> {
    ctx.db.list_presets().map_err(e)
}

#[tauri::command]
pub fn save_preset(ctx: S, mut preset: Preset) -> R<Preset> {
    if preset.id.is_empty() || preset.builtin {
        preset.id = format!("custom-{}", &new_id()[..8]);
        preset.builtin = false;
    }
    preset.name = preset.name.trim().to_string();
    if preset.name.is_empty() {
        return Err("Preset needs a name".into());
    }
    preset.settings.bass_db = snap_bass_db(preset.settings.bass_db);
    preset.settings.preset_id = Some(preset.id.clone());
    ctx.db.save_preset(&preset).map_err(e)?;
    Ok(preset)
}

#[tauri::command]
pub fn delete_preset(ctx: S, id: String) -> R<()> {
    ctx.db.delete_preset(&id).map_err(e)
}

// ---------------- setlists ----------------

#[tauri::command]
pub fn list_setlists(ctx: S) -> R<Vec<Setlist>> {
    ctx.db.list_setlists().map_err(e)
}

#[tauri::command]
pub fn get_setlist(ctx: S, id: String) -> R<Setlist> {
    ctx.db.get_setlist(&id).map_err(e)?.ok_or_else(|| "Setlist not found".into())
}

#[tauri::command]
pub fn save_setlist(ctx: S, mut setlist: Setlist) -> R<Setlist> {
    if setlist.id.is_empty() {
        setlist.id = new_id();
        setlist.created_at = now_iso();
    }
    setlist.name = setlist.name.trim().to_string();
    if setlist.name.is_empty() {
        setlist.name = "Untitled Setlist".into();
    }
    for (i, it) in setlist.items.iter_mut().enumerate() {
        if it.id.is_empty() {
            it.id = new_id();
        }
        it.position = i as u32;
    }
    setlist.updated_at = now_iso();
    ctx.db.save_setlist(&setlist).map_err(e)?;
    let _ = tauri::Emitter::emit(&ctx.app, "setlists:changed", ());
    ctx.db.get_setlist(&setlist.id).map_err(e)?.ok_or_else(|| "save failed".into())
}

#[tauri::command]
pub fn delete_setlist(ctx: S, id: String) -> R<()> {
    ctx.db.delete_setlist(&id).map_err(e)?;
    let _ = tauri::Emitter::emit(&ctx.app, "setlists:changed", ());
    Ok(())
}

#[tauri::command]
pub fn duplicate_setlist(ctx: S, id: String) -> R<Setlist> {
    let mut s = ctx.db.get_setlist(&id).map_err(e)?.ok_or("Setlist not found")?;
    s.id = String::new();
    s.name = format!("{} (Copy)", s.name);
    for it in s.items.iter_mut() {
        it.id = String::new();
    }
    save_setlist(ctx, s)
}

#[tauri::command]
pub fn export_setlist(ctx: S, id: String, path: String) -> R<()> {
    let s = ctx.db.get_setlist(&id).map_err(e)?.ok_or("Setlist not found")?;
    let songs: Vec<_> = s
        .items
        .iter()
        .filter_map(|i| ctx.db.get_song(&i.song_id).ok().flatten())
        .map(|x| serde_json::json!({ "id": x.id, "title": x.title, "artist": x.artist, "key": x.key, "bpm": x.bpm }))
        .collect();
    let j = serde_json::json!({ "format": "wpe-setlist", "version": 1, "exportedAt": now_iso(), "setlist": s, "songs": songs });
    std::fs::write(path, serde_json::to_vec_pretty(&j).map_err(e)?).map_err(e)
}

#[tauri::command]
pub fn import_setlist(ctx: S, path: String) -> R<Setlist> {
    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).map_err(e)?).map_err(e)?;
    if j["format"] != "wpe-setlist" {
        return Err("This is not a Worship Playback Engine setlist file".into());
    }
    let mut s: Setlist = serde_json::from_value(j["setlist"].clone()).map_err(e)?;
    let library = ctx.db.list_songs().map_err(e)?;
    let exported: Vec<serde_json::Value> = j["songs"].as_array().cloned().unwrap_or_default();
    let mut missing = 0;
    s.items.retain_mut(|it| {
        if library.iter().any(|x| x.id == it.song_id) {
            it.id = String::new();
            return true;
        }
        let title = exported.iter().find(|x| x["id"] == it.song_id.as_str()).and_then(|x| x["title"].as_str());
        if let Some(found) = title.and_then(|t| library.iter().find(|x| x.title.eq_ignore_ascii_case(t))) {
            it.song_id = found.id.clone();
            it.id = String::new();
            it.start_section_id = None;
            it.arrangement = None;
            true
        } else {
            missing += 1;
            false
        }
    });
    s.id = String::new();
    if missing > 0 {
        s.notes = format!("{}\n({missing} song(s) were not found in this library)", s.notes).trim().to_string();
    }
    save_setlist(ctx, s)
}

// ---------------- playback ----------------

#[tauri::command]
pub async fn player_load(ctx: S<'_>, song_id: String, context: PlayContext) -> R<LoadedInfo> {
    let ctx = ctx.inner().clone();
    blocking(move || ctx.player.load(&ctx, &song_id, context, None, None)).await
}

#[tauri::command]
pub fn player_play(ctx: S, count_in: bool) -> R<()> {
    ctx.player.play(&ctx, count_in).map_err(e)
}
#[tauri::command]
pub fn player_pause(ctx: S) -> R<()> {
    ctx.player.pause(&ctx).map_err(e)
}
#[tauri::command]
pub fn player_stop(ctx: S) -> R<()> {
    ctx.player.stop(&ctx).map_err(e)
}
#[tauri::command]
pub fn player_panic(ctx: S) {
    ctx.player.panic(&ctx)
}
#[tauri::command]
pub fn player_fade(ctx: S, seconds: f32) {
    ctx.player.fade_out(&ctx, seconds.clamp(0.5, 15.0))
}
#[tauri::command]
pub fn player_seek(ctx: S, seconds: f64) {
    ctx.player.seek(&ctx, seconds)
}
#[tauri::command]
pub fn player_jump(ctx: S, index: usize, immediate: bool) {
    ctx.player.jump(&ctx, index, immediate)
}
#[tauri::command]
pub fn player_cancel_jump(ctx: S) {
    ctx.player.cancel_jump(&ctx)
}
#[tauri::command]
pub fn player_loop(ctx: S, on: bool) {
    ctx.player.set_loop(&ctx, on)
}
#[tauri::command]
pub fn player_set_track(ctx: S, role: TrackRole, gain_db: f32, muted: bool) {
    ctx.player.set_track(&ctx, role, gain_db.clamp(-60.0, 6.0), muted)
}
#[tauri::command]
pub fn player_set_click(ctx: S, enabled: bool) {
    ctx.player.set_click(&ctx, enabled)
}
#[tauri::command]
pub fn player_set_cue(ctx: S, enabled: bool) {
    ctx.player.set_cue(&ctx, enabled)
}
#[tauri::command]
pub fn player_loaded(ctx: S) -> Option<LoadedInfo> {
    ctx.player.loaded()
}

// ---------------- live ----------------

#[tauri::command]
pub async fn live_open(ctx: S<'_>, setlist_id: String) -> R<LiveInfo> {
    let ctx = ctx.inner().clone();
    blocking(move || ctx.player.live_open(&ctx, &setlist_id)).await
}
#[tauri::command]
pub async fn live_goto(ctx: S<'_>, index: usize) -> R<LoadedInfo> {
    let ctx = ctx.inner().clone();
    blocking(move || ctx.player.live_goto(&ctx, index)).await
}
#[tauri::command]
pub async fn live_step(ctx: S<'_>, delta: i32) -> R<LoadedInfo> {
    let ctx = ctx.inner().clone();
    blocking(move || ctx.player.live_step(&ctx, delta)).await
}
#[tauri::command]
pub fn live_close(ctx: S) {
    ctx.player.live_close(&ctx)
}
#[tauri::command]
pub fn live_info(ctx: S) -> Option<LiveInfo> {
    ctx.player.live_info()
}

#[tauri::command]
pub fn run_preflight(ctx: S, setlist_id: String) -> R<preflight::PreflightReport> {
    let s = ctx.db.get_setlist(&setlist_id).map_err(e)?.ok_or("Setlist not found")?;
    let songs: Vec<Option<Song>> = s.items.iter().map(|i| ctx.db.get_song(&i.song_id).ok().flatten()).collect();
    let running = ctx.engine.running();
    let dev = preflight::DeviceState {
        connected: running.is_some(),
        name: running.as_ref().map(|r| r.device_name.clone()).unwrap_or_default(),
        channels: running.as_ref().map(|r| r.channels).unwrap_or(0),
        sample_rate: running.as_ref().map(|r| r.sample_rate).unwrap_or(0),
    };
    Ok(preflight::run(&s, &songs, &dev, &ctx.audio.read().routing))
}

// ---------------- diagnostics & resources ----------------

#[tauri::command]
pub async fn system_readiness(ctx: S<'_>) -> R<system::Readiness> {
    let ctx = ctx.inner().clone();
    blocking(move || Ok(system::readiness(&ctx))).await
}

#[tauri::command]
pub fn list_resources(ctx: S) -> Vec<resources::ResourceStatus> {
    ctx.resources.status(&ctx.paths.read())
}

#[tauri::command]
pub fn download_resource(ctx: S, id: String) -> R<()> {
    ctx.resources.download(ctx.inner().clone(), &id).map_err(e)
}

#[tauri::command]
pub fn cancel_download(ctx: S, id: String) {
    ctx.resources.cancel(&id)
}

#[tauri::command]
pub fn remove_resource(ctx: S, id: String) -> R<()> {
    ctx.resources.remove(&ctx, &id).map_err(e)
}

#[tauri::command]
pub fn install_python_env(ctx: S) -> R<()> {
    resources::install_python_env(ctx.inner().clone()).map_err(e)
}

#[tauri::command]
pub fn get_logs(limit: Option<usize>, category: Option<String>) -> Vec<LogEntry> {
    logs::recent(limit.unwrap_or(500), category.as_deref())
}

// ---------------- AI ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub provider: String,
    pub model: Option<String>,
    pub available: bool,
    pub models: Vec<String>,
}

#[tauri::command]
pub async fn ai_status(ctx: S<'_>) -> R<AiStatus> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let settings = ctx.settings.read().clone();
        let models = if settings.ollama_enabled {
            system::ollama_models(&settings.ollama_url).unwrap_or_default()
        } else {
            vec![]
        };
        let p = ai::provider(&settings);
        let model = (p.name() == "ollama").then(|| {
            settings
                .ollama_model
                .clone()
                .filter(|m| models.contains(m))
                .or_else(|| models.iter().find(|m| m.starts_with("qwen2.5")).cloned())
                .or_else(|| models.iter().find(|m| m.contains("llama3")).cloned())
                .unwrap_or_else(|| models.first().cloned().unwrap_or_default())
        });
        Ok(AiStatus { provider: p.name().into(), available: p.name() != "none", model, models })
    })
    .await
}

fn ai_model_name(settings: &AppSettings) -> Option<String> {
    settings.ollama_model.clone()
}

#[tauri::command]
pub async fn ai_song_command(ctx: S<'_>, song_id: String, text: String) -> R<ai::AiProposal> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let text = text.trim().chars().take(800).collect::<String>();
        if text.is_empty() {
            anyhow::bail!("Type what you want to change.");
        }
        let song = ctx.db.get_song(&song_id)?.ok_or_else(|| anyhow::anyhow!("Song not found"))?;
        let settings = ctx.settings.read().clone();
        let p = ai::provider(&settings);
        let user = format!("{}\nMD REQUEST: {text}", ai::song_context(&song));
        logs::info(cat::AI, format!("[{}] song command: {text}", p.name()));
        let raw = p.complete_json(ai::SONG_SYSTEM_PROMPT, &user, &ai::intent_schema())?;
        let intent = ai::parse_intent(&raw).map_err(|err| {
            logs::warn(cat::AI, format!("rejected AI output: {raw}"));
            err
        })?;
        let mut proposal = ai::rules_for_song(&song, &intent);
        proposal.provider = p.name().into();
        proposal.model = ai_model_name(&settings);
        logs::info(cat::AI, format!("proposal: {} change(s), {} warning(s)", proposal.changes.len(), proposal.warnings.len()));
        Ok(proposal)
    })
    .await
}

#[tauri::command]
pub async fn ai_setlist_command(ctx: S<'_>, text: String) -> R<ai::AiProposal> {
    let ctx = ctx.inner().clone();
    blocking(move || {
        let text = text.trim().chars().take(800).collect::<String>();
        let library = ctx.db.list_songs()?;
        if library.is_empty() {
            anyhow::bail!("Your library is empty — import songs first.");
        }
        let settings = ctx.settings.read().clone();
        let p = ai::provider(&settings);
        let user = format!("{}\nREQUEST: {text}", ai::library_context(&library));
        logs::info(cat::AI, format!("[{}] setlist command: {text}", p.name()));
        let raw = p.complete_json(ai::SETLIST_SYSTEM_PROMPT, &user, &ai::intent_schema())?;
        let intent = ai::parse_intent(&raw)?;
        let mut proposal = ai::rules_for_setlist(&library, &intent);
        proposal.provider = p.name().into();
        proposal.model = ai_model_name(&settings);
        Ok(proposal)
    })
    .await
}

// ---------------- library backup ----------------

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryExport {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub songs: Vec<Song>,
    pub setlists: Vec<Setlist>,
    pub presets: Vec<Preset>,
    pub routing_profiles: Vec<RoutingProfile>,
}

#[tauri::command]
pub fn export_library(ctx: S, path: String) -> R<usize> {
    let x = LibraryExport {
        format: "wpe-library".into(),
        version: 1,
        exported_at: now_iso(),
        songs: ctx.db.list_songs().map_err(e)?,
        setlists: ctx.db.list_setlists().map_err(e)?,
        presets: ctx.db.list_presets().map_err(e)?.into_iter().filter(|p| !p.builtin).collect(),
        routing_profiles: ctx.db.list_routing_profiles().map_err(e)?,
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&x).map_err(e)?).map_err(e)?;
    logs::info(cat::SYSTEM, format!("Library exported to {path}"));
    Ok(x.songs.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryImportResult {
    pub songs: usize,
    pub skipped_songs: usize,
    pub setlists: usize,
    pub presets: usize,
}

#[tauri::command]
pub fn import_library(ctx: S, path: String) -> R<LibraryImportResult> {
    let x: LibraryExport = serde_json::from_slice(&std::fs::read(&path).map_err(e)?)
        .map_err(|err| format!("Not a valid library export: {err}"))?;
    if x.format != "wpe-library" {
        return Err("Not a Worship Playback Engine library export".into());
    }
    let mut songs = 0;
    let mut skipped = 0;
    for s in x.songs {
        if Path::new(&s.dir).join(&s.source_path).exists() {
            ctx.db.upsert_song(&sanitize_song(s)).map_err(e)?;
            songs += 1;
        } else {
            skipped += 1;
        }
    }
    let lib = ctx.db.list_songs().map_err(e)?;
    let mut setlists = 0;
    for mut sl in x.setlists {
        sl.items.retain(|i| lib.iter().any(|s| s.id == i.song_id));
        ctx.db.save_setlist(&sl).map_err(e)?;
        setlists += 1;
    }
    let mut presets = 0;
    for p in x.presets {
        if ctx.db.save_preset(&p).is_ok() {
            presets += 1;
        }
    }
    for r in x.routing_profiles {
        let _ = ctx.db.save_routing_profile(&r);
    }
    let _ = tauri::Emitter::emit(&ctx.app, "library:changed", ());
    let _ = tauri::Emitter::emit(&ctx.app, "setlists:changed", ());
    Ok(LibraryImportResult { songs, skipped_songs: skipped, setlists, presets })
}
