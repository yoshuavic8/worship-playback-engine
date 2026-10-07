//! Optional local AI assistant.
//!
//!   User → AIProvider (Ollama, local) → structured JSON → schema validation → Rules Engine
//!        → proposal shown to the user → user confirms → normal app commands
//!
//! AI prepares. Rules decide. Audio Engine performs.
//! The LLM can never execute commands, touch files, download anything or reach the audio engine.

use crate::models::*;
use crate::presets::{builtin_presets, snap_bass_db};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub trait AIProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete_json(&self, system: &str, user: &str, schema: &serde_json::Value) -> Result<String>;
}

pub struct NoAIProvider;

impl AIProvider for NoAIProvider {
    fn name(&self) -> &'static str {
        "none"
    }
    fn complete_json(&self, _: &str, _: &str, _: &serde_json::Value) -> Result<String> {
        Err(anyhow!(
            "The AI assistant is offline (Ollama not available). Presets and manual controls always work."
        ))
    }
}

pub struct OllamaProvider {
    pub url: String,
    pub model: String,
}

impl AIProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }
    fn complete_json(&self, system: &str, user: &str, schema: &serde_json::Value) -> Result<String> {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(90)).build()?;
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "format": schema,
            "options": { "temperature": 0, "num_ctx": 4096 },
            "keep_alive": "5m",
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });
        let r = client
            .post(format!("{}/api/chat", self.url.trim_end_matches('/')))
            .json(&body)
            .send()
            .map_err(|e| anyhow!("Ollama is not reachable ({e})"))?;
        if !r.status().is_success() {
            return Err(anyhow!("Ollama returned {}", r.status()));
        }
        let j: serde_json::Value = r.json()?;
        j["message"]["content"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| anyhow!("Ollama returned no content"))
    }
}

/// Pick the provider: Ollama if enabled and reachable, otherwise NoAIProvider.
pub fn provider(settings: &crate::settings::AppSettings) -> Box<dyn AIProvider> {
    if !settings.ollama_enabled {
        return Box::new(NoAIProvider);
    }
    match crate::system::ollama_models(&settings.ollama_url) {
        Ok(models) if !models.is_empty() => {
            let model = settings
                .ollama_model
                .clone()
                .filter(|m| models.contains(m))
                .or_else(|| models.iter().find(|m| m.starts_with("qwen2.5")).cloned())
                .or_else(|| models.iter().find(|m| m.contains("llama3")).cloned())
                .unwrap_or_else(|| models[0].clone());
            Box::new(OllamaProvider { url: settings.ollama_url.clone(), model })
        }
        _ => Box::new(NoAIProvider),
    }
}

// ---------------- schema ----------------

/// Raw structured output requested from the model. Every field is optional; the Rules Engine decides.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AiIntent {
    pub intent: String,
    pub preset: Option<String>,
    pub vocals: Option<String>,
    pub drums: Option<String>,
    pub bass: Option<String>,
    pub bass_db: Option<f64>,
    pub click: Option<bool>,
    pub cue: Option<bool>,
    pub guide_vocal: Option<bool>,
    pub arrangement: Option<Vec<String>>,
    pub songs: Option<Vec<String>>,
    pub setlist_name: Option<String>,
    pub explanation: String,
}

pub fn intent_schema() -> serde_json::Value {
    let s = |t: &str| serde_json::json!({ "type": [t, "null"] });
    serde_json::json!({
        "type": "object",
        "properties": {
            "intent": { "type": "string", "enum": ["update_song", "arrangement", "setlist", "unknown"] },
            "preset": s("string"),
            "vocals": s("string"),
            "drums": s("string"),
            "bass": s("string"),
            "bassDb": s("number"),
            "click": s("boolean"),
            "cue": s("boolean"),
            "guideVocal": s("boolean"),
            "arrangement": { "type": ["array", "null"], "items": { "type": "string" } },
            "songs": { "type": ["array", "null"], "items": { "type": "string" } },
            "setlistName": s("string"),
            "explanation": { "type": "string" }
        },
        "required": ["intent", "explanation"]
    })
}

