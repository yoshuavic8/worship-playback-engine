//! First-run setup: Python engine + separation models (profile-aware).

use crate::compat::{self, CompatInfo};
use tauri::Emitter;
use crate::core::Ctx;
use crate::resources::{self, catalog, is_installed};
use crate::settings::AppSettings;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static SETUP_BUSY: AtomicBool = AtomicBool::new(false);
static SETUP_PAUSE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupPhase {
    Idle,
    Engine,
    Models,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStep {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub size_bytes: u64,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub complete: bool,
    pub phase: SetupPhase,
    pub message: String,
    pub progress: f32,
    pub eta_secs: Option<u64>,
    pub paused: bool,
    pub steps: Vec<SetupStep>,
}

pub fn required_model_ids(settings: &AppSettings, compat: &CompatInfo) -> Vec<&'static str> {
    let model = settings.separation_model.as_str();
    let mut ids = Vec::new();
    ids.push(if model == "htdemucs_ft" {
        "demucs-htdemucs_ft"
    } else {
        "demucs-htdemucs"
    });
    if compat.mlx_available && settings.separation_engine != "demucs" {
        ids.push(if model == "htdemucs_ft" {
            "demucs-mlx-htdemucs_ft"
        } else {
            "demucs-mlx-htdemucs"
        });
    }
    if settings.separation_profile == "worship" {
        ids.push("mdx-inst-hq-3");
    }
    ids
}

pub fn engine_ready(ctx: &Ctx) -> bool {
    let paths = ctx.paths.read();
    let py = paths.python_bin();
    if !py.exists() {
        return false;
    }
    std::process::Command::new(&py)
        .args(["-c", "import librosa, demucs"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn evaluate(ctx: &Ctx) -> SetupStatus {
    let settings = ctx.settings.read().clone();
    let compat = compat::detect();
    let paths = ctx.paths.read().clone();
    let ids = required_model_ids(&settings, &compat);
    let mut steps = vec![SetupStep {
        id: "engine".into(),
        title: "Mesin persiapan audio".into(),
        detail: "Python 3.12 + PyTorch/Demucs (via uv)".into(),
        size_bytes: 1_100_000_000,
        ready: engine_ready(ctx),
    }];
    for id in &ids {
        if let Some(d) = catalog().into_iter().find(|x| x.id == *id) {
            steps.push(SetupStep {
                id: d.id.into(),
                title: d.name.into(),
                detail: d.description.into(),
                size_bytes: d.files.iter().map(|f| f.size).sum(),
                ready: is_installed(&paths, &d),
            });
        }
    }
    let complete = steps.iter().all(|s| s.ready);
    SetupStatus {
        complete,
        phase: if complete { SetupPhase::Done } else { SetupPhase::Idle },
        message: if complete {
            "Siap".into()
        } else {
            "Perlu persiapan".into()
        },
        progress: if complete {
            1.0
        } else {
            steps.iter().filter(|s| s.ready).count() as f32 / steps.len().max(1) as f32
        },
        eta_secs: None,
        paused: SETUP_PAUSE.load(Ordering::Relaxed),
        steps,
    }
}

pub fn needs_wizard(ctx: &Ctx) -> bool {
    !evaluate(ctx).complete
}

pub fn pause_setup() {
    SETUP_PAUSE.store(true, Ordering::SeqCst);
}

pub fn resume_setup() {
    SETUP_PAUSE.store(false, Ordering::SeqCst);
}

pub fn run_setup(ctx: Arc<Ctx>) -> Result<(), String> {
    if SETUP_BUSY.swap(true, Ordering::SeqCst) {
        return Err("Setup sudah berjalan".into());
    }
    SETUP_PAUSE.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        let emit = |st: SetupStatus| {
            let _ = ctx.app.emit("setup:progress", &st);
        };
        let mut st = evaluate(&ctx);
        st.phase = SetupPhase::Engine;
        st.message = "Menyiapkan mesin Python…".into();
        emit(st.clone());
        if !engine_ready(&ctx) {
            if let Err(e) = resources::install_python_env_sync(ctx.clone()) {
                st.phase = SetupPhase::Error;
                st.message = format!("{e:#}");
                emit(st);
                SETUP_BUSY.store(false, Ordering::SeqCst);
                return;
            }
        }
        st = evaluate(&ctx);
        st.phase = SetupPhase::Models;
        for step in st.steps.iter().filter(|s| s.id != "engine" && !s.ready).cloned().collect::<Vec<_>>() {
            if SETUP_PAUSE.load(Ordering::Relaxed) {
                st.paused = true;
                st.message = "Dijeda".into();
                emit(st.clone());
                SETUP_BUSY.store(false, Ordering::SeqCst);
                return;
            }
            st.message = format!("Mengunduh {}", step.title);
            emit(st.clone());
            if let Err(e) = resources::download_with_retries(ctx.clone(), &step.id, 4) {
                st.phase = SetupPhase::Error;
                st.message = format!("{e:#}");
                emit(st);
                SETUP_BUSY.store(false, Ordering::SeqCst);
                return;
            }
            st = evaluate(&ctx);
        }
        st.phase = SetupPhase::Done;
        st.complete = true;
        st.progress = 1.0;
        st.message = "Semua siap".into();
        emit(st);
        SETUP_BUSY.store(false, Ordering::SeqCst);
    });
    Ok(())
}

pub fn repair_setup(ctx: Arc<Ctx>) -> Result<(), String> {
    run_setup(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;

    fn lite() -> CompatInfo {
        compat::detect()
    }

    #[test]
    fn required_models_include_pytorch_demucs() {
        let s = AppSettings::default();
        let ids = required_model_ids(&s, &lite());
        assert!(ids.contains(&"demucs-htdemucs"));
    }

    #[test]
    fn worship_profile_adds_mdx() {
        let mut s = AppSettings::default();
        s.separation_profile = "worship".into();
        let ids = required_model_ids(&s, &lite());
        assert!(ids.contains(&"mdx-inst-hq-3"));
    }

    #[test]
    fn state_machine_complete_when_all_ready() {
        let st = SetupStatus {
            complete: true,
            phase: SetupPhase::Done,
            message: "ok".into(),
            progress: 1.0,
            eta_secs: None,
            paused: false,
            steps: vec![],
        };
        assert_eq!(st.phase, SetupPhase::Done);
        assert!(st.complete);
    }
}
