use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub user_name: String,
    pub library_path: Option<String>,
    /// Kept in sync with `ai_provider == "ollama"` (older builds read this).
    pub ollama_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: Option<String>,
    /// none | ollama | freellmapi. Empty on settings saved before v0.3.0.
    pub ai_provider: String,
    /// FreeLLMAPI (OpenAI-compatible) base URL. The token is in the Keychain.
    pub freellm_url: String,
    /// auto | auto:fast | auto:smart | a listed model id
    pub freellm_model: String,
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
    /// Speak "Verse… 2… 3… 4" over the last bar before each section.
    pub count_cue: bool,
    /// Fade-out length in seconds (F key / Fade button).
    pub fade_seconds: f32,
}

impl AppSettings {
    /// Fill in the AI provider for settings from older versions and keep `ollama_enabled` in sync.
    pub fn normalize_ai(&mut self) {
        if !["none", "ollama", "freellmapi"].contains(&self.ai_provider.as_str()) {
            self.ai_provider = if self.ollama_enabled { "ollama" } else { "none" }.into();
        }
        self.ollama_enabled = self.ai_provider == "ollama";
        if self.freellm_url.trim().is_empty() {
            self.freellm_url = crate::ai::FREELLM_DEFAULT_URL.into();
        }
        if self.freellm_model.trim().is_empty() {
            self.freellm_model = "auto".into();
        }
    }
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
            ai_provider: String::new(),
            freellm_url: crate::ai::FREELLM_DEFAULT_URL.into(),
            freellm_model: "auto".into(),
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
            count_cue: true,
            fade_seconds: 4.0,
        }
    }
}