pub fn parse_intent(raw: &str) -> Result<AiIntent> {
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).map_err(|e| anyhow!("The AI answer was not valid JSON ({e})"))?;
    if !v.is_object() {
        return Err(anyhow!("The AI answer was not a JSON object"));
    }
    let intent: AiIntent = serde_json::from_value(v).map_err(|e| anyhow!("The AI answer did not match the schema ({e})"))?;
    if !["update_song", "arrangement", "setlist", "unknown"].contains(&intent.intent.as_str()) {
        return Err(anyhow!("Unknown intent \"{}\"", intent.intent));
    }
    Ok(intent)
}

// ---------------- rules engine ----------------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub field: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposal {
    pub intent: String,
    pub summary: String,
    pub changes: Vec<Change>,
    pub prep: Option<PrepSettings>,
    pub routing: Option<SongRouting>,
    pub arrangement: Option<Vec<String>>,
    pub setlist_song_ids: Vec<String>,
    pub setlist_name: Option<String>,
    pub warnings: Vec<String>,
    pub requires_rerender: bool,
    pub provider: String,
    pub model: Option<String>,
}

fn mode_str(m: StemMode) -> &'static str {
    match m {
        StemMode::Remove => "REMOVE",
        StemMode::Keep => "KEEP",
        StemMode::Optional => "OPTIONAL",
    }
}

fn parse_mode(s: &str, allow_optional: bool) -> Option<StemMode> {
    match s.trim().to_ascii_uppercase().as_str() {
        "REMOVE" | "OFF" | "MUTE" => Some(StemMode::Remove),
        "KEEP" | "ON" => Some(StemMode::Keep),
        "OPTIONAL" if allow_optional => Some(StemMode::Optional),
        _ => None,
    }
}

