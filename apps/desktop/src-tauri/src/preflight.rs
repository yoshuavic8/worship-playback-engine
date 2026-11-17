//! LIVE PREFLIGHT — verifies a setlist is safe to play before entering Live Mode.

use crate::audio::routing;
use crate::library::{file_exists, song_path};
use crate::models::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreflightItem {
    pub id: String,
    pub label: String,
    /// ok | warning | error
    pub level: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongPreflight {
    pub song_id: String,
    pub title: String,
    pub level: String,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    /// READY | WARNING | NOT READY
    pub verdict: String,
    pub checks: Vec<PreflightItem>,
    pub songs: Vec<SongPreflight>,
}

pub struct DeviceState {
    pub connected: bool,
    pub name: String,
    pub channels: u16,
    pub sample_rate: u32,
}

fn wav_rate(path: &std::path::Path) -> Option<u32> {
    hound::WavReader::open(path).ok().map(|r| r.spec().sample_rate)
}

pub fn run(setlist: &Setlist, songs: &[Option<Song>], device: &DeviceState, profile: &RoutingProfile) -> PreflightReport {
    let mut checks = vec![];
    let mut add = |id: &str, label: &str, level: &str, detail: String| {
        checks.push(PreflightItem { id: id.into(), label: label.into(), level: level.into(), detail })
    };
    let mut per_song = vec![];
    let (mut unprepared, mut missing, mut no_backup, mut wrong_rate, mut no_click, mut no_cue) =
        (0, 0, 0, 0, 0, 0);

    for (item, song) in setlist.items.iter().zip(songs.iter()) {
        let Some(s) = song else {
            missing += 1;
            per_song.push(SongPreflight {
                song_id: item.song_id.clone(),
                title: "(deleted song)".into(),
                level: "error".into(),
                issues: vec!["Song no longer exists in the library".into()],
            });
            continue;
        };
        let mut issues = vec![];
        let mut level = "ok";
        let backing = file_exists(s, &s.rendered.backing);
        let backup = file_exists(s, &s.rendered.backup);
        let source = song_path(s, &s.source_path).is_file();
        if !backing && !backup && !source {
            missing += 1;
            level = "error";
            issues.push("No audio file found".into());
        } else if !backing {
            unprepared += 1;
            level = "warning";
            issues.push(if backup { "Primary backing missing — backup will play".into() } else { "Not prepared — original (with vocals) will play".into() });
        }
        if backing && !backup {
            no_backup += 1;
            if level == "ok" {
                level = "warning";
            }
            issues.push("No backup mix".into());
        }
        if backing {
            if let Some(r) = wav_rate(&song_path(s, s.rendered.backing.as_ref().unwrap())) {
                if r != 48000 {
                    wrong_rate += 1;
                    issues.push(format!("Backing is {r} Hz (expected 48 kHz)"));
                }
            }
        }
        let click = item.click_enabled.unwrap_or(s.routing.click_enabled);
        if click && s.bpm.is_none() {
            no_click += 1;
            if level == "ok" {
                level = "warning";
            }
            issues.push("Click enabled but BPM unknown".into());
        }
        let cue = item.cue_enabled.unwrap_or(s.routing.cue_enabled);
        if cue && s.sections.is_empty() {
            no_cue += 1;
            if level == "ok" {
                level = "warning";
            }
            issues.push("Cue enabled but no sections".into());
        }
        per_song.push(SongPreflight { song_id: s.id.clone(), title: s.title.clone(), level: level.into(), issues });
    }

    let n = setlist.items.len();
    if n == 0 {
        add("songs", "Setlist has songs", "error", "The setlist is empty".into());
    }
    add(
        "prepared",
        "All songs prepared",
        if unprepared == 0 { "ok" } else { "warning" },
        if unprepared == 0 { format!("{n} of {n} live-ready") } else { format!("{unprepared} song(s) not fully prepared") },
    );
    add(
        "assets",
        "Audio files exist",
        if missing == 0 { "ok" } else { "error" },
        if missing == 0 { "All audio present".into() } else { format!("{missing} song(s) have no playable audio") },
    );
    add(
        "rate",
        "Sample rate = 48 kHz",
        if wrong_rate == 0 && device.sample_rate == 48000 { "ok" } else { "warning" },
        format!("Device runs at {} Hz{}", device.sample_rate, if wrong_rate > 0 { format!(", {wrong_rate} file(s) resampled") } else { String::new() }),
    );
    add(
        "device",
        "Audio device connected",
        if device.connected { "ok" } else { "error" },
        if device.connected { format!("{} · {} outputs", device.name, device.channels) } else { "No audio output running".into() },
    );
    let issues = routing::validate(profile, device.channels);
    let routing_err: Vec<_> = issues.iter().filter(|i| i.level == "error").map(|i| i.message.clone()).collect();
    add(
        "foh",
        "FOH routing OK",
        if !routing_err.is_empty() { "error" } else if profile.foh == OutputTarget::None { "warning" } else { "ok" },
        if !routing_err.is_empty() { routing_err.join(" ") } else { format!("{} → {}", "FOH", describe(&profile.foh)) },
    );
    add(
        "monitor",
        "Monitor routing OK",
        if profile.monitor == OutputTarget::None { "error" } else if !routing_err.is_empty() { "error" } else { "ok" },
        format!("Monitor → {} (click & cue never reach FOH)", describe(&profile.monitor)),
    );
    add(
        "click",
        "Click available",
        if no_click == 0 { "ok" } else { "warning" },
        if no_click == 0 { "Procedural click ready".into() } else { format!("{no_click} song(s) without BPM") },
    );
    add(
        "cue",
        "Cue available",
        if no_cue == 0 { "ok" } else { "warning" },
        if no_cue == 0 { "Spoken cues rendered locally".into() } else { format!("{no_cue} song(s) without sections") },
    );
    add(
        "backup",
        "Backup tracks available",
        if no_backup == 0 { "ok" } else { "warning" },
        if no_backup == 0 { "Every prepared song has a backup mix".into() } else { format!("{no_backup} song(s) without backup") },
    );
    let any_err = checks.iter().any(|c| c.level == "error");
    let any_warn = checks.iter().any(|c| c.level == "warning");
    PreflightReport {
        verdict: if any_err { "NOT READY" } else if any_warn { "WARNING" } else { "READY" }.into(),
        checks,
        songs: per_song,
    }
}

pub fn describe(t: &OutputTarget) -> String {
    match t {
        OutputTarget::None => "not routed".into(),
        OutputTarget::Mono { channel } => format!("Output {} (mono)", channel + 1),
        OutputTarget::Stereo { left, right } => format!("Outputs {}-{}", left + 1, right + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setlist(n: usize) -> Setlist {
        Setlist {
            id: "s".into(),
            name: "Sunday".into(),
            date: None,
            notes: String::new(),
            items: (0..n)
                .map(|i| SetlistItem {
                    id: format!("i{i}"),
                    song_id: format!("x{i}"),
                    position: i as u32,
                    preset_id: None,
                    click_enabled: None,
                    cue_enabled: None,
                    foh_enabled: None,
                    monitor_backing: None,
                    start_section_id: None,
                    arrangement: None,
                    transition: Transition::Stop,
                    notes: String::new(),
                })
                .collect(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn dev(connected: bool) -> DeviceState {
        DeviceState { connected, name: "Test".into(), channels: 4, sample_rate: 48000 }
    }

    #[test]
    fn empty_or_disconnected_is_not_ready() {
        let r = run(&setlist(0), &[], &dev(true), &RoutingProfile::multi());
        assert_eq!(r.verdict, "NOT READY");
        let r = run(&setlist(1), &[None], &dev(false), &RoutingProfile::multi());
        assert_eq!(r.verdict, "NOT READY");
    }

    #[test]
    fn unsafe_routing_blocks_live() {
        let mut p = RoutingProfile::multi();
        p.monitor = OutputTarget::Stereo { left: 0, right: 1 };
        let r = run(&setlist(0), &[], &dev(true), &p);
        assert!(r.checks.iter().any(|c| c.id == "foh" && c.level == "error"));
    }
}
