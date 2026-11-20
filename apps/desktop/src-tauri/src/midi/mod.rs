//! MIDI control (Ableton-style MIDI Learn) and the computer keyboard as a learnable source.
//!
//! CoreMIDI inputs (via `midir`) are polled for hot-plug; every message goes through one
//! dispatcher thread that resolves the mapping and drives the player. Nothing here touches
//! the audio thread directly — it only sends the same commands the UI sends.

pub mod mapping;
pub mod msg;
pub mod quantize;

use crate::audio::mixer::TrackRole;
use crate::audio::state::PlaybackState;
use crate::core::Ctx;
use crate::logs::{self, cat};
use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use mapping::{ButtonMode, Mapping, Source, Target};
use msg::{ControlMsg, Encoder, Pickup, SourceKind};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnArm {
    pub target: Target,
    /// Per-song section mappings belong to this song.
    pub song_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiStatus {
    pub devices: Vec<String>,
    pub armed: Option<LearnArm>,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    device: Option<String>,
    kind: SourceKind,
    channel: u8,
    number: u8,
    value: u8,
    key: Option<String>,
    mapped: bool,
}

pub struct MidiHub {
    maps: RwLock<Vec<Mapping>>,
    armed: Mutex<Option<LearnArm>>,
    pickups: Mutex<HashMap<String, Pickup>>,
    held: Mutex<HashMap<String, bool>>,
    jumps: Mutex<quantize::JumpMemory>,
    devices: RwLock<Vec<String>>,
    error: RwLock<Option<String>>,
    tx: Sender<(Option<String>, ControlMsg)>,
    rx: Mutex<Option<Receiver<(Option<String>, ControlMsg)>>>,
    last_activity: Mutex<Instant>,
    /// Audio settings changed by a knob; written to the database by the poll thread.
    levels_dirty: std::sync::atomic::AtomicBool,
}

impl Default for MidiHub {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiHub {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            maps: RwLock::new(vec![]),
            armed: Mutex::new(None),
            pickups: Mutex::new(HashMap::new()),
            held: Mutex::new(HashMap::new()),
            jumps: Mutex::new(quantize::JumpMemory::default()),
            devices: RwLock::new(vec![]),
            error: RwLock::new(None),
            tx,
            rx: Mutex::new(Some(rx)),
            last_activity: Mutex::new(Instant::now() - Duration::from_secs(1)),
            levels_dirty: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn start(&self, ctx: Arc<Ctx>) {
        if let Ok(maps) = ctx.db.list_midi_mappings() {
            *self.maps.write() = maps;
        }
        let Some(rx) = self.rx.lock().take() else { return };
        let c = ctx.clone();
        std::thread::Builder::new()
            .name("wpe-midi".into())
            .spawn(move || {
                while let Ok((device, m)) = rx.recv() {
                    c.midi.handle(&c, device, m);
                }
            })
            .expect("spawn midi dispatcher");
        let c = ctx;
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("wpe-midi-ports".into())
            .spawn(move || ports_loop(c, tx))
            .expect("spawn midi ports");
    }

    pub fn status(&self) -> MidiStatus {
        MidiStatus {
            devices: self.devices.read().clone(),
            armed: self.armed.lock().clone(),
            available: self.error.read().is_none(),
            error: self.error.read().clone(),
        }
    }

    pub fn mappings(&self) -> Vec<Mapping> {
        self.maps.read().clone()
    }

    pub fn arm(&self, arm: Option<LearnArm>) {
        *self.armed.lock() = arm;
    }

    /// Computer keyboard as a source (forwarded by the UI; only keys that are mapped, or any
    /// key while Learn is armed).
    pub fn key(&self, code: &str, pressed: bool) {
        let _ = self.tx.send((Some("Computer keyboard".into()), msg::key_msg(code, pressed)));
    }

    fn reload(&self, ctx: &Ctx) {
        if let Ok(m) = ctx.db.list_midi_mappings() {
            *self.maps.write() = m;
        }
        self.pickups.lock().clear();
        self.held.lock().clear();
        let _ = ctx.app.emit("midi:mappingsChanged", ());
    }

    pub fn save(&self, ctx: &Ctx, m: Mapping) -> Result<Mapping> {
        let m = m.sanitize();
        ctx.db.save_midi_mapping(&m)?;
        self.reload(ctx);
        Ok(m)
    }

    pub fn delete(&self, ctx: &Ctx, id: &str) -> Result<()> {
        ctx.db.delete_midi_mapping(id)?;
        self.reload(ctx);
        Ok(())
    }

    /// Remove every mapping (scope "all"), only global ones ("global") or one song's ("song:<id>").
    pub fn clear(&self, ctx: &Ctx, scope: &str) -> Result<usize> {
        let doomed: Vec<String> = self
            .maps
            .read()
            .iter()
            .filter(|m| match scope {
                "all" => true,
                "global" => m.song_id.is_none(),
                s => s.strip_prefix("song:").is_some_and(|id| m.song_id.as_deref() == Some(id)),
            })
            .map(|m| m.id.clone())
            .collect();
        for id in &doomed {
            ctx.db.delete_midi_mapping(id)?;
        }
        self.reload(ctx);
        Ok(doomed.len())
    }

    /// Add a preset. Global mappings on the same controls are replaced; per-song ones stay.
    pub fn apply_preset(&self, ctx: &Ctx, name: &str) -> Result<usize> {
        let preset = match name {
            "minilab-mk2" => mapping::minilab_mk2(),
            _ => anyhow::bail!("Unknown preset {name}"),
        };
        let existing = self.maps.read().clone();
        for old in existing.iter().filter(|o| o.song_id.is_none()) {
            if preset.iter().any(|p| p.source.same_control(&old.source)) {
                ctx.db.delete_midi_mapping(&old.id)?;
            }
        }
        for m in &preset {
            ctx.db.save_midi_mapping(m)?;
        }
        self.reload(ctx);
        logs::info(cat::SYSTEM, format!("MIDI preset {name} applied ({} mappings)", preset.len()));
        Ok(preset.len())
    }

    // ---------------- dispatcher ----------------

    fn handle(&self, ctx: &Ctx, device: Option<String>, m: ControlMsg) {
        let loaded_song = ctx.player.loaded().map(|l| l.song_id);
        let armed = self.armed.lock().clone();
        if let Some(arm) = armed {
            // Learn: the first press / knob turn after a control was clicked.
            if m.pressed || m.kind == SourceKind::Cc {
                self.learn(ctx, arm, device.clone(), &m);
            }
            self.activity(ctx, device, &m, true, true);
            return;
        }
        let maps = self.maps.read();
        let found = mapping::resolve(&maps, &m, loaded_song.as_deref()).cloned();
        drop(maps);
        self.activity(ctx, device, &m, found.is_some(), false);
        let Some(map) = found else { return };
        if let Err(e) = self.execute(ctx, &map, &m) {
            logs::warn(cat::PLAYBACK, format!("MIDI action failed: {e:#}"));
        }
    }

    fn activity(&self, ctx: &Ctx, device: Option<String>, m: &ControlMsg, mapped: bool, force: bool) {
        let mut last = self.last_activity.lock();
        if !force && last.elapsed() < Duration::from_millis(60) {
            return;
        }
        *last = Instant::now();
        let _ = ctx.app.emit(
            "midi:activity",
            Activity { device, kind: m.kind, channel: m.channel, number: m.number, value: m.value, key: m.key.clone(), mapped },
        );
    }

    fn learn(&self, ctx: &Ctx, arm: LearnArm, device: Option<String>, m: &ControlMsg) {
        let source = Source::from_msg(m, device);
        // A knob sending 61–63 / 65–67 is an Arturia "Relative #1" encoder.
        let encoder = if m.kind == SourceKind::Cc && arm.target.is_continuous() && matches!(m.value, 61..=63 | 65..=67) {
            Encoder::Relative1
        } else {
            Encoder::Absolute
        };
        let song_id = if arm.target.is_per_song() { arm.song_id.clone() } else { None };
        // One control does one thing in a scope: learning it again moves it here.
        let replaced: Vec<String> = self
            .maps
            .read()
            .iter()
            .filter(|x| x.source.same_control(&source) && x.song_id == song_id)
            .map(|x| x.id.clone())
            .collect();
        for id in &replaced {
            let _ = ctx.db.delete_midi_mapping(id);
        }
        let mapping = Mapping {
            id: crate::models::new_id(),
            song_id,
            source,
            mode: arm.target.default_mode(),
            target: arm.target,
            encoder,
            min: 0.0,
            max: 1.0,
            created_at: crate::models::now_iso(),
        }
        .sanitize();
        match ctx.db.save_midi_mapping(&mapping) {
            Ok(()) => {
                *self.armed.lock() = None;
                self.reload(ctx);
                let _ = ctx.app.emit("midi:learned", serde_json::json!({ "mapping": mapping, "replaced": replaced.len() }));
            }
            Err(e) => logs::error(cat::SYSTEM, format!("Could not save MIDI mapping: {e:#}")),
        }
    }

    /// Edge detection for buttons. Returns Some(true) on press, Some(false) on release.
    fn edge(&self, map: &Mapping, m: &ControlMsg) -> Option<bool> {
        match m.kind {
            SourceKind::Pc => Some(true),
            _ => {
                let mut held = self.held.lock();
                let was = held.insert(map.id.clone(), m.pressed).unwrap_or(false);
                (was != m.pressed).then_some(m.pressed)
            }
        }
    }

    fn execute(&self, ctx: &Ctx, map: &Mapping, m: &ControlMsg) -> Result<()> {
        if map.target.is_continuous() {
            return self.continuous(ctx, map, m);
        }
        let Some(pressed) = self.edge(map, m) else { return Ok(()) };
        if map.target.is_switch() {
            let on = match map.mode {
                ButtonMode::Momentary => pressed,
                _ if !pressed => return Ok(()),
                ButtonMode::Toggle | ButtonMode::Trigger => !switch_state(ctx, &map.target),
            };
            set_switch(ctx, &map.target, on);
            return Ok(());
        }
        if pressed {
            self.trigger(ctx, &map.target)?;
        }
        Ok(())
    }

    fn continuous(&self, ctx: &Ctx, map: &Mapping, m: &ControlMsg) -> Result<()> {
        let Some(cur) = level(ctx, &map.target) else { return Ok(()) };
        let next = match (m.kind, map.encoder) {
            (SourceKind::Cc, Encoder::Absolute) => {
                let v = map.min + (m.value as f32 / 127.0) * (map.max - map.min);
                let mut p = self.pickups.lock();
                match p.entry(map.id.clone()).or_default().feed(v, cur) {
                    Some(v) => v,
                    None => return Ok(()),
                }
            }
            (SourceKind::Cc, enc) => match msg::relative_delta(enc, m.value) {
                Some(d) => (cur + d as f32 * msg::RELATIVE_STEP).clamp(map.min, map.max),
                None => return Ok(()),
            },
            // A pad or key on a fader: jump between min and max.
            _ => {
                if self.edge(map, m) != Some(true) {
                    return Ok(());
                }
                if cur > (map.min + map.max) / 2.0 {
                    map.min
                } else {
                    map.max
                }
            }
        };
        if (next - cur).abs() > 1e-4 {
            set_level(ctx, &map.target, next);
        }
        Ok(())
    }

    fn trigger(&self, ctx: &Ctx, target: &Target) -> Result<()> {
        let p = &ctx.player;
        match target {
            Target::SectionSlot { .. } | Target::SongSection { .. } => {
                let Some(info) = p.loaded() else { return Ok(()) };
                let status = p.status(ctx);
                let ids: Vec<String> = info.segments.iter().map(|s| s.section_id.clone()).collect();
                let Some(index) = mapping::section_index(target, &ids, status.segment_index) else {
                    notice(ctx, "That section is not in this song.");
                    return Ok(());
                };
                self.jump(ctx, index, &status);
            }
            Target::NextSection | Target::PrevSection => {
                let Some(info) = p.loaded() else { return Ok(()) };
                let status = p.status(ctx);
                if info.segments.is_empty() {
                    return Ok(());
                }
                let base = status.queued_index.unwrap_or(status.segment_index) as i64;
                let d = if *target == Target::NextSection { 1 } else { -1 };
                let index = (base + d).clamp(0, info.segments.len() as i64 - 1) as usize;
                p.jump(ctx, index, !rolling(status.state), false);
            }
            Target::CancelJump => {
                self.jumps.lock().clear();
                p.cancel_jump(ctx);
            }
            Target::PlayPause => p.toggle_smart(ctx)?,
            Target::Stop => p.stop(ctx)?,
            Target::FadeOut => {
                let secs = ctx.settings.read().fade_seconds;
                p.fade_out(ctx, secs);
            }
            Target::NextSong | Target::PrevSong => {
                if p.live_info().is_none() {
                    return Ok(());
                }
                if rolling(p.status(ctx).state) {
                    notice(ctx, "Stop or fade the current song first.");
                    return Ok(());
                }
                let d = if *target == Target::NextSong { 1 } else { -1 };
                let c = ctx.arc();
                std::thread::spawn(move || {
                    if let Err(e) = c.player.live_step(&c, d) {
                        notice(&c, &format!("{e:#}"));
                    }
                });
            }
            Target::TransposeUp | Target::TransposeDown => {
                let Some(l) = ctx.player.loaded() else { return Ok(()) };
                if !ctx.player.is_idle(ctx) {
                    notice(ctx, "Change the key before the song starts (stop first).");
                    return Ok(());
                }
                let d = if *target == Target::TransposeUp { 1 } else { -1 };
                let next = l.transpose_wanted as i32 + d;
                if next.abs() > crate::transpose::MAX as i32 {
                    notice(ctx, "Transpose range is −4 to +4.");
                    return Ok(());
                }
                let c = ctx.arc();
                std::thread::spawn(move || match crate::transpose::set_song_key(&c, &l.song_id, next) {
                    Ok(song) => {
                        let name = crate::keys::transposed_name(song.key.as_deref(), song.transpose);
                        let ready = crate::transpose::is_ready(&song, song.transpose);
                        notice(&c, &format!("Key {name}{}", if ready { "" } else { " (preparing…)" }));
                    }
                    Err(e) => notice(&c, &format!("{e:#}")),
                });
            }
            _ => {}
        }
        Ok(())
    }

    /// Same rule as tapping a section on the Live page.
    fn jump(&self, ctx: &Ctx, index: usize, status: &crate::player::PlaybackStatus) {
        let p = &ctx.player;
        if !rolling(status.state) {
            self.jumps.lock().clear();
            p.jump(ctx, index, true, false);
            return;
        }
        let queued = status.queued_index.map(|q| (q, status.queued_bar));
        match self.jumps.lock().press(index, queued, Instant::now()) {
            quantize::JumpMode::Cancel => p.cancel_jump(ctx),
            quantize::JumpMode::Bar => p.jump(ctx, index, false, true),
            quantize::JumpMode::Section => p.jump(ctx, index, false, false),
        }
    }

    pub fn flush_levels(&self, ctx: &Ctx) {
        if self.levels_dirty.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let audio = ctx.audio.read().clone();
            let _ = ctx.db.set_setting("audio", &audio);
            let _ = ctx.app.emit("audio:levelsChanged", &audio);
        }
    }
}

fn rolling(s: PlaybackState) -> bool {
    matches!(s, PlaybackState::Playing | PlaybackState::CountIn)
}

fn notice(ctx: &Ctx, text: &str) {
    let _ = ctx.app.emit("midi:notice", text);
}

fn switch_state(ctx: &Ctx, t: &Target) -> bool {
    let info = ctx.player.loaded();
    let track = |role: TrackRole| info.as_ref().and_then(|l| l.tracks.iter().find(|x| x.role == role).cloned());
    match t {
        Target::Vamp => ctx.player.status(ctx).looping,
        Target::ToggleClick => info.as_ref().map(|l| l.click_enabled).unwrap_or(false),
        Target::ToggleCue => info.as_ref().map(|l| l.cue_enabled).unwrap_or(false),
        Target::TrackMute { role } => track(*role).map(|x| x.muted).unwrap_or(false),
        Target::TrackSolo { role } => track(*role).map(|x| x.solo).unwrap_or(false),
        _ => false,
    }
}

fn set_switch(ctx: &Ctx, t: &Target, on: bool) {
    let p = &ctx.player;
    match t {
        Target::Vamp => p.set_loop(ctx, on),
        Target::ToggleClick => p.set_click(ctx, on),
        Target::ToggleCue => p.set_cue(ctx, on),
        Target::TrackMute { role } => p.set_track_mute(ctx, *role, on),
        Target::TrackSolo { role } => p.set_track_solo(ctx, *role, on),
        _ => return,
    }
    p.emit_mix(ctx);
}

fn level(ctx: &Ctx, t: &Target) -> Option<f32> {
    use mapping::{from_db, LEVEL_DB, TRACK_DB};
    let a = ctx.audio.read();
    match t {
        Target::MasterVolume => Some(from_db(a.foh_master_db, LEVEL_DB)),
        Target::ClickVolume => Some(from_db(a.click_volume_db, LEVEL_DB)),
        Target::CueVolume => Some(from_db(a.cue_volume_db, LEVEL_DB)),
        Target::TrackVolume { role } => ctx
            .player
            .loaded()
            .and_then(|l| l.tracks.iter().find(|x| x.role == *role).map(|x| from_db(x.gain_db, TRACK_DB))),
        _ => None,
    }
}

/// FOH master / click / cue level from the Live page (fast path: no device reopen).
pub fn set_audio_level(ctx: &Ctx, kind: &str, db: f32) {
    let t = match kind {
        "master" => Target::MasterVolume,
        "click" => Target::ClickVolume,
        "cue" => Target::CueVolume,
        _ => return,
    };
    set_level(ctx, &t, mapping::from_db(db.clamp(-40.0, 6.0), mapping::LEVEL_DB));
}

fn set_level(ctx: &Ctx, t: &Target, v: f32) {
    use mapping::{to_db, LEVEL_DB, TRACK_DB};
    match t {
        Target::TrackVolume { role } => {
            ctx.player.set_track_level(ctx, *role, to_db(v, TRACK_DB));
            ctx.player.emit_mix(ctx);
        }
        Target::MasterVolume | Target::ClickVolume | Target::CueVolume => {
            let db = to_db(v, LEVEL_DB);
            let audio = {
                let mut a = ctx.audio.write();
                match t {
                    Target::MasterVolume => a.foh_master_db = db,
                    Target::ClickVolume => a.click_volume_db = db,
                    _ => a.cue_volume_db = db,
                }
                a.clone()
            };
            ctx.engine.set_levels(crate::core::levels_for(&audio));
            ctx.midi.levels_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Watch CoreMIDI inputs: connect new ones, drop vanished ones (hot-plug), every 1.5 s.
fn ports_loop(ctx: Arc<Ctx>, tx: Sender<(Option<String>, ControlMsg)>) {
    use midir::{Ignore, MidiInput, MidiInputConnection};
    let mut conns: HashMap<String, MidiInputConnection<()>> = HashMap::new();
    let mut logged_error = false;
    loop {
        ctx.midi.flush_levels(&ctx);
        match MidiInput::new("WPE scan") {
            Ok(scan) => {
                if ctx.midi.error.read().is_some() {
                    *ctx.midi.error.write() = None;
                }
                let mut names: Vec<String> = vec![];
                let mut bases: Vec<String> = vec![];
                let ports = scan.ports();
                for port in &ports {
                    let base = scan.port_name(port).unwrap_or_else(|_| "MIDI input".into());
                    bases.push(base.clone());
                    let mut name = base.clone();
                    let mut n = 2;
                    while names.contains(&name) {
                        name = format!("{base} #{n}");
                        n += 1;
                    }
                    names.push(name);
                }
                conns.retain(|name, _| {
                    let keep = names.contains(name);
                    if !keep {
                        logs::info(cat::SYSTEM, format!("MIDI input disconnected: {name}"));
                    }
                    keep
                });
                for (i, name) in names.iter().enumerate() {
                    if conns.contains_key(name) {
                        continue;
                    }
                    let Ok(mut input) = MidiInput::new("WPE input") else { continue };
                    input.ignore(Ignore::All);
                    // Find the same port again with this client (the list may have changed).
                    let Some(port) = input.ports().into_iter().nth(i) else { continue };
                    if input.port_name(&port).ok().as_deref() != Some(bases[i].as_str()) {
                        continue;
                    }
                    let tx = tx.clone();
                    let dev = name.clone();
                    match input.connect(
                        &port,
                        "wpe-in",
                        move |_, bytes, _| {
                            if let Some(m) = msg::parse(bytes) {
                                let _ = tx.send((Some(dev.clone()), m));
                            }
                        },
                        (),
                    ) {
                        Ok(c) => {
                            logs::info(cat::SYSTEM, format!("MIDI input connected: {name}"));
                            conns.insert(name.clone(), c);
                        }
                        Err(e) => logs::warn(cat::SYSTEM, format!("MIDI input {name} could not be opened: {e}")),
                    }
                }
                let mut connected: Vec<String> = conns.keys().cloned().collect();
                connected.sort();
                let changed = *ctx.midi.devices.read() != connected;
                if changed {
                    *ctx.midi.devices.write() = connected;
                    let _ = ctx.app.emit("midi:devices", ctx.midi.status());
                }
            }
            Err(e) => {
                if !logged_error {
                    logs::warn(cat::SYSTEM, format!("MIDI is unavailable: {e}"));
                    logged_error = true;
                }
                *ctx.midi.error.write() = Some(e.to_string());
            }
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
}