/// Validate and normalise a song-settings intent against the current song. Authoritative.
pub fn rules_for_song(song: &Song, it: &AiIntent) -> AiProposal {
    let mut warnings = vec![];
    let mut prep = song.prep.clone();
    let mut routing = song.routing.clone();
    if let Some(pid) = it.preset.as_ref().filter(|p| !p.is_empty()) {
        match builtin_presets().into_iter().find(|p| p.id.eq_ignore_ascii_case(pid)) {
            Some(p) => {
                prep = p.settings.clone();
                routing.click_enabled = p.settings.click;
                routing.cue_enabled = p.settings.cue;
            }
            None => warnings.push(format!("Ignored unknown preset \"{pid}\".")),
        }
    }
    let set_mode = |name: &str, v: &Option<String>, target: &mut StemMode, opt: bool, w: &mut Vec<String>| {
        if let Some(s) = v.as_ref().filter(|s| !s.is_empty()) {
            match parse_mode(s, opt) {
                Some(m) => *target = m,
                None => w.push(format!("Ignored invalid {name} value \"{s}\".")),
            }
        }
    };
    set_mode("vocals", &it.vocals, &mut prep.vocals, false, &mut warnings);
    set_mode("drums", &it.drums, &mut prep.drums, true, &mut warnings);
    set_mode("bass", &it.bass, &mut prep.bass, false, &mut warnings);
    if let Some(db) = it.bass_db {
        let snapped = snap_bass_db(db as f32);
        if (snapped as f64 - db).abs() > 0.01 {
            warnings.push(format!("Bass level {db} dB adjusted to the allowed step {snapped} dB."));
        }
        prep.bass_db = snapped;
        if prep.bass == StemMode::Remove && it.bass.is_none() {
            prep.bass = StemMode::Keep;
        }
    }
    if prep.vocals == StemMode::Keep {
        warnings.push("Vocals will stay in the backing track — unusual for live worship.".into());
    }
    if let Some(c) = it.click {
        routing.click_enabled = c;
    }
    if let Some(c) = it.cue {
        routing.cue_enabled = c;
    }
    if let Some(g) = it.guide_vocal {
        routing.guide_vocal_to_monitor = g;
    }
    if routing.click_enabled && song.bpm.is_none() {
        warnings.push("Click requested but the song has no BPM yet — prepare or set BPM first.".into());
    }
    if routing.cue_enabled && song.sections.is_empty() {
        warnings.push("Cue requested but the song has no sections yet.".into());
    }
    if prep != song.prep {
        prep.preset_id = builtin_presets()
            .into_iter()
            .find(|p| {
                let mut a = p.settings.clone();
                let mut b = prep.clone();
                a.preset_id = None;
                b.preset_id = None;
                a.click = b.click;
                a.cue = b.cue;
                a == b
            })
            .map(|p| p.id);
    }

    let arrangement = it.arrangement.as_ref().filter(|a| !a.is_empty()).map(|labels| {
        let mut ids = vec![];
        for l in labels {
            let l = l.trim();
            let found = song
                .sections
                .iter()
                .find(|s| s.label.eq_ignore_ascii_case(l))
                .or_else(|| song.sections.iter().find(|s| s.kind.eq_ignore_ascii_case(l)))
                .or_else(|| {
                    // "Chorus 2" → second chorus, falling back to the first one
                    let base = l.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ');
                    song.sections.iter().find(|s| s.label.to_lowercase().starts_with(&base.to_lowercase()))
                });
            match found {
                Some(s) => ids.push(s.id.clone()),
                None => warnings.push(format!("Section \"{l}\" does not exist in this song — skipped.")),
            }
        }
        ids
    });

    let mut changes = vec![];
    let mut diff = |field: &str, a: String, b: String| {
        if a != b {
            changes.push(Change { field: field.into(), from: a, to: b });
        }
    };
    diff("Vocals", mode_str(song.prep.vocals).into(), mode_str(prep.vocals).into());
    diff("Drums", mode_str(song.prep.drums).into(), mode_str(prep.drums).into());
    diff(
        "Bass",
        if song.prep.bass == StemMode::Remove { "REMOVE".into() } else { format!("{} dB", song.prep.bass_db) },
        if prep.bass == StemMode::Remove { "REMOVE".into() } else { format!("{} dB", prep.bass_db) },
    );
    diff("Click", on_off(song.routing.click_enabled), on_off(routing.click_enabled));
    diff("Cue", on_off(song.routing.cue_enabled), on_off(routing.cue_enabled));
    diff("Guide vocal → Monitor", on_off(song.routing.guide_vocal_to_monitor), on_off(routing.guide_vocal_to_monitor));
    let label_of = |ids: &[String]| {
        ids.iter()
            .filter_map(|id| song.sections.iter().find(|s| &s.id == id).map(|s| s.label.clone()))
            .collect::<Vec<_>>()
            .join(" → ")
    };
    if let Some(a) = &arrangement {
        let cur = if song.arrangement.is_empty() {
            song.sections.iter().map(|s| s.label.clone()).collect::<Vec<_>>().join(" → ")
        } else {
            label_of(&song.arrangement)
        };
        diff("Arrangement", cur, label_of(a));
    }

    let stems_changed = prep.vocals != song.prep.vocals
        || prep.drums != song.prep.drums
        || prep.bass != song.prep.bass
        || prep.bass_db != song.prep.bass_db
        || prep.other_db != song.prep.other_db;
    AiProposal {
        intent: it.intent.clone(),
        summary: it.explanation.trim().to_string(),
        changes,
        prep: Some(prep),
        routing: Some(routing),
        arrangement,
        setlist_song_ids: vec![],
        setlist_name: None,
        warnings,
        requires_rerender: stems_changed && song.rendered.backing.is_some(),
        provider: String::new(),
        model: None,
    }
}

fn on_off(b: bool) -> String {
    if b { "ON".into() } else { "OFF".into() }
}

