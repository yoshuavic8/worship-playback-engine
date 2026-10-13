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
            "options": { "temperature": 0, "num_ctx": 8192 },
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
    /// 0, 1, 2, or 4. Null when the MD did not mention count-in.
    pub count_in_bars: Option<f64>,
    /// 2, 4, 6, or 8. Null when the MD did not mention the voice cue.
    pub cue_lead_beats: Option<f64>,
    /// Absolute Beat 1 time in seconds.
    pub beat_offset: Option<f64>,
    /// "musical_start" snaps Beat 1 to where the music actually begins.
    pub snap_downbeat: Option<String>,
    /// Milliseconds. Negative moves Beat 1 earlier.
    pub nudge_downbeat_ms: Option<f64>,
    /// Beats. Positive moves Beat 1 later.
    pub nudge_downbeat_beats: Option<f64>,
    pub bpm: Option<f64>,
    /// 0.5 halves the current BPM, 2 doubles it.
    pub bpm_scale: Option<f64>,
    /// A BPM copied from the song's tempo map.
    pub use_region_bpm: Option<f64>,
    pub time_signature: Option<String>,
    pub key: Option<String>,
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
            "countInBars": s("number"),
            "cueLeadBeats": s("number"),
            "beatOffset": s("number"),
            "snapDownbeat": s("string"),
            "nudgeDownbeatMs": s("number"),
            "nudgeDownbeatBeats": s("number"),
            "bpm": s("number"),
            "bpmScale": s("number"),
            "useRegionBpm": s("number"),
            "timeSignature": s("string"),
            "key": s("string"),
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

/// Validated timeline edits. Absent fields are left untouched.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_in_bars: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cue_lead_beats: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

impl TimelinePatch {
    fn is_empty(&self) -> bool {
        self.count_in_bars.is_none()
            && self.cue_lead_beats.is_none()
            && self.beat_offset.is_none()
            && self.bpm.is_none()
            && self.time_signature.is_none()
            && self.key.is_none()
    }
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
    pub timeline: Option<TimelinePatch>,
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

