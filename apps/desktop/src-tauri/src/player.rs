//! Playback controller. Owns the playback/setlist state machines, prepares `LoadedSong`s off the
//! audio thread (with failsafe fallbacks) and publishes status events to the UI.
//! Nothing here ever calls an AI model.

use crate::audio::arrangement::{self, PlanSegment};
use crate::audio::buffer::{db_to_gain, AudioBuffer};
use crate::audio::click::BeatGrid;
use crate::audio::decode;
use crate::audio::mixer::{transport, LoadedSong, MixerCmd, Segment, Track, TrackRole};
use crate::audio::state::{PlaybackEvent, PlaybackState, SetlistEvent, SetlistState};
use crate::core::Ctx;
use crate::library::{file_exists, song_path};
use crate::logs::{self, cat};
use crate::models::*;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Emitter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayContext {
    Live,
    Rehearsal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub role: TrackRole,
    pub gain_db: f32,
    pub muted: bool,
    pub to_foh: bool,
    pub to_mon: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedInfo {
    pub generation: u64,
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub key: Option<String>,
    pub bpm: Option<f64>,
    pub time_signature: String,
    pub duration: f64,
    pub segments: Vec<PlanSegment>,
    pub start_segment: usize,
    pub tracks: Vec<TrackInfo>,
    /// backing | stems | backup | original
    pub source: String,
    pub warnings: Vec<String>,
    pub click_enabled: bool,
    pub click_available: bool,
    pub cue_enabled: bool,
    pub cue_available: bool,
    pub count_in_bars: u32,
    pub context: PlayContext,
    pub live_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub generation: u64,
    pub song_id: Option<String>,
    pub position: f64,
    pub duration: f64,
    pub segment_index: usize,
    pub queued_index: Option<usize>,
    pub looping: bool,
    pub count_in_remaining: f64,
    pub fading: bool,
    pub meters: [f32; 4],
    pub live_index: Option<usize>,
    pub setlist_state: Option<SetlistState>,
    pub engine_running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveInfo {
    pub setlist: Setlist,
    pub songs: Vec<Song>,
    pub index: usize,
    pub state: SetlistState,
}

struct LiveSession {
    setlist: Setlist,
    songs: Vec<Song>,
    index: usize,
    state: SetlistState,
}

struct Inner {
    state: PlaybackState,
    loaded: Option<LoadedInfo>,
    live: Option<LiveSession>,
    generation: u64,
    last_ended: u64,
    last_transport: u8,
}

struct Cache {
    map: HashMap<(PathBuf, u32), (Arc<AudioBuffer>, u64)>,
    tick: u64,
}

const CACHE_LIMIT_BYTES: usize = 3 * 1024 * 1024 * 1024;

pub struct Player {
    inner: Mutex<Inner>,
    cache: Mutex<Cache>,
    load_lock: Mutex<()>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                state: PlaybackState::Stopped,
                loaded: None,
                live: None,
                generation: 0,
                last_ended: 0,
                last_transport: 0,
            }),
            cache: Mutex::new(Cache { map: HashMap::new(), tick: 0 }),
            load_lock: Mutex::new(()),
        }
    }

    // ---------------- buffer cache ----------------

    fn get_buffer(&self, path: &Path, rate: u32) -> Result<Arc<AudioBuffer>> {
        let key = (path.to_path_buf(), rate);
        {
            let mut c = self.cache.lock();
            c.tick += 1;
            let t = c.tick;
            if let Some((b, used)) = c.map.get_mut(&key) {
                *used = t;
                return Ok(b.clone());
            }
        }
        let buf = Arc::new(decode::load_at_rate(path, rate)?);
        let mut c = self.cache.lock();
        c.tick += 1;
        let t = c.tick;
        c.map.insert(key, (buf.clone(), t));
        // evict least recently used until under budget
        loop {
            let total: usize = c.map.values().map(|(b, _)| b.bytes()).sum();
            if total <= CACHE_LIMIT_BYTES || c.map.len() <= 1 {
                break;
            }
            let oldest = c.map.iter().min_by_key(|(_, (_, u))| *u).map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    c.map.remove(&k);
                }
                None => break,
            }
        }
        Ok(buf)
    }

    pub fn invalidate_song(&self, song: &Song) {
        let dir = PathBuf::from(&song.dir);
        self.cache.lock().map.retain(|(p, _), _| !p.starts_with(&dir));
    }

    pub fn clear_cache(&self) {
        self.cache.lock().map.clear();
    }

    /// Click positions from the song's tempo map. A song with only a BPM is one step point.
    fn build_beat_grid(&self, song: &Song, sr: u32, duration: f64) -> Option<BeatGrid> {
        let grid = song.resolved_grid()?;
        crate::audio::tempo_map::to_beat_grid(&grid, sr, duration)
    }

    /// Push a new beat table to the mixer without reloading the audio.
    pub fn apply_grid(&self, ctx: &Ctx, song: &Song) {
        let info = self.inner.lock().loaded.clone();
        let Some(info) = info else { return };
        if info.song_id != song.id {
            return;
        }
        let Some(tempo) = song.resolved_grid() else { return };
        let sr = ctx.engine.sample_rate();
        let Some(grid) = crate::audio::tempo_map::to_beat_grid(&tempo, sr, song.duration.max(info.duration)) else {
            return;
        };
        let cue_at = info
            .segments
            .iter()
            .map(|s| Some(crate::audio::tempo_map::cue_frame(&tempo, s.start, song.cue_lead_beats, sr)))
            .collect();
        ctx.engine.send(MixerCmd::SetBeatTable { grid, cue_at });
    }

    // ---------------- state ----------------

    fn effective_state(&self, ctx: &Ctx) -> PlaybackState {
        let i = self.inner.lock();
        match i.state {
            PlaybackState::Loading | PlaybackState::Nexting | PlaybackState::Error => i.state,
            _ => {
                if i.loaded.is_none() {
                    return PlaybackState::Stopped;
                }
                match ctx.engine.status.transport.load(Ordering::Relaxed) {
                    transport::PLAYING => PlaybackState::Playing,
                    transport::COUNT_IN => PlaybackState::CountIn,
                    transport::PAUSED => PlaybackState::Paused,
                    transport::PANIC => PlaybackState::Panic,
                    _ => PlaybackState::Ready,
                }
            }
        }
    }

    fn apply(&self, ctx: &Ctx, ev: PlaybackEvent) -> Result<PlaybackState> {
        let cur = self.effective_state(ctx);
        let next = cur
            .transition(ev)
            .ok_or_else(|| anyhow!("Cannot {:?} while {:?}", ev, cur))?;
        self.inner.lock().state = next;
        Ok(next)
    }

    pub fn loaded(&self) -> Option<LoadedInfo> {
        self.inner.lock().loaded.clone()
    }

    // ---------------- loading ----------------

    pub fn load(
        &self,
        ctx: &Ctx,
        song_id: &str,
        context: PlayContext,
        item: Option<&SetlistItem>,
        live_index: Option<usize>,
    ) -> Result<LoadedInfo> {
        let _guard = self.load_lock.lock();
        let song = ctx.db.get_song(song_id)?.ok_or_else(|| anyhow!("Song not found"))?;
        if ctx.engine.running().is_none() {
            // try to (re)open the configured device once
            let _ = crate::core::open_audio(ctx);
        }
        if self.apply(ctx, PlaybackEvent::Load).is_err() {
            // Loading while playing is only allowed through Next or after stop; stop first.
            ctx.engine.send(MixerCmd::Stop);
            self.inner.lock().state = PlaybackState::Loading;
        }
        let _ = ctx.app.emit("playback:loading", serde_json::json!({ "songId": song_id }));
        match self.build(ctx, &song, context, item, live_index) {
            Ok((loaded, info)) => {
                if !ctx.engine.send(MixerCmd::Load(Box::new(loaded))) {
                    self.inner.lock().state = PlaybackState::Error;
                    return Err(anyhow!("Audio engine is not running. Check Devices."));
                }
                {
                    let mut i = self.inner.lock();
                    i.loaded = Some(info.clone());
                    i.state = PlaybackState::Ready;
                    i.last_ended = ctx.engine.status.ended_count.load(Ordering::Relaxed);
                }
                logs::info(cat::PLAYBACK, format!("Loaded \"{}\" from {}", info.title, info.source));
                for w in &info.warnings {
                    logs::warn(cat::PLAYBACK, format!("{}: {w}", info.title));
                }
                let _ = ctx.app.emit("playback:songChanged", &info);
                Ok(info)
            }
            Err(e) => {
                self.inner.lock().state = PlaybackState::Error;
                logs::error(cat::PLAYBACK, format!("Load failed for \"{}\": {e:#}", song.title));
                let _ = ctx.app.emit("audio:error", format!("{e}"));
                Err(e)
            }
        }
    }

    fn build(
        &self,
        ctx: &Ctx,
        song: &Song,
        context: PlayContext,
        item: Option<&SetlistItem>,
        live_index: Option<usize>,
    ) -> Result<(LoadedSong, LoadedInfo)> {
        let rate = ctx.engine.sample_rate();
        let audio = ctx.audio.read().clone();
        let rehearsal_profile = audio.routing.mode == "rehearsal";
        let mut warnings = vec![];

        // Effective prep settings: setlist item preset overrides the song's own.
        let mut prep = song.prep.clone();
        let mut preset_override = false;
        if let Some(pid) = item.and_then(|i| i.preset_id.clone()) {
            if Some(&pid) != song.prep.preset_id.as_ref() {
                if let Some(p) = ctx.db.list_presets()?.into_iter().find(|p| p.id == pid) {
                    prep = p.settings;
                    preset_override = true;
                }
            }
        }
        let to_foh = item.and_then(|i| i.foh_enabled).unwrap_or(song.routing.backing_to_foh);
        let to_mon = rehearsal_profile
            || item.and_then(|i| i.monitor_backing).unwrap_or(song.routing.backing_to_monitor);

        let stems_ok = file_exists(song, &song.stems.other)
            && file_exists(song, &song.stems.bass)
            && file_exists(song, &song.stems.drums);
        // Play the stems themselves whenever they exist, including live.
        // Starting levels match the prepared mix; the MD can then ride each
        // track. The rendered backing is only the fallback.
        let use_stems = stems_ok;

        let mut tracks: Vec<Track> = vec![];
        let mut infos: Vec<TrackInfo> = vec![];
        let push = |t: Track, gain_db: f32, tracks: &mut Vec<Track>, infos: &mut Vec<TrackInfo>| {
            infos.push(TrackInfo { role: t.role, gain_db, muted: t.muted, to_foh: t.to_foh, to_mon: t.to_mon });
            tracks.push(t);
        };
        let mut source = String::new();

        if use_stems {
            let stem = |rel: &Option<String>| self.get_buffer(&song_path(song, rel.as_ref().unwrap()), rate);
            match (stem(&song.stems.other), stem(&song.stems.bass), stem(&song.stems.drums)) {
                (Ok(o), Ok(b), Ok(d)) => {
                    push(Track::new(TrackRole::Other, o, db_to_gain(prep.other_db), to_foh, to_mon), prep.other_db, &mut tracks, &mut infos);
                    let mut bt = Track::new(TrackRole::Bass, b, db_to_gain(prep.bass_db), to_foh, to_mon);
                    bt.muted = prep.bass == StemMode::Remove;
                    push(bt, prep.bass_db, &mut tracks, &mut infos);
                    let mut dt = Track::new(TrackRole::Drums, d, 1.0, to_foh, to_mon);
                    dt.muted = prep.drums != StemMode::Keep;
                    push(dt, 0.0, &mut tracks, &mut infos);
                    source = "stems".into();
                }
                _ => warnings.push("Stems could not be loaded — using the rendered backing instead.".into()),
            }
        }

        if source.is_empty() {
            let candidates: [(&str, TrackRole, &Option<String>); 3] = [
                ("backing", TrackRole::Backing, &song.rendered.backing),
                ("backup", TrackRole::Backup, &song.rendered.backup),
                ("original", TrackRole::Original, &Some(song.source_path.clone())),
            ];
            for (name, role, rel) in candidates {
                let Some(rel) = rel else { continue };
                let path = song_path(song, rel);
                if !path.is_file() {
                    if name != "original" {
                        warnings.push(format!("{name}.wav is missing."));
                    }
                    continue;
                }
                match self.get_buffer(&path, rate) {
                    Ok(b) => {
                        push(Track::new(role, b, 1.0, to_foh, to_mon), 0.0, &mut tracks, &mut infos);
                        source = name.into();
                        break;
                    }
                    Err(e) => warnings.push(format!("{name} could not be decoded ({e}). Falling back.")),
                }
            }
            if source.is_empty() {
                return Err(anyhow!("No playable audio found for \"{}\". Re-import the song.", song.title));
            }
            if source == "backup" {
                warnings.push("Playing BACKUP mix (primary backing unavailable).".into());
            }
            if source == "original" && song.rendered.backing.is_none() {
                warnings.push("Song is not prepared — playing the ORIGINAL file (vocals included).".into());
            }
            // Optional drums: excluded from backing, but available as a live toggle.
            if source == "backing" && prep.drums == StemMode::Optional && file_exists(song, &song.stems.drums) {
                if let Ok(d) = self.get_buffer(&song_path(song, song.stems.drums.as_ref().unwrap()), rate) {
                    let mut t = Track::new(TrackRole::Drums, d, 1.0, to_foh, to_mon);
                    t.muted = true;
                    push(t, 0.0, &mut tracks, &mut infos);
                }
            }
        }

        // Guide vocal: monitor only, never FOH. Always loaded when the stem
        // exists so it can be opened from the live mixer.
        if file_exists(song, &song.stems.vocals) {
            if let Ok(v) = self.get_buffer(&song_path(song, song.stems.vocals.as_ref().unwrap()), rate) {
                let mut t = Track::new(TrackRole::Vocals, v, db_to_gain(-3.0), false, true);
                t.muted = !(song.routing.guide_vocal_to_monitor && context == PlayContext::Live)
                    && !(context == PlayContext::Rehearsal && prep.vocals == StemMode::Keep);
                push(t, -3.0, &mut tracks, &mut infos);
            }
        }

        let frames = tracks.iter().map(|t| t.buf.frames()).max().unwrap_or(0);
        let duration = frames as f64 / rate as f64;

        // Arrangement plan.
        let arr = item.and_then(|i| i.arrangement.clone()).unwrap_or_else(|| song.arrangement.clone());
        let plan = arrangement::build_plan(&song.sections, &arr, duration);
        let start_segment = arrangement::start_index(&plan, item.and_then(|i| i.start_section_id.as_deref()));

        let tempo = song.resolved_grid();
        let grid = self.build_beat_grid(&song, ctx.engine.sample_rate(), duration);
        let click_available = grid.is_some();
        let mut click_enabled = item
            .and_then(|i| i.click_enabled)
            .unwrap_or(if preset_override { prep.click } else { song.routing.click_enabled });
        if click_enabled && !click_available {
            warnings.push("Click is enabled but BPM is unknown — click disabled.".into());
            click_enabled = false;
        }

        // Cues (pre-rendered, cached WAVs).
        let mut cue_enabled = item
            .and_then(|i| i.cue_enabled)
            .unwrap_or(if preset_override { prep.cue } else { song.routing.cue_enabled });
        let has_sections = !song.sections.is_empty();
        let voice = ctx.settings.read().cue_voice.clone();
        let cue_root = ctx.paths.read().cues();
        let mut missing_cues: Vec<String> = Vec::new();
        let segments: Vec<Segment> = plan
            .iter()
            .map(|s| {
                let cue = if has_sections {
                    match crate::cue::ensure_cue(&cue_root, voice.as_deref(), &s.label)
                        .and_then(|p| self.get_buffer(&p, rate))
                    {
                        Ok(b) => Some(b),
                        Err(e) => {
                            logs::warn(cat::AUDIO, format!("cue render failed for \"{}\": {e:#}", s.label));
                            missing_cues.push(s.label.clone());
                            None
                        }
                    }
                } else {
                    None
                };
                let cue_at = tempo.as_ref().map(|g| crate::audio::tempo_map::cue_frame(g, s.start, song.cue_lead_beats, rate));
                Segment {
                    start: (s.start * rate as f64).round() as u64,
                    end: ((s.end * rate as f64).round() as u64).min(frames),
                    cue,
                    cue_at,
                }
            })
            .collect();
        let cue_available = has_sections && segments.iter().any(|s| s.cue.is_some());
        if cue_enabled && !has_sections {
            warnings.push("Cue is enabled but the song has no sections.".into());
            cue_enabled = false;
        } else if cue_enabled && has_sections && !cue_available {
            warnings.push("Cue audio could not be generated.".into());
            cue_enabled = false;
        } else if !missing_cues.is_empty() {
            warnings.push(format!("Some cues could not be generated: {}.", missing_cues.join(", ")));
        }

        let bpb = song.beats_per_bar();
        let cue_lead_frames = match grid.as_ref() {
            Some(g) => (g.frames_per_beat * song.cue_lead_beats.max(1) as f64) as u64,
            None => rate as u64 * 2,
        };
        let generation = {
            let mut i = self.inner.lock();
            i.generation += 1;
            i.generation
        };
        let loaded = LoadedSong {
            generation,
            tracks,
            frames,
            segments,
            start_segment,
            grid,
            click_enabled,
            cue_enabled,
            count_in_beats: song.count_in_bars * bpb,
            cue_lead_frames,
        };
        let info = LoadedInfo {
            generation,
            song_id: song.id.clone(),
            title: song.title.clone(),
            artist: song.artist.clone(),
            key: song.key.clone(),
            bpm: song.bpm,
            time_signature: song.time_signature.clone(),
            duration,
            segments: plan,
            start_segment,
            tracks: infos,
            source,
            warnings,
            click_enabled,
            click_available,
            cue_enabled,
            cue_available,
            count_in_bars: song.count_in_bars,
            context,
            live_index,
        };
        Ok((loaded, info))
    }

    /// Decode a song's main audio into the cache ahead of time (used for "next song" in live).
    pub fn preload(&self, ctx: &Ctx, song: &Song) {
        let rate = ctx.engine.sample_rate();
        let rel = [&song.rendered.backing, &song.rendered.backup]
            .into_iter()
            .find(|r| file_exists(song, r))
            .cloned()
            .flatten()
            .unwrap_or_else(|| song.source_path.clone());
        let _ = self.get_buffer(&song_path(song, &rel), rate);
    }

    // ---------------- transport ----------------

    pub fn play(&self, ctx: &Ctx, count_in: bool) -> Result<()> {
        self.apply(ctx, PlaybackEvent::Play)?;
        ctx.engine.send(MixerCmd::Play { count_in });
        self.live_event(SetlistEvent::Play);
        let _ = ctx.app.emit("playback:started", ());
        Ok(())
    }

    pub fn pause(&self, ctx: &Ctx) -> Result<()> {
        self.apply(ctx, PlaybackEvent::Pause)?;
        ctx.engine.send(MixerCmd::Pause);
        let _ = ctx.app.emit("playback:paused", ());
        Ok(())
    }

    pub fn toggle(&self, ctx: &Ctx, count_in: bool) -> Result<()> {
        match self.effective_state(ctx) {
            PlaybackState::Playing | PlaybackState::CountIn => self.pause(ctx),
            PlaybackState::Paused => self.play(ctx, false),
            _ => self.play(ctx, count_in),
        }
    }

    pub fn stop(&self, ctx: &Ctx) -> Result<()> {
        let _ = self.apply(ctx, PlaybackEvent::Stop);
        ctx.engine.send(MixerCmd::Stop);
        self.live_event(SetlistEvent::Stop);
        let _ = ctx.app.emit("playback:stopped", ());
        Ok(())
    }

    /// PANIC: never needs confirmation, never fails.
    pub fn panic(&self, ctx: &Ctx) {
        ctx.engine.send(MixerCmd::Panic);
        self.inner.lock().state = PlaybackState::Panic;
        logs::warn(cat::PLAYBACK, "PANIC STOP");
        let _ = ctx.app.emit("playback:stopped", serde_json::json!({ "panic": true }));
    }

    pub fn fade_out(&self, ctx: &Ctx, seconds: f32) {
        ctx.engine.send(MixerCmd::FadeOut { seconds });
    }

    pub fn seek(&self, ctx: &Ctx, seconds: f64) {
        let rate = ctx.engine.sample_rate() as f64;
        ctx.engine.send(MixerCmd::Seek { frame: (seconds.max(0.0) * rate) as u64 });
    }

    pub fn jump(&self, ctx: &Ctx, index: usize, immediate: bool) {
        ctx.engine.send(MixerCmd::JumpToSegment { index, immediate });
    }

    pub fn cancel_jump(&self, ctx: &Ctx) {
        ctx.engine.send(MixerCmd::CancelJump);
    }

    pub fn set_loop(&self, ctx: &Ctx, on: bool) {
        ctx.engine.send(MixerCmd::SetLoop(on));
    }

    pub fn set_track(&self, ctx: &Ctx, role: TrackRole, gain_db: f32, muted: bool) {
        ctx.engine.send(MixerCmd::SetTrack { role, gain: db_to_gain(gain_db), muted });
        if let Some(l) = self.inner.lock().loaded.as_mut() {
            for t in l.tracks.iter_mut().filter(|t| t.role == role) {
                t.gain_db = gain_db;
                t.muted = muted;
            }
        }
    }

    pub fn set_click(&self, ctx: &Ctx, enabled: bool) {
        let mut i = self.inner.lock();
        if let Some(l) = i.loaded.as_mut() {
            let on = enabled && l.click_available;
            l.click_enabled = on;
            ctx.engine.send(MixerCmd::SetClick { enabled: on });
        }
    }

    pub fn set_cue(&self, ctx: &Ctx, enabled: bool) {
        let mut i = self.inner.lock();
        if let Some(l) = i.loaded.as_mut() {
            let on = enabled && l.cue_available;
            l.cue_enabled = on;
            ctx.engine.send(MixerCmd::SetCue { enabled: on });
        }
    }

    pub fn unload(&self, ctx: &Ctx) {
        ctx.engine.send(MixerCmd::Unload);
        let mut i = self.inner.lock();
        i.loaded = None;
        i.state = PlaybackState::Stopped;
    }

    // ---------------- live session ----------------

    fn live_event(&self, ev: SetlistEvent) {
        if let Some(l) = self.inner.lock().live.as_mut() {
            l.state = l.state.transition(ev);
        }
    }

    pub fn live_info(&self) -> Option<LiveInfo> {
        self.inner.lock().live.as_ref().map(|l| LiveInfo {
            setlist: l.setlist.clone(),
            songs: l.songs.clone(),
            index: l.index,
            state: l.state,
        })
    }

    pub fn live_open(&self, ctx: &Ctx, setlist_id: &str) -> Result<LiveInfo> {
        let setlist = ctx.db.get_setlist(setlist_id)?.ok_or_else(|| anyhow!("Setlist not found"))?;
        if setlist.items.is_empty() {
            return Err(anyhow!("This setlist has no songs yet."));
        }
        let mut songs = vec![];
        for it in &setlist.items {
            songs.push(ctx.db.get_song(&it.song_id)?.ok_or_else(|| anyhow!("A song in this setlist was deleted"))?);
        }
        let _ = self.stop(ctx);
        self.inner.lock().live = Some(LiveSession { setlist, songs, index: 0, state: SetlistState::SetlistReady });
        self.live_goto(ctx, 0)?;
        logs::info(cat::PLAYBACK, "Live session started");
        Ok(self.live_info().unwrap())
    }

    pub fn live_close(&self, ctx: &Ctx) {
        let _ = self.stop(ctx);
        self.inner.lock().live = None;
    }

    pub fn live_goto(&self, ctx: &Ctx, index: usize) -> Result<LoadedInfo> {
        let (item, song_id, next_song) = {
            let mut i = self.inner.lock();
            let l = i.live.as_mut().ok_or_else(|| anyhow!("Live mode is not active"))?;
            if index >= l.songs.len() {
                return Err(anyhow!("End of setlist"));
            }
            l.index = index;
            l.state = l.state.transition(SetlistEvent::LoadSong);
            (l.setlist.items[index].clone(), l.songs[index].id.clone(), l.songs.get(index + 1).cloned())
        };
        let _ = self.apply(ctx, PlaybackEvent::Next);
        let info = self.load(ctx, &song_id, PlayContext::Live, Some(&item), Some(index))?;
        self.live_event(SetlistEvent::SongLoaded);
        if let Some(next) = next_song {
            let c = ctx.arc();
            std::thread::spawn(move || c.player.preload(&c, &next));
        }
        Ok(info)
    }

    pub fn live_step(&self, ctx: &Ctx, delta: i32) -> Result<LoadedInfo> {
        let (cur, len) = {
            let i = self.inner.lock();
            let l = i.live.as_ref().ok_or_else(|| anyhow!("Live mode is not active"))?;
            (l.index as i32, l.songs.len() as i32)
        };
        let n = (cur + delta).clamp(0, len - 1) as usize;
        self.live_goto(ctx, n)
    }

    // ---------------- status polling ----------------

    pub fn status(&self, ctx: &Ctx) -> PlaybackStatus {
        let st = &ctx.engine.status;
        let rate = st.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let state = self.effective_state(ctx);
        let i = self.inner.lock();
        let q = st.queued.load(Ordering::Relaxed);
        PlaybackStatus {
            state,
            generation: st.generation.load(Ordering::Relaxed),
            song_id: i.loaded.as_ref().map(|l| l.song_id.clone()),
            position: st.position.load(Ordering::Relaxed) as f64 / rate,
            duration: i.loaded.as_ref().map(|l| l.duration).unwrap_or(0.0),
            segment_index: st.segment.load(Ordering::Relaxed) as usize,
            queued_index: if q >= 0 { Some(q as usize) } else { None },
            looping: st.looping.load(Ordering::Relaxed),
            count_in_remaining: st.preroll_remaining.load(Ordering::Relaxed) as f64 / rate,
            fading: st.fading.load(Ordering::Relaxed),
            meters: st.take_meters(),
            live_index: i.live.as_ref().map(|l| l.index),
            setlist_state: i.live.as_ref().map(|l| l.state),
            engine_running: ctx.engine.running().is_some(),
        }
    }

    /// Called ~30×/s by the poller thread.
    pub fn tick(&self, ctx: &Ctx) {
        let status = self.status(ctx);
        let _ = ctx.app.emit("playback:position", &status);
        let st = &ctx.engine.status;
        if st.stream_error.swap(false, Ordering::Relaxed) {
            ctx.engine.mark_failed("The audio device stopped or was disconnected.".into());
            logs::error(cat::AUDIO, "Audio stream error — device disconnected?");
            let _ = ctx.app.emit("audio:error", "The audio device stopped or was disconnected.");
            let _ = ctx.app.emit("audio:deviceChanged", ());
        }
        let ended = st.ended_count.load(Ordering::Relaxed);
        let transport_now = st.transport.load(Ordering::Relaxed);
        let (song_ended, advance) = {
            let mut i = self.inner.lock();
            let e = ended != i.last_ended;
            i.last_ended = ended;
            i.last_transport = transport_now;
            let mut adv = None;
            if e {
                if let Some(l) = i.live.as_mut() {
                    l.state = l.state.transition(SetlistEvent::SongEnded);
                    if l.index + 1 < l.songs.len() {
                        let auto = l.setlist.items[l.index].transition == Transition::Auto;
                        adv = Some((l.index + 1, auto));
                    }
                }
            }
            (e, adv)
        };
        if song_ended {
            let _ = ctx.app.emit("playback:ended", ());
            if let Some((next, auto)) = advance {
                if ctx.settings.read().auto_advance || auto {
                    let c = ctx.arc();
                    std::thread::spawn(move || {
                        c.player.live_event(SetlistEvent::Advance);
                        if c.player.live_goto(&c, next).is_ok() && auto {
                            let _ = c.player.play(&c, true);
                        }
                    });
                }
            }
        }
    }
}

pub fn spawn_poller(ctx: Arc<Ctx>) {
    std::thread::Builder::new()
        .name("wpe-status".into())
        .spawn(move || loop {
            ctx.player.tick(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(33));
        })
        .expect("spawn poller");
}