pub fn rules_for_setlist(library: &[Song], it: &AiIntent) -> AiProposal {
    let mut warnings = vec![];
    let mut ids = vec![];
    for s in it.songs.clone().unwrap_or_default() {
        let s = s.trim();
        let found = library
            .iter()
            .find(|x| x.id == s)
            .or_else(|| library.iter().find(|x| x.title.eq_ignore_ascii_case(s)));
        match found {
            Some(song) if !ids.contains(&song.id) => ids.push(song.id.clone()),
            Some(_) => {}
            None => warnings.push(format!("\"{s}\" is not in your library — skipped.")),
        }
    }
    if ids.len() > 12 {
        ids.truncate(12);
        warnings.push("Limited to 12 songs.".into());
    }
    if ids.is_empty() {
        warnings.push("No matching songs were suggested.".into());
    }
    AiProposal {
        intent: "setlist".into(),
        summary: it.explanation.trim().to_string(),
        changes: ids
            .iter()
            .enumerate()
            .map(|(i, id)| Change {
                field: format!("{:02}", i + 1),
                from: String::new(),
                to: library.iter().find(|s| &s.id == id).map(|s| s.title.clone()).unwrap_or_default(),
            })
            .collect(),
        prep: None,
        routing: None,
        arrangement: None,
        setlist_song_ids: ids,
        setlist_name: it.setlist_name.clone().filter(|n| !n.trim().is_empty()),
        warnings,
        requires_rerender: false,
        provider: String::new(),
        model: None,
    }
}

// ---------------- prompts ----------------

pub const SONG_SYSTEM_PROMPT: &str = r#"You are the assistant of a worship playback app used by a church Music Director (MD).
You translate the MD's request (Indonesian or English) into JSON settings for ONE song. You never control playback.

Meaning of the fields (the backing track replaces musicians who are NOT playing live):
- vocals: "REMOVE" (default, singers sing live) or "KEEP".
- drums: "REMOVE" when a live drummer plays; "KEEP" when there is NO drummer (track provides drums); "OPTIONAL" when unsure (drums can be toggled live).
- bass: "KEEP" or "REMOVE". bassDb is one of -12, -9, -6, -3, 0.
  * live bassist plays → bass "KEEP" with bassDb -6 (support only), or "REMOVE" if the MD wants it fully removed.
  * NO bassist → bass "KEEP" with bassDb 0.
- click / cue: true or false. A live drummer usually needs click=true. guideVocal: guide vocal in the monitor mix.
- preset (optional): FULL_BAND (live drums+bass), NO_DRUMMER, NO_BASS_PLAYER, FULL_TRACK (only singers/acoustic live).
- arrangement: ordered list of section labels copied EXACTLY from the song's section list, only if the MD asks to change the song structure (repeat, skip, jump). Otherwise null.
Indonesian hints: "tidak main"/"nggak main"/"ga ada" = not playing, "main" = playing, "ulang" = repeat, "skip"/"lewati" = skip.
Use null for anything the MD did not mention. intent = "update_song" for settings, "arrangement" if only structure changes, "unknown" if the request is unrelated.
explanation: one short sentence in the MD's language describing what you changed."#;

pub const SETLIST_SYSTEM_PROMPT: &str = r#"You are the assistant of a worship playback app. Build a setlist from the user's song LIBRARY only.
Return intent "setlist", "songs" = ordered list of song ids taken EXACTLY from the library list, an optional "setlistName",
and a one-sentence "explanation" in the user's language. Consider tempo flow (fast songs first, slower worship songs later
unless asked otherwise) and key compatibility. Never invent songs. If nothing fits, return an empty list."#;

pub fn song_context(song: &Song) -> String {
    let sections = if song.sections.is_empty() {
        "(no sections yet)".to_string()
    } else {
        song.sections.iter().map(|s| s.label.clone()).collect::<Vec<_>>().join(", ")
    };
    format!(
        "SONG: {} — {}\nBPM: {}\nKey: {}\nSections (in order): {}\nCurrent: vocals={} drums={} bass={} bassDb={} click={} cue={}\n",
        song.title,
        song.artist,
        song.bpm.map(|b| format!("{b}")).unwrap_or("unknown".into()),
        song.key.clone().unwrap_or("unknown".into()),
        sections,
        mode_str(song.prep.vocals),
        mode_str(song.prep.drums),
        mode_str(song.prep.bass),
        song.prep.bass_db,
        song.routing.click_enabled,
        song.routing.cue_enabled,
    )
}

