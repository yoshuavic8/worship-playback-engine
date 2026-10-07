//! Application context shared by commands, the poller and background workers.

use crate::audio::buffer::db_to_gain;
use crate::audio::click::ClickSound;
use crate::audio::engine::{AudioEngine, RunningInfo};
use crate::audio::mixer::MixLevels;
use crate::audio::routing;
use crate::db::Db;
use crate::logs::{self, cat};
use crate::models::AudioSettings;
use crate::paths::AppPaths;
use crate::player::Player;
use crate::prep::PrepQueue;
use crate::resources::ResourceManager;
use crate::settings::AppSettings;
use anyhow::Result;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};
use tauri::AppHandle;

pub struct Ctx {
    pub app: AppHandle,
    pub db: Db,
    pub paths: RwLock<AppPaths>,
    pub engine: AudioEngine,
    pub settings: RwLock<AppSettings>,
    pub audio: RwLock<AudioSettings>,
    pub player: Player,
    pub queue: PrepQueue,
    pub resources: ResourceManager,
    pub worker_dir: PathBuf,
    me: OnceLock<Weak<Ctx>>,
}

impl Ctx {
    pub fn arc(&self) -> Arc<Ctx> {
        self.me.get().and_then(|w| w.upgrade()).expect("context alive")
    }
}

pub fn levels_for(audio: &AudioSettings) -> MixLevels {
    let rehearsal = audio.routing.mode == "rehearsal";
    MixLevels {
        click: db_to_gain(audio.click_volume_db),
        cue: db_to_gain(audio.cue_volume_db),
        monitor_backing: if rehearsal { 1.0 } else { db_to_gain(audio.monitor_backing_db) },
        foh_master: 1.0,
        monitor_master: 1.0,
    }
}

pub fn init(app: AppHandle, root: PathBuf, resource_dir: Option<PathBuf>) -> Result<Arc<Ctx>> {
    let boot_paths = AppPaths::new(root.clone(), None);
    logs::init(&boot_paths.logs());
    logs::info(cat::SYSTEM, format!("Starting Worship Playback Engine {}", env!("CARGO_PKG_VERSION")));
    let db = Db::open(&boot_paths.db())?;
    db.recover_interrupted_jobs()?;
    let settings: AppSettings = db.get_setting("app").unwrap_or_default();
    let mut audio: AudioSettings = db.get_setting("audio").unwrap_or_default();
    let paths = AppPaths::new(root, settings.library_path.clone().map(PathBuf::from));
    let engine = AudioEngine::new(
        routing::to_bus_map(&audio.routing),
        levels_for(&audio),
        ClickSound::parse(&audio.click_sound),
    );
    if audio.sample_rate == 0 {
        audio.sample_rate = 48000;
    }
    let worker_dir = crate::paths::worker_dir(resource_dir);
    let ctx = Arc::new(Ctx {
        app,
        db,
        paths: RwLock::new(paths),
        engine,
        settings: RwLock::new(settings),
        audio: RwLock::new(audio),
        player: Player::new(),
        queue: PrepQueue::new(),
        resources: ResourceManager::new(),
        worker_dir,
        me: OnceLock::new(),
    });
    let _ = ctx.me.set(Arc::downgrade(&ctx));
    if let Err(e) = open_audio(&ctx) {
        logs::error(cat::AUDIO, format!("Audio device could not be opened: {e:#}"));
    }
    ctx.queue.start(ctx.clone());
    crate::player::spawn_poller(ctx.clone());
    Ok(ctx)
}

/// Open the configured device (or the system default), apply a safe routing profile and persist it.
pub fn open_audio(ctx: &Ctx) -> Result<RunningInfo> {
    let mut audio = ctx.audio.read().clone();
    let devices = crate::audio::engine::list_devices();
    let device = audio
        .device_name
        .clone()
        .filter(|n| devices.iter().any(|d| &d.name == n));
    if audio.device_name.is_some() && device.is_none() {
        logs::warn(cat::AUDIO, format!("Configured device {:?} not found — using system default", audio.device_name));
    }
    let info = ctx.engine.open(device.clone(), audio.sample_rate, audio.buffer_size)?;
    let first_run = ctx.db.get_setting::<serde_json::Value>("audio").is_none();
    if first_run {
        audio.routing = routing::default_profile_for(&info.device_name, info.channels);
    }
    // Ensure routing fits the device; otherwise fall back to the default profile for it.
    if !routing::is_live_safe(&audio.routing, info.channels) {
        let p = routing::default_profile_for(&info.device_name, info.channels);
        logs::warn(
            cat::ROUTING,
            format!("Routing \"{}\" is not valid for {} — switched to \"{}\"", audio.routing.name, info.device_name, p.name),
        );
        audio.routing = p;
    }
    ctx.engine.set_bus(routing::to_bus_map(&audio.routing));
    ctx.engine.set_levels(levels_for(&audio));
    let rates: Vec<u32> = devices
        .iter()
        .find(|d| d.name == info.device_name)
        .map(|d| d.sample_rates.clone())
        .unwrap_or_default();
    let _ = ctx.db.remember_device(&info.device_name, info.channels, &rates, Some(&audio.routing.id));
    *ctx.audio.write() = audio.clone();
    let _ = ctx.db.set_setting("audio", &audio);
    logs::info(
        cat::AUDIO,
        format!(
            "Audio running on \"{}\" — {} ch, {} Hz, buffer {:?}",
            info.device_name, info.channels, info.sample_rate, info.buffer_size
        ),
    );
    Ok(info)
}
