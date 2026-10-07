use crate::models::{PrepSettings, Preset, StemMode};

pub fn builtin_presets() -> Vec<Preset> {
    let p = |id: &str, name: &str, desc: &str, drums, bass, bass_db: f32, click, cue| Preset {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        builtin: true,
        settings: PrepSettings {
            preset_id: Some(id.into()),
            vocals: StemMode::Remove,
            drums,
            bass,
            bass_db,
            other_db: 0.0,
            click,
            cue,
        },
    };
    vec![
        p(
            "FULL_BAND",
            "Full Band",
            "Live drummer and bassist. Track keeps keys/pads/guitars, bass is reduced.",
            StemMode::Remove,
            StemMode::Keep,
            -6.0,
            true,
            true,
        ),
        p(
            "NO_DRUMMER",
            "No Drummer",
            "No drummer today — the track keeps its drums.",
            StemMode::Keep,
            StemMode::Keep,
            0.0,
            false,
            false,
        ),
        p(
            "NO_BASS_PLAYER",
            "No Bass Player",
            "Live drummer, no bassist — the track keeps full bass.",
            StemMode::Remove,
            StemMode::Keep,
            0.0,
            true,
            true,
        ),
        p(
            "FULL_TRACK",
            "Full Track",
            "Minimal band or acoustic set. Everything except vocals comes from the track.",
            StemMode::Keep,
            StemMode::Keep,
            0.0,
            false,
            false,
        ),
    ]
}

/// Allowed bass levels exposed in the UI. Anything else is snapped to the nearest step.
pub const BASS_STEPS: [f32; 5] = [-12.0, -9.0, -6.0, -3.0, 0.0];

pub fn snap_bass_db(db: f32) -> f32 {
    let mut best = BASS_STEPS[0];
    for s in BASS_STEPS {
        if (s - db).abs() < (best - db).abs() {
            best = s;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_spec() {
        let p = builtin_presets();
        let full = p.iter().find(|p| p.id == "FULL_BAND").unwrap();
        assert_eq!(full.settings.vocals, StemMode::Remove);
        assert_eq!(full.settings.drums, StemMode::Remove);
        assert_eq!(full.settings.bass_db, -6.0);
        assert!(full.settings.click && full.settings.cue);
        let nd = p.iter().find(|p| p.id == "NO_DRUMMER").unwrap();
        assert_eq!(nd.settings.drums, StemMode::Keep);
        assert!(!nd.settings.click);
    }

    #[test]
    fn bass_snapping() {
        assert_eq!(snap_bass_db(-7.0), -6.0);
        assert_eq!(snap_bass_db(-100.0), -12.0);
        assert_eq!(snap_bass_db(4.0), 0.0);
    }
}
