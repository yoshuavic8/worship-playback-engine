//! MIDI / keyboard mappings: what a pad, knob or key controls, and which mapping wins.
//! Section buttons can be mapped per song; everything else is global.

use super::msg::{ControlMsg, Encoder, SourceKind};
use crate::audio::mixer::TrackRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub kind: SourceKind,
    /// 0..=15 (shown as 1..16). Ignored for the computer keyboard.
    #[serde(default)]
    pub channel: u8,
    /// Note, CC or program number.
    #[serde(default)]
    pub number: u8,
    /// KeyboardEvent.code for keyboard sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Device the mapping was learned from (display only — any device may trigger it,
    /// so a controller still works after it is unplugged and plugged in again).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl Source {
    pub fn from_msg(m: &ControlMsg, device: Option<String>) -> Self {
        Self { kind: m.kind, channel: m.channel, number: m.number, key: m.key.clone(), device }
    }

    pub fn matches(&self, m: &ControlMsg) -> bool {
        if self.kind != m.kind {
            return false;
        }
        match self.kind {
            SourceKind::Key => self.key.is_some() && self.key == m.key,
            _ => self.channel == m.channel && self.number == m.number,
        }
    }

    pub fn same_control(&self, other: &Source) -> bool {
        self.kind == other.kind
            && match self.kind {
                SourceKind::Key => self.key == other.key,
                _ => self.channel == other.channel && self.number == other.number,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Target {
    /// Global: section N of whatever song is loaded (1-based position in the running order).
    SectionSlot { slot: u32 },
    /// Per song: a specific section (next occurrence in the arrangement).
    SongSection {
        #[serde(rename = "sectionId")]
        section_id: String,
        #[serde(default)]
        label: String,
    },
    PlayPause,
    Stop,
    FadeOut,
    NextSong,
    PrevSong,
    NextSection,
    PrevSection,
    CancelJump,
    Vamp,
    ToggleClick,
    ToggleCue,
    TransposeUp,
    TransposeDown,
    MasterVolume,
    ClickVolume,
    CueVolume,
    TrackVolume { role: TrackRole },
    TrackMute { role: TrackRole },
    TrackSolo { role: TrackRole },
}

impl Target {
    /// Knob / fader targets (0..1 value). Everything else is a button.
    pub fn is_continuous(&self) -> bool {
        matches!(self, Target::MasterVolume | Target::ClickVolume | Target::CueVolume | Target::TrackVolume { .. })
    }

    /// On/off targets that can be toggled or held.
    pub fn is_switch(&self) -> bool {
        matches!(
            self,
            Target::Vamp | Target::ToggleClick | Target::ToggleCue | Target::TrackMute { .. } | Target::TrackSolo { .. }
        )
    }

    pub fn is_per_song(&self) -> bool {
        matches!(self, Target::SongSection { .. })
    }

    pub fn default_mode(&self) -> ButtonMode {
        if self.is_switch() {
            ButtonMode::Toggle
        } else {
            ButtonMode::Trigger
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ButtonMode {
    /// Fires on press (sections, transport).
    #[default]
    Trigger,
    /// Press flips on/off (mute, solo, vamp).
    Toggle,
    /// On while held, off on release (hold-to-vamp, hold-to-mute).
    Momentary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub id: String,
    /// Some = only for this song (section buttons). None = all songs.
    #[serde(default)]
    pub song_id: Option<String>,
    pub source: Source,
    pub target: Target,
    #[serde(default)]
    pub mode: ButtonMode,
    #[serde(default)]
    pub encoder: Encoder,
    /// Range on the 0..1 control (knobs and faders).
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
    #[serde(default)]
    pub created_at: String,
}

fn one() -> f32 {
    1.0
}

impl Mapping {
    pub fn sanitize(mut self) -> Self {
        self.min = if self.min.is_finite() { self.min.clamp(0.0, 1.0) } else { 0.0 };
        self.max = if self.max.is_finite() { self.max.clamp(0.0, 1.0) } else { 1.0 };
        if self.min > self.max {
            std::mem::swap(&mut self.min, &mut self.max);
        }
        if !self.target.is_per_song() {
            self.song_id = None;
        }
        if !self.target.is_switch() && self.mode == ButtonMode::Toggle {
            self.mode = ButtonMode::Trigger;
        }
        if self.source.kind != SourceKind::Cc {
            self.encoder = Encoder::Absolute;
        }
        self
    }
}

/// The mapping for an incoming control. A per-song mapping for the loaded song wins over a
/// global one on the same pad, so pad 1 can be "Chorus" in one song and slot 1 elsewhere.
pub fn resolve<'a>(maps: &'a [Mapping], msg: &ControlMsg, song_id: Option<&str>) -> Option<&'a Mapping> {
    let mut global = None;
    for m in maps.iter().filter(|m| m.source.matches(msg)) {
        match (&m.song_id, song_id) {
            (Some(s), Some(cur)) if s == cur => return Some(m),
            (None, _) if global.is_none() => global = Some(m),
            _ => {}
        }
    }
    global
}

/// Plan index for a section target, given the running order and the current position.
/// A per-song section jumps to its next occurrence after the current part (wrapping).
pub fn section_index(target: &Target, plan_ids: &[String], current: usize) -> Option<usize> {
    match target {
        Target::SectionSlot { slot } => {
            let i = (*slot as usize).checked_sub(1)?;
            (i < plan_ids.len()).then_some(i)
        }
        Target::SongSection { section_id, .. } => {
            let n = plan_ids.len();
            (1..=n)
                .map(|k| (current + k) % n)
                .find(|&i| &plan_ids[i] == section_id)
        }
        _ => None,
    }
}

// ---------------- 0..1 ↔ dB ----------------

pub const TRACK_DB: (f32, f32) = (-24.0, 6.0);
pub const LEVEL_DB: (f32, f32) = (-40.0, 6.0);

pub fn to_db(v: f32, range: (f32, f32)) -> f32 {
    let db = range.0 + v.clamp(0.0, 1.0) * (range.1 - range.0);
    (db * 2.0).round() / 2.0
}

pub fn from_db(db: f32, range: (f32, f32)) -> f32 {
    ((db - range.0) / (range.1 - range.0)).clamp(0.0, 1.0)
}

// ---------------- presets ----------------

/// Arturia MiniLab MkII (see docs/review/RISET-V030.md §5). Assumes the WPE user memory:
/// pads 1–16 send notes 36–51 on channel 10 and the encoders are set to Relative #1 in
/// MIDI Control Center. Pads 9–16 in the factory memory (CC 22–29) are mapped too.
pub fn minilab_mk2() -> Vec<Mapping> {
    let now = crate::models::now_iso();
    let mk = |source: Source, target: Target, mode: ButtonMode, encoder: Encoder| Mapping {
        id: crate::models::new_id(),
        song_id: None,
        source,
        target,
        mode,
        encoder,
        min: 0.0,
        max: 1.0,
        created_at: now.clone(),
    };
    let note = |n: u8| Source { kind: SourceKind::Note, channel: 9, number: n, key: None, device: Some("Arturia MiniLab mkII".into()) };
    let cc = |n: u8| Source { kind: SourceKind::Cc, channel: 0, number: n, key: None, device: Some("Arturia MiniLab mkII".into()) };
    let mut v = vec![];
    for i in 0..8u8 {
        v.push(mk(note(36 + i), Target::SectionSlot { slot: i as u32 + 1 }, ButtonMode::Trigger, Encoder::Absolute));
    }
    let bank2: [(Target, ButtonMode); 8] = [
        (Target::PlayPause, ButtonMode::Trigger),
        (Target::NextSong, ButtonMode::Trigger),
        (Target::PrevSong, ButtonMode::Trigger),
        (Target::Vamp, ButtonMode::Toggle),
        (Target::ToggleClick, ButtonMode::Toggle),
        (Target::TrackMute { role: TrackRole::Vocals }, ButtonMode::Toggle),
        (Target::TransposeDown, ButtonMode::Trigger),
        (Target::TransposeUp, ButtonMode::Trigger),
    ];
    for (i, (t, mode)) in bank2.iter().enumerate() {
        v.push(mk(note(44 + i as u8), t.clone(), *mode, Encoder::Absolute));
        v.push(mk(cc(22 + i as u8), t.clone(), *mode, Encoder::Absolute));
    }
    let knobs: [(u8, Target); 8] = [
        (112, Target::MasterVolume),
        (74, Target::ClickVolume),
        (71, Target::TrackVolume { role: TrackRole::Vocals }),
        (76, Target::TrackVolume { role: TrackRole::Drums }),
        (77, Target::TrackVolume { role: TrackRole::Bass }),
        (93, Target::TrackVolume { role: TrackRole::Other }),
        (73, Target::CueVolume),
        (75, Target::TrackVolume { role: TrackRole::Backing }),
    ];
    for (n, t) in knobs {
        v.push(mk(cc(n), t, ButtonMode::Trigger, Encoder::Relative1));
    }
    // Encoder 1 click: play / pause (spare for pad 9). Sustain pedal: hold to vamp.
    v.push(mk(cc(113), Target::PlayPause, ButtonMode::Trigger, Encoder::Absolute));
    v.push(mk(cc(64), Target::Vamp, ButtonMode::Momentary, Encoder::Absolute));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::msg::parse;

    fn map(id: &str, song: Option<&str>, src: Source, target: Target) -> Mapping {
        Mapping {
            id: id.into(),
            song_id: song.map(String::from),
            source: src,
            target,
            mode: ButtonMode::Trigger,
            encoder: Encoder::Absolute,
            min: 0.0,
            max: 1.0,
            created_at: String::new(),
        }
    }

    fn pad(n: u8) -> Source {
        Source { kind: SourceKind::Note, channel: 9, number: n, key: None, device: None }
    }

    #[test]
    fn per_song_mapping_wins_over_global() {
        let maps = vec![
            map("g", None, pad(36), Target::SectionSlot { slot: 1 }),
            map("a", Some("songA"), pad(36), Target::SongSection { section_id: "chorus".into(), label: "Chorus".into() }),
            map("b", Some("songB"), pad(36), Target::SongSection { section_id: "bridge".into(), label: "Bridge".into() }),
        ];
        let msg = parse(&[0x99, 36, 100]).unwrap();
        assert_eq!(resolve(&maps, &msg, Some("songA")).unwrap().id, "a");
        assert_eq!(resolve(&maps, &msg, Some("songB")).unwrap().id, "b");
        assert_eq!(resolve(&maps, &msg, Some("songC")).unwrap().id, "g", "falls back to the global slot");
        assert_eq!(resolve(&maps, &msg, None).unwrap().id, "g");
    }

    #[test]
    fn channel_and_kind_must_match() {
        let maps = vec![map("g", None, pad(36), Target::PlayPause)];
        assert!(resolve(&maps, &parse(&[0x90, 36, 100]).unwrap(), None).is_none(), "wrong channel");
        assert!(resolve(&maps, &parse(&[0xB9, 36, 100]).unwrap(), None).is_none(), "CC, not a note");
        let key = Source { kind: SourceKind::Key, channel: 0, number: 0, key: Some("KeyQ".into()), device: None };
        let maps = vec![map("k", None, key, Target::Stop)];
        assert!(resolve(&maps, &crate::midi::msg::key_msg("KeyQ", true), None).is_some());
        assert!(resolve(&maps, &crate::midi::msg::key_msg("KeyW", true), None).is_none());
    }

    #[test]
    fn section_targets_resolve_in_the_running_order() {
        let plan: Vec<String> = ["intro", "verse", "chorus", "verse", "chorus", "outro"].iter().map(|s| s.to_string()).collect();
        assert_eq!(section_index(&Target::SectionSlot { slot: 1 }, &plan, 3), Some(0));
        assert_eq!(section_index(&Target::SectionSlot { slot: 6 }, &plan, 0), Some(5));
        assert_eq!(section_index(&Target::SectionSlot { slot: 7 }, &plan, 0), None);
        assert_eq!(section_index(&Target::SectionSlot { slot: 0 }, &plan, 0), None);
        let chorus = Target::SongSection { section_id: "chorus".into(), label: String::new() };
        assert_eq!(section_index(&chorus, &plan, 0), Some(2), "next chorus after the intro");
        assert_eq!(section_index(&chorus, &plan, 2), Some(4), "from the first chorus: the next one");
        assert_eq!(section_index(&chorus, &plan, 5), Some(2), "wraps to the first chorus");
    }

    #[test]
    fn sanitize_keeps_scope_rules() {
        let mut m = map("x", Some("song"), pad(1), Target::PlayPause);
        m.min = 0.9;
        m.max = 0.1;
        m.mode = ButtonMode::Toggle;
        let m = m.sanitize();
        assert_eq!(m.song_id, None, "transport is always global");
        assert_eq!((m.min, m.max), (0.1, 0.9));
        assert_eq!(m.mode, ButtonMode::Trigger);
    }

    #[test]
    fn db_scale_roundtrip() {
        assert_eq!(to_db(1.0, TRACK_DB), 6.0);
        assert_eq!(to_db(0.0, TRACK_DB), -24.0);
        assert!((from_db(0.0, TRACK_DB) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn minilab_preset_is_global_and_unique() {
        let p = minilab_mk2();
        assert!(p.iter().all(|m| m.song_id.is_none()));
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                assert!(!a.source.same_control(&b.source), "duplicate control in preset");
            }
        }
        assert!(p.iter().any(|m| m.target == Target::SectionSlot { slot: 8 }));
        assert!(p.iter().filter(|m| m.target.is_continuous()).all(|m| m.encoder == Encoder::Relative1));
    }
}
