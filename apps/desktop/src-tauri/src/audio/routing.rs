//! Routing rules: logical buses → physical outputs, with safety validation.

use super::mixer::BusMap;
use crate::models::{OutputTarget, RoutingProfile};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutingIssue {
    /// "error" blocks live use, "warning" is informational.
    pub level: String,
    pub message: String,
}

/// Validate a routing profile for a device with `channels` outputs.
/// The key rule: FOH and Monitor may never share a physical channel,
/// otherwise click/cue would leak into the house.
pub fn validate(profile: &RoutingProfile, channels: u16) -> Vec<RoutingIssue> {
    let mut issues = vec![];
    let err = |m: String| RoutingIssue { level: "error".into(), message: m };
    let warn = |m: String| RoutingIssue { level: "warning".into(), message: m };
    let foh = profile.foh.channels();
    let mon = profile.monitor.channels();
    for c in foh.iter().chain(mon.iter()) {
        if *c >= channels {
            issues.push(err(format!(
                "Output {} does not exist on this device (it has {} outputs).",
                c + 1,
                channels
            )));
        }
    }
    if foh.iter().any(|c| mon.contains(c)) {
        issues.push(err(
            "FOH and Monitor share an output. Click and cue would be audible in the house.".into(),
        ));
    }
    if profile.monitor == OutputTarget::None {
        issues.push(warn("Monitor bus is not routed — the band will not hear click or cue.".into()));
    }
    if profile.foh == OutputTarget::None && profile.mode != "rehearsal" {
        issues.push(warn("FOH bus is not routed — nothing will be sent to the house.".into()));
    }
    if profile.mode == "rehearsal" {
        issues.push(warn("Rehearsal profile: only the monitor mix is played (no FOH output).".into()));
    }
    issues
}

pub fn is_live_safe(profile: &RoutingProfile, channels: u16) -> bool {
    validate(profile, channels).iter().all(|i| i.level != "error")
}

/// Sensible default profile for a device.
pub fn default_profile_for(device_name: &str, channels: u16) -> RoutingProfile {
    let n = device_name.to_lowercase();
    let builtin = n.contains("macbook") || n.contains("speaker") || n.contains("headphone") || n.contains("airpods");
    if builtin {
        RoutingProfile::rehearsal()
    } else if channels >= 4 {
        RoutingProfile::multi()
    } else {
        RoutingProfile::split()
    }
}

pub fn to_bus_map(profile: &RoutingProfile) -> BusMap {
    BusMap { foh: profile.foh.clone(), monitor: profile.monitor.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_outputs_are_rejected() {
        let p = RoutingProfile {
            id: "x".into(),
            name: "bad".into(),
            mode: "custom".into(),
            foh: OutputTarget::Stereo { left: 0, right: 1 },
            monitor: OutputTarget::Mono { channel: 1 },
        };
        assert!(!is_live_safe(&p, 4));
    }

    #[test]
    fn missing_channels_are_rejected() {
        assert!(!is_live_safe(&RoutingProfile::multi(), 2));
        assert!(is_live_safe(&RoutingProfile::multi(), 4));
        assert!(is_live_safe(&RoutingProfile::split(), 2));
    }

    #[test]
    fn defaults_per_device() {
        assert_eq!(default_profile_for("MacBook Air Speakers", 2).mode, "rehearsal");
        assert_eq!(default_profile_for("Scarlett 4i4", 4).mode, "multi");
        assert_eq!(default_profile_for("UR22", 2).mode, "split");
    }
}