pub fn library_context(songs: &[Song]) -> String {
    let mut s = String::from("LIBRARY (id | title | artist | key | bpm | ready):\n");
    for x in songs.iter().take(200) {
        s.push_str(&format!(
            "{} | {} | {} | {} | {} | {}\n",
            x.id,
            x.title,
            x.artist,
            x.key.clone().unwrap_or("-".into()),
            x.bpm.map(|b| format!("{b:.0}")).unwrap_or("-".into()),
            matches!(x.status, SongStatus::Ready)
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> Song {
        let mut s: Song = serde_json::from_value(serde_json::json!({
            "id": "g", "title": "Goodness of God", "artist": "Bethel", "bpm": 74.0, "key": "A",
            "timeSignature": "4/4", "beatOffset": 0.0, "duration": 280.0, "sampleRate": 48000, "channels": 2,
            "dir": "/tmp", "sourcePath": "source/original.mp3", "stems": {}, "rendered": {"backing": "rendered/backing.wav"},
            "status": "ready", "prep": PrepSettings::default(), "routing": SongRouting::default(),
            "sections": [], "arrangement": [], "countInBars": 1, "cueLeadBeats": 4, "notes": "",
            "lastError": null, "analysis": null, "createdAt": "", "updatedAt": "", "preparedAt": null
        }))
        .unwrap();
        for (i, l) in ["Intro", "Verse 1", "Chorus", "Verse 2", "Chorus", "Bridge", "Outro"].iter().enumerate() {
            s.sections.push(Section { id: format!("s{i}"), label: l.to_string(), kind: l.to_lowercase(), start: i as f64 * 20.0 });
        }
        s
    }

    /// OLLAMA ACCEPTANCE: "drummer tidak main, bassist main" → validated settings.
    #[test]
    fn no_drummer_bassist_plays() {
        let raw = r#"{"intent":"update_song","preset":"NO_DRUMMER","drums":"KEEP","bass":"KEEP","bassDb":-6,"click":false,"explanation":"Drum dari track, bass dikurangi."}"#;
        let it = parse_intent(raw).unwrap();
        let p = rules_for_song(&song(), &it);
        let prep = p.prep.unwrap();
        assert_eq!(prep.drums, StemMode::Keep);
        assert_eq!(prep.bass, StemMode::Keep);
        assert_eq!(prep.bass_db, -6.0);
        assert!(!p.routing.unwrap().click_enabled);
        assert!(p.requires_rerender);
        assert!(p.changes.iter().any(|c| c.field == "Drums" && c.to == "KEEP"));
    }

    #[test]
    fn invalid_values_are_rejected_not_applied() {
        let raw = r#"{"intent":"update_song","drums":"EXPLODE","bassDb":-7.4,"preset":"rm -rf /","explanation":"x"}"#;
        let p = rules_for_song(&song(), &parse_intent(raw).unwrap());
        let prep = p.prep.unwrap();
        assert_eq!(prep.drums, StemMode::Remove);
        assert_eq!(prep.bass_db, -6.0);
        assert!(p.warnings.len() >= 3);
    }

    #[test]
    fn garbage_is_not_json() {
        assert!(parse_intent("sure! here you go").is_err());
        assert!(parse_intent(r#"{"intent":"format_disk","explanation":""}"#).is_err());
        assert!(parse_intent("[1,2]").is_err());
    }

    #[test]
    fn arrangement_maps_labels_to_sections() {
        let raw = r#"{"intent":"arrangement","arrangement":["Intro","Verse 1","Chorus","Bridge","Chorus","Outro","Tag"],"explanation":"skip verse 2"}"#;
        let p = rules_for_song(&song(), &parse_intent(raw).unwrap());
        let a = p.arrangement.unwrap();
        assert_eq!(a, vec!["s0", "s1", "s2", "s5", "s2", "s6"]);
        assert!(p.warnings.iter().any(|w| w.contains("Tag")));
    }

    #[test]
    fn setlist_only_uses_library_songs() {
        let lib = vec![song()];
        let raw = r#"{"intent":"setlist","songs":["g","made-up-song","Goodness of God"],"explanation":"ok"}"#;
        let p = rules_for_setlist(&lib, &parse_intent(raw).unwrap());
        assert_eq!(p.setlist_song_ids, vec!["g"]);
        assert_eq!(p.warnings.len(), 1);
    }
}
