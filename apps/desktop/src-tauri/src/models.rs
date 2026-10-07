//! Core domain types shared between the database, the audio engine and the IPC layer.
//! All types serialize as camelCase JSON to match `apps/desktop/src/lib/types.ts`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SongStatus {
    Imported,
    Queued,
    Processing,
    Ready,
    Warning,
    Error,
}

impl SongStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SongStatus::Imported => "imported",
            SongStatus::Queued => "queued",
            SongStatus::Processing => "processing",
            SongStatus::Ready => "ready",
            SongStatus::Warning => "warning",
            SongStatus::Error => "error",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "queued" => SongStatus::Queued,
            "processing" => SongStatus::Processing,
            "ready" => SongStatus::Ready,
            "warning" => SongStatus::Warning,
            "error" => SongStatus::Error,
            _ => SongStatus::Imported,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StemMode {
    Remove,
    Keep,
    /// Excluded from the backing render but loaded as a separate, live-toggleable track.
    Optional,
}

/// How a song should be prepared for worship. Mirrors the preset definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrepSettings {
    pub preset_id: Option<String>,
    pub vocals: StemMode,
    pub drums: StemMode,
    pub bass: StemMode,
    pub bass_db: f32,
    pub other_db: f32,
    pub click: bool,
    pub cue: bool,
}

impl Default for PrepSettings {
    fn default() -> Self {
        Self {
            preset_id: Some("FULL_BAND".into()),
            vocals: StemMode::Remove,
            drums: StemMode::Remove,
            bass: StemMode::Keep,
            bass_db: -6.0,
            other_db: 0.0,
            click: true,
            cue: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    pub settings: PrepSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StemPaths {
    pub vocals: Option<String>,
    pub drums: Option<String>,
    pub bass: Option<String>,
    pub other: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedPaths {
    pub backing: Option<String>,
    pub backup: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub label: String,
    /// intro | verse | prechorus | chorus | bridge | instrumental | tag | outro | other
    pub kind: String,
    /// Start time in seconds (song time). The end is the next section's start.
    pub start: f64,
}

/// Per-song routing defaults. Click and cue are monitor-only by design and have no FOH flag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SongRouting {
    pub backing_to_foh: bool,
    pub backing_to_monitor: bool,
    pub guide_vocal_to_monitor: bool,
    pub click_enabled: bool,
    pub cue_enabled: bool,
}

impl Default for SongRouting {
    fn default() -> Self {
        Self {
            backing_to_foh: true,
            backing_to_monitor: true,
            guide_vocal_to_monitor: false,
            click_enabled: true,
            cue_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub time_signature: String,
    /// Time (seconds) of the first downbeat, used to anchor the click grid.
    pub beat_offset: f64,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u32,
    /// Absolute path to the song package directory.
    pub dir: String,
    /// Paths below are relative to `dir`.
    pub source_path: String,
    pub stems: StemPaths,
    pub rendered: RenderedPaths,
    pub status: SongStatus,
    pub prep: PrepSettings,
    pub routing: SongRouting,
    pub sections: Vec<Section>,
    /// Ordered section ids. Empty means "play sections in order".
    pub arrangement: Vec<String>,
    pub count_in_bars: u32,
    pub cue_lead_beats: u32,
    pub notes: String,
    pub last_error: Option<String>,
    pub analysis: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub prepared_at: Option<String>,
}

impl Song {
    pub fn beats_per_bar(&self) -> u32 {
        parse_time_signature(&self.time_signature).0
    }
}

pub fn parse_time_signature(ts: &str) -> (u32, u32) {
    let mut it = ts.split('/');
    let n = it.next().and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(4);
    let d = it.next().and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(4);
    (n.clamp(1, 16), d.clamp(1, 16))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    /// Stop after the song, cue up the next one.
    Stop,
    /// Automatically start the next song when this one ends.
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetlistItem {
    pub id: String,
    pub song_id: String,
    pub position: u32,
    pub preset_id: Option<String>,
    pub click_enabled: Option<bool>,
    pub cue_enabled: Option<bool>,
    pub foh_enabled: Option<bool>,
    pub monitor_backing: Option<bool>,
    pub start_section_id: Option<String>,
    pub arrangement: Option<Vec<String>>,
    pub transition: Transition,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Setlist {
    pub id: String,
    pub name: String,
    pub date: Option<String>,
    pub notes: String,
    pub items: Vec<SetlistItem>,
    pub created_at: String,
    pub updated_at: String,
}

/// Where a logical bus is sent on the physical device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OutputTarget {
    None,
    Mono { channel: u16 },
    Stereo { left: u16, right: u16 },
}

impl OutputTarget {
    pub fn channels(&self) -> Vec<u16> {
        match self {
            OutputTarget::None => vec![],
            OutputTarget::Mono { channel } => vec![*channel],
            OutputTarget::Stereo { left, right } => vec![*left, *right],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutingProfile {
    pub id: String,
    pub name: String,
    /// "multi" | "split" | "rehearsal" | "custom"
    pub mode: String,
    pub foh: OutputTarget,
    pub monitor: OutputTarget,
}

impl RoutingProfile {
    pub fn split() -> Self {
        Self {
            id: "split".into(),
            name: "Compatibility — L/R Split".into(),
            mode: "split".into(),
            foh: OutputTarget::Mono { channel: 0 },
            monitor: OutputTarget::Mono { channel: 1 },
        }
    }
    pub fn multi() -> Self {
        Self {
            id: "multi".into(),
            name: "Multi-output — FOH 1-2 / Monitor 3-4".into(),
            mode: "multi".into(),
            foh: OutputTarget::Stereo { left: 0, right: 1 },
            monitor: OutputTarget::Stereo { left: 2, right: 3 },
        }
    }
    pub fn rehearsal() -> Self {
        Self {
            id: "rehearsal".into(),
            name: "Rehearsal — Monitor mix on 1-2".into(),
            mode: "rehearsal".into(),
            foh: OutputTarget::None,
            monitor: OutputTarget::Stereo { left: 0, right: 1 },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub routing: RoutingProfile,
    pub click_volume_db: f32,
    pub cue_volume_db: f32,
    pub monitor_backing_db: f32,
    pub click_sound: String,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device_name: None,
            sample_rate: 48000,
            buffer_size: 256,
            routing: RoutingProfile::rehearsal(),
            click_volume_db: -6.0,
            cue_volume_db: -3.0,
            monitor_backing_db: -6.0,
            click_sound: "classic".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreparationJob {
    pub id: String,
    pub song_id: String,
    pub song_title: String,
    /// queued | running | completed | failed | cancelled
    pub status: String,
    pub stage: String,
    pub progress: f32,
    pub message: String,
    pub error: Option<String>,
    pub model: Option<String>,
    pub device: Option<String>,
    pub processing_seconds: Option<f64>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "song".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Goodness of God"), "goodness-of-god");
        assert_eq!(slugify("  What A Beautiful Name!! "), "what-a-beautiful-name");
        assert_eq!(slugify("???"), "song");
    }

    #[test]
    fn time_signature_parse() {
        assert_eq!(parse_time_signature("6/8"), (6, 8));
        assert_eq!(parse_time_signature("garbage"), (4, 4));
    }
}
