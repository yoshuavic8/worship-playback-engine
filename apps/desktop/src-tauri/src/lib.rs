//! Worship Playback Engine — Tauri/Rust backend.
//!
//! AI prepares. Rules decide. Audio Engine performs.
//! If AI disappears, worship playback must continue.

pub mod ai;
pub mod audio;
pub mod commands;
pub mod core;
pub mod cue;
pub mod db;
pub mod library;
pub mod logs;
pub mod models;
pub mod paths;
pub mod player;
pub mod preflight;
pub mod prep;
pub mod presets;
pub mod resources;
pub mod settings;
pub mod system;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&root)?;
            let resource_dir = app.path().resource_dir().ok();
            let ctx = core::init(app.handle().clone(), root, resource_dir)
                .map_err(|e| Box::<dyn std::error::Error>::from(format!("{e:#}")))?;
            app.manage(ctx);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::save_settings,
            commands::audio_status,
            commands::list_audio_devices,
            commands::save_audio_settings,
            commands::test_signal,
            commands::list_voices,
            commands::preview_cue,
            commands::list_songs,
            commands::import_songs,
            commands::save_song,
            commands::accept_song_review,
            commands::delete_song,
            commands::duplicate_song,
            commands::get_waveform,
            commands::rerender_backing,
            commands::set_downbeat,
            commands::set_grid,
            commands::prepare_songs,
            commands::cancel_job,
            commands::list_jobs,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            commands::list_setlists,
            commands::get_setlist,
            commands::save_setlist,
            commands::delete_setlist,
            commands::duplicate_setlist,
            commands::export_setlist,
            commands::import_setlist,
            commands::player_load,
            commands::player_play,
            commands::player_pause,
            commands::player_stop,
            commands::player_panic,
            commands::player_fade,
            commands::player_seek,
            commands::player_jump,
            commands::player_cancel_jump,
            commands::player_loop,
            commands::player_set_track,
            commands::player_set_click,
            commands::player_set_cue,
            commands::player_loaded,
            commands::live_open,
            commands::live_goto,
            commands::live_step,
            commands::live_close,
            commands::live_info,
            commands::run_preflight,
            commands::system_readiness,
            commands::list_resources,
            commands::download_resource,
            commands::cancel_download,
            commands::remove_resource,
            commands::install_python_env,
            commands::get_logs,
            commands::ai_status,
            commands::ai_song_command,
            commands::ai_setlist_command,
            commands::export_library,
            commands::import_library,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Worship Playback Engine");
}