    let timeline = timeline_for_song(song, it, &mut warnings);

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
    let bpb = song.beats_per_bar();
    if let Some(bars) = timeline.count_in_bars {
        diff("Count-in", count_in_label(song.count_in_bars, song.bpm, bpb), count_in_label(bars, timeline.bpm.or(song.bpm), bpb));
    }
    if let Some(beats) = timeline.cue_lead_beats {
        diff("Voice cue lead", format!("{} beats", song.cue_lead_beats), format!("{beats} beats"));
    }
    if let Some(off) = timeline.beat_offset {
        diff("First downbeat", format!("{:.3}s", song.beat_offset), format!("{off:.3}s"));
    }
    if let Some(bpm) = timeline.bpm {
        diff(
            "BPM",
            song.bpm.map(fmt_num).unwrap_or_else(|| "—".into()),
            fmt_num(bpm),
        );
    }
    if let Some(ts) = &timeline.time_signature {
        diff("Time signature", song.time_signature.clone(), ts.clone());
    }
    if let Some(key) = &timeline.key {
        diff("Key", song.key.clone().unwrap_or_else(|| "—".into()), key.clone());
    }
    let timeline = (!timeline.is_empty()).then_some(timeline);

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
        timeline,
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

const TIME_SIGS: &[&str] = &["4/4", "3/4", "2/4", "6/8"];
const MAJOR_KEYS: &[&str] = &["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const MINOR_KEYS: &[&str] = &["Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm"];

fn round_bpm(b: f64) -> f64 {
    (b * 1000.0).round() / 1000.0
}

/// Beat 1 can sit anywhere in the song. A long intro, prayer, or reading
/// is not capped at 30 seconds.
fn clamp_beat(song: &Song, t: f64) -> f64 {
    let hi = if song.duration > 0.0 { song.duration } else { t.max(0.0) };
    round3(t.clamp(0.0, hi))
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn fmt_num(n: f64) -> String {
    if (n - n.round()).abs() < 1e-6 {
        format!("{}", n.round() as i64)
    } else {
        format!("{n:.1}")
    }
}

fn analysis_f64(song: &Song, key: &str) -> Option<f64> {
    song.analysis.as_ref()?.get(key)?.as_f64()
}

fn tempo_regions(song: &Song) -> Vec<(f64, f64)> {
    song.analysis
        .as_ref()
        .and_then(|a| a.get("tempoMap"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let t = r.get("startTime")?.as_f64()?;
                    let bpm = r.get("bpm")?.as_f64()?;
                    Some((t, bpm))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn count_in_label(bars: u32, bpm: Option<f64>, bpb: u32) -> String {
    if bars == 0 {
        return "off".into();
    }
    let unit = if bars == 1 { "bar" } else { "bars" };
    match bpm.filter(|b| *b > 0.0) {
        Some(bpm) => {
            let beats = bars * bpb;
            let secs = beats as f64 * 60.0 / bpm;
            format!("{bars} {unit} (~{secs:.1}s, {beats} beats)")
        }
        None => format!("{bars} {unit}"),
    }
}

fn canonical_key(raw: &str) -> Option<String> {
    MAJOR_KEYS
        .iter()
        .chain(MINOR_KEYS.iter())
        .find(|k| k.eq_ignore_ascii_case(raw.trim()))
        .map(|k| (*k).to_string())
}

/// Apply only the timeline fields the model actually set, and only when they match the editor.
fn timeline_for_song(song: &Song, it: &AiIntent, warnings: &mut Vec<String>) -> TimelinePatch {
    let mut patch = TimelinePatch {
        count_in_bars: None,
        cue_lead_beats: None,
        beat_offset: None,
        bpm: None,
        time_signature: None,
        key: None,
    };
    if let Some(n) = it.count_in_bars {
        let bars = n.round() as i64;
        if [0, 1, 2, 4].contains(&bars) {
            let bars = bars as u32;
            if bars != song.count_in_bars {
                patch.count_in_bars = Some(bars);
            }
        } else {
            warnings.push(format!("Count-in must be 0, 1, 2, or 4 bars — ignored {}.", fmt_num(n)));
        }
    }
    if let Some(n) = it.cue_lead_beats {
        let beats = n.round() as i64;
        if [2, 4, 6, 8].contains(&beats) {
            let beats = beats as u32;
            if beats != song.cue_lead_beats {
                patch.cue_lead_beats = Some(beats);
            }
        } else {
            warnings.push(format!("Voice cue lead must be 2, 4, 6, or 8 beats — ignored {}.", fmt_num(n)));
        }
    }

    if let Some(abs) = it.beat_offset {
        let t = clamp_beat(song, abs);
        if (t - song.beat_offset).abs() > 0.0005 {
            patch.beat_offset = Some(t);
        }
    } else if it.snap_downbeat.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        let snap = it.snap_downbeat.as_deref().unwrap_or("").trim();
        if snap.eq_ignore_ascii_case("musical_start") {
            match analysis_f64(song, "musicalStart") {
                Some(t) => {
                    let t = clamp_beat(song, t);
                    if (t - song.beat_offset).abs() > 0.0005 {
                        patch.beat_offset = Some(t);
                    }
                }
                None => warnings.push("No musical start was detected — downbeat left unchanged.".into()),
            }
        } else {
            warnings.push(format!("Unknown downbeat snap \"{snap}\" — ignored."));
        }
    } else if it.nudge_downbeat_ms.is_some() || it.nudge_downbeat_beats.is_some() {
        let mut delta = 0.0;
        if let Some(ms) = it.nudge_downbeat_ms {
            let clamped = ms.clamp(-2000.0, 2000.0);
            if (clamped - ms).abs() > 0.01 {
                warnings.push("Downbeat nudge limited to ±2000 ms.".into());
            }
            delta += clamped / 1000.0;
        }
        if let Some(beats) = it.nudge_downbeat_beats {
            match song.bpm.filter(|b| *b > 0.0) {
                Some(bpm) => {
                    let clamped = beats.clamp(-8.0, 8.0);
                    if (clamped - beats).abs() > 0.01 {
                        warnings.push("Downbeat nudge limited to ±8 beats.".into());
                    }
                    delta += clamped * (60.0 / bpm);
                }
                None => warnings.push("Cannot nudge the downbeat by beats — the song has no BPM.".into()),
            }
        }
        let t = clamp_beat(song, song.beat_offset + delta);
        if (t - song.beat_offset).abs() > 0.0005 {
            patch.beat_offset = Some(t);
        }
    }

    if let Some(b) = it.bpm {
        let b = round_bpm(b);
        if (20.0..=300.0).contains(&b) {
            if song.bpm.map(|cur| (cur - b).abs() > 0.05).unwrap_or(true) {
                patch.bpm = Some(b);
            }
        } else {
            warnings.push(format!("BPM {} is outside 20–300 — ignored.", fmt_num(b)));
        }
    } else if let Some(region) = it.use_region_bpm {
        let regions = tempo_regions(song);
        match regions.iter().find(|(_, bpm)| (bpm - region).abs() < 0.2) {
            Some((_, bpm)) => {
                let bpm = round_bpm(*bpm);
                if song.bpm.map(|cur| (cur - bpm).abs() > 0.05).unwrap_or(true) {
                    patch.bpm = Some(bpm);
                }
            }
            None => warnings.push(format!("BPM {} is not in this song's tempo map — ignored.", fmt_num(region))),
        }
    } else if let Some(scale) = it.bpm_scale {
        let factor = if (scale - 0.5).abs() < 0.01 {
            Some(0.5)
        } else if (scale - 2.0).abs() < 0.01 {
            Some(2.0)
        } else {
            None
        };
        match (factor, song.bpm.filter(|b| *b > 0.0)) {
            (Some(factor), Some(cur)) => {
                let b = round_bpm((cur * factor).clamp(20.0, 300.0));
                if (b - cur).abs() > 0.05 {
                    patch.bpm = Some(b);
                }
            }
            (None, _) => warnings.push("BPM scale must be ÷2 (0.5) or ×2 (2) — ignored.".into()),
            (_, None) => warnings.push("Cannot scale BPM — the song has no tempo yet.".into()),
        }
    }

    if let Some(ts) = it.time_signature.as_ref().map(|s| s.replace(' ', "")).filter(|s| !s.is_empty()) {
        if TIME_SIGS.contains(&ts.as_str()) {
            if ts != song.time_signature {
                patch.time_signature = Some(ts);
            }
        } else {
            warnings.push(format!("Time signature \"{ts}\" is not supported — ignored."));
        }
    }
    if let Some(raw) = it.key.as_ref().filter(|k| !k.trim().is_empty()) {
        match canonical_key(raw) {
            Some(key) if song.key.as_deref() != Some(key.as_str()) => patch.key = Some(key),
            Some(_) => {}
            None => warnings.push(format!("Key \"{raw}\" is not a concert key this app uses — ignored.")),
        }
    }
    patch
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
        timeline: None,
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

Timeline (read the SONG block — it already has count-in, Beat 1, tempo mode, tempo regions, and each section's bar:beat and duration). Set these ONLY when the MD asks:
- countInBars: 0, 1, 2, or 4. "hitung 2 bar" / "count-in 2 bars" → 2. "tanpa count" → 0.
- cueLeadBeats: 2, 4, 6, or 8 (how early the voice cue speaks).
- beatOffset: absolute seconds of Beat 1, only when the MD gives a time.
- snapDownbeat: "musical_start" when they want Beat 1 where the music starts (the Musical start line).
- nudgeDownbeatMs: milliseconds, negative = earlier ("mundur 10 ms" → -10). nudgeDownbeatBeats: positive = later ("maju 1 beat" → 1, "mundur 1 beat" → -1).
- bpm: a specific number. bpmScale: 0.5 to halve, 2 to double. useRegionBpm: copy a BPM number from Tempo regions ("pakai tempo yang lebih cepat" / "use that region's BPM").
- timeSignature: one of 4/4, 3/4, 2/4, 6/8. key: a concert key such as G or Bm.
The click always follows the tempo map (including drifting or changing tempos). Do not mention "locking" the beat grid — that control no longer exists.
Indonesian hints: "tidak main"/"nggak main"/"ga ada" = not playing, "main" = playing, "ulang" = repeat, "skip"/"lewati" = skip, "hitung" = count-in, "ketukan 1" / "downbeat" = Beat 1.
Use null for anything the MD did not mention. Do not invent a BPM or a downbeat. intent = "update_song" for settings or timeline, "arrangement" if only structure changes, "unknown" if the request is unrelated.
explanation: one short sentence in the MD's language describing what you changed."#;

pub const SETLIST_SYSTEM_PROMPT: &str = r#"You are the assistant of a worship playback app. Build a setlist from the user's song LIBRARY only.
Return intent "setlist", "songs" = ordered list of song ids taken EXACTLY from the library list, an optional "setlistName",
and a one-sentence "explanation" in the user's language. Consider tempo flow (fast songs first, slower worship songs later
unless asked otherwise) and key compatibility. Never invent songs. If nothing fits, return an empty list."#;

fn fmt_secs(t: f64) -> String {
    format!("{t:.2}s")
}

fn bar_beat_label(t: f64, bpm: Option<f64>, offset: f64, bpb: u32) -> String {
    let Some(bpm) = bpm.filter(|b| *b > 0.0) else {
        return fmt_secs(t);
    };
    let beat = 60.0 / bpm;
    let beat_num = ((t - offset) / beat).round() as i64;
    let bpb = bpb.max(1) as i64;
    let bar = beat_num.div_euclid(bpb) + 1;
    let b = beat_num.rem_euclid(bpb) + 1;
    format!("Bar {bar} · Beat {b}")
}

/// Compact song brief for the local model. Includes the timeline the editor shows:
/// count-in, first downbeat, tempo mode, tempo regions, and each section's bar:beat and duration.
pub fn song_context(song: &Song) -> String {
    let bpb = song.beats_per_bar();
    let pct = |v: f64| format!("{}%", (v * 100.0).round() as i64);
    let mut lines = vec![format!("SONG: {} — {}", song.title, song.artist)];

    let mut bpm = song.bpm.map(fmt_num).unwrap_or_else(|| "unknown".into());
    if let Some(c) = analysis_f64(song, "bpmConfidence") {
        bpm.push_str(&format!(" (confidence {})", pct(c)));
    }
    lines.push(format!("BPM: {bpm}"));

    let mut key = song.key.clone().unwrap_or_else(|| "unknown".into());
    if let Some(c) = analysis_f64(song, "keyConfidence") {
        key.push_str(&format!(" (confidence {})", pct(c)));
    }
    lines.push(format!("Key: {key}"));
    lines.push(format!("Time signature: {}", song.time_signature));
    lines.push(format!(
        "First downbeat (Beat 1): {}{}",
        fmt_secs(song.beat_offset),
        analysis_f64(song, "downbeatConfidence")
            .map(|c| format!(" (confidence {})", pct(c)))
            .unwrap_or_default()
    ));
    match (analysis_f64(song, "musicalStart"), analysis_f64(song, "audioStart")) {
        (Some(m), Some(a)) => lines.push(format!("Musical start: {} · file audio from {}", fmt_secs(m), fmt_secs(a))),
        (Some(m), None) => lines.push(format!("Musical start: {}", fmt_secs(m))),
        (None, Some(a)) => lines.push(format!("File audio from {}", fmt_secs(a))),
        (None, None) => {}
    }
    if let Some(mode) = song.analysis.as_ref().and_then(|a| a.get("tempoMode")).and_then(|v| v.as_str()) {
        let stability = analysis_f64(song, "stability").map(|s| format!(" · stability {}", pct(s))).unwrap_or_default();
        lines.push(format!("Tempo mode: {mode}{stability}"));
    }
    lines.push(format!(
        "Count-in: {} · voice cue lead: {} beats",
        count_in_label(song.count_in_bars, song.bpm, bpb),
        song.cue_lead_beats
    ));

    let regions = tempo_regions(song);
    if !regions.is_empty() {
        lines.push("Tempo regions (copy a BPM into useRegionBpm to adopt it):".into());
        for (i, (start, bpm)) in regions.iter().take(12).enumerate() {
            let end = regions.get(i + 1).map(|(t, _)| fmt_secs(*t)).unwrap_or_else(|| "end".into());
            let delta = if i == 0 {
                String::new()
            } else {
                format!(" ({:+.1})", bpm - regions[i - 1].1)
            };
            lines.push(format!("  {}–{} · {:.1} BPM{delta}", fmt_secs(*start), end, bpm));
        }
        if regions.len() > 12 {
            lines.push(format!("  … {} more regions", regions.len() - 12));
        }
    }

    if song.sections.is_empty() {
        lines.push("Sections: (none yet)".into());
    } else {
        lines.push("Sections (label @ start · bar:beat · duration):".into());
        let shown = song.sections.len().min(32);
        for (i, s) in song.sections.iter().take(shown).enumerate() {
            let end = song.sections.get(i + 1).map(|n| n.start).unwrap_or(song.duration);
            let dur = (end - s.start).max(0.0);
            lines.push(format!(
                "  {} @ {} · {} · {}",
                s.label,
                fmt_secs(s.start),
                bar_beat_label(s.start, song.bpm, song.beat_offset, bpb),
                fmt_secs(dur)
            ));
        }
        if song.sections.len() > shown {
            lines.push(format!("  … {} more sections", song.sections.len() - shown));
        }
    }

    lines.push(format!(
        "Current mix: vocals={} drums={} bass={} bassDb={} click={} cue={}",
        mode_str(song.prep.vocals),
        mode_str(song.prep.drums),
        mode_str(song.prep.bass),
        song.prep.bass_db,
        song.routing.click_enabled,
        song.routing.cue_enabled,
    ));

    if let Some(warnings) = song.analysis.as_ref().and_then(|a| a.get("warnings")).and_then(|v| v.as_array()) {
        let notes: Vec<String> = warnings
            .iter()
            .filter_map(|w| w.as_str())
            .take(3)
            .map(|w| {
                let w = w.trim();
                if w.chars().count() > 160 {
                    format!("{}…", w.chars().take(160).collect::<String>())
                } else {
                    w.to_string()
                }
            })
            .collect();
        if !notes.is_empty() {
            lines.push("Analysis notes:".into());
            for n in notes {
                lines.push(format!("  - {n}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
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

    fn song_with_timeline() -> Song {
        let mut s = song();
        s.count_in_bars = 1;
        s.cue_lead_beats = 4;
        s.beat_offset = 0.4;
        s.bpm = Some(100.0);
        s.analysis = Some(serde_json::json!({
            "tempoMode": "drifting",
            "stability": 0.62,
            "downbeatConfidence": 0.41,
            "bpmConfidence": 0.8,
            "musicalStart": 2.5,
            "audioStart": 0.2,
            "beatGridLocked": false,
            "tempoMap": [
                { "startBeat": 0, "startTime": 0.4, "bpm": 100.0, "timeSignature": "4/4" },
                { "startBeat": 64, "startTime": 38.8, "bpm": 108.0, "timeSignature": "4/4" }
            ],
            "warnings": ["Live tempo drift detected — verify the downbeat."]
        }));
        s
    }

    #[test]
    fn song_brief_includes_timeline() {
        let mut s = song_with_timeline();
        s.beat_offset = 0.0;
        let brief = song_context(&s);
        assert!(brief.contains("Count-in: 1 bar"), "{brief}");
        assert!(brief.contains("First downbeat (Beat 1): 0.00s"), "{brief}");
        assert!(brief.contains("Musical start: 2.50s"), "{brief}");
        assert!(brief.contains("Tempo mode: drifting"), "{brief}");
        assert!(brief.contains("108.0 BPM (+8.0)"), "{brief}");
        assert!(brief.contains("Intro @ 0.00s · Bar 1 · Beat 1"), "{brief}");
        assert!(brief.contains("Analysis notes:"), "{brief}");
    }

    #[test]
    fn count_in_is_applied() {
        let raw = r#"{"intent":"update_song","countInBars":2,"explanation":"Count-in 2 bar."}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(raw).unwrap());
        let t = p.timeline.unwrap();
        assert_eq!(t.count_in_bars, Some(2));
        assert!(p.changes.iter().any(|c| c.field == "Count-in" && c.to.starts_with("2 bars")));
    }

    #[test]
    fn bad_timeline_values_are_rejected() {
        let raw = r#"{"intent":"update_song","countInBars":3,"cueLeadBeats":5,"timeSignature":"7/8","key":"H","bpmScale":3,"explanation":"x"}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(raw).unwrap());
        assert!(p.timeline.is_none());
        assert!(p.warnings.len() >= 4);
    }

    #[test]
    fn snap_downbeat_uses_musical_start() {
        let raw = r#"{"intent":"update_song","snapDownbeat":"musical_start","explanation":"Beat 1 di awal musik."}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(raw).unwrap());
        assert_eq!(p.timeline.unwrap().beat_offset, Some(2.5));
    }

    #[test]
    fn nudge_downbeat_by_one_beat() {
        let raw = r#"{"intent":"update_song","nudgeDownbeatBeats":1,"explanation":"maju 1 beat"}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(raw).unwrap());
        // 100 BPM → 0.6s per beat, offset 0.4 + 0.6
        assert_eq!(p.timeline.unwrap().beat_offset, Some(1.0));
    }

    #[test]
    fn region_bpm_and_halve_are_validated() {
        let adopt = r#"{"intent":"update_song","useRegionBpm":108,"explanation":"pakai tempo yang lebih cepat"}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(adopt).unwrap());
        assert_eq!(p.timeline.unwrap().bpm, Some(108.0));

        let missing = r#"{"intent":"update_song","useRegionBpm":90,"explanation":"x"}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(missing).unwrap());
        assert!(p.timeline.is_none());

        let half = r#"{"intent":"update_song","bpmScale":0.5,"explanation":"setengah"}"#;
        let p = rules_for_song(&song_with_timeline(), &parse_intent(half).unwrap());
        assert_eq!(p.timeline.unwrap().bpm, Some(50.0));
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
