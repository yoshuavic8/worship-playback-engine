use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub user_name: String,
    pub library_path: Option<String>,
    pub ollama_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: Option<String>,
    pub cue_voice: Option<String>,
    pub ui_scale: f32,
    pub shortcuts: BTreeMap<String, String>,
    pub default_count_in_bars: u32,
    pub separation_model: String,
    /// auto | demucs-mlx | demucs
    pub separation_engine: String,
    /// auto | mps | cpu (PyTorch engine only)
    pub separation_device: String,
    /// worship = Inst HQ 3 vocals + Demucs instruments. fast = Demucs only.
    pub separation_profile: String,
    pub auto_advance: bool,
    pub onboarding_done: bool,
    /// en | id — dates and relative time in the UI
    #[serde(default = "default_ui_locale")]
    pub ui_locale: String,
}

fn default_ui_locale() -> String {
    "en".into()
}

pub fn default_shortcuts() -> BTreeMap<String, String> {
    [
        ("playPause", "Space"),
        ("prevSection", "ArrowLeft"),
        ("nextSection", "ArrowRight"),
        ("panic", "Escape"),
        ("nextSong", "KeyN"),
        ("prevSong", "KeyP"),
        ("fadeOut", "KeyF"),
        ("loopSection", "KeyL"),
        ("stop", "KeyS"),
        ("toggleClick", "KeyC"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            user_name: std::env::var("USER")
                .ok()
                .map(|u| {
                    let mut c = u.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => u,
                    }
                })
                .unwrap_or_else(|| "Friend".into()),
            library_path: None,
            ollama_enabled: true,
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: None,
            cue_voice: Some("Samantha".into()),
            ui_scale: 1.0,
            shortcuts: default_shortcuts(),
            default_count_in_bars: 1,
            separation_model: "htdemucs".into(),
            separation_engine: "auto".into(),
            separation_device: "auto".into(),
            separation_profile: "worship".into(),
            auto_advance: true,
            onboarding_done: false,
            ui_locale: "en".into(),
        }
    }
}
