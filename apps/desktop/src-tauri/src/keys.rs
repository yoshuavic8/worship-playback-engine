//! Musical key parsing for transpose-ready storage (pitch class 0–11 + major/minor).

const MAJOR: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const MINOR: [&str; 12] = ["Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm"];

pub fn parse_key_label(label: &str) -> Option<(u8, String)> {
    let s = label.trim();
    if s.is_empty() {
        return None;
    }
    for (i, name) in MINOR.iter().enumerate() {
        if s.eq_ignore_ascii_case(name) {
            return Some((i as u8, "minor".into()));
        }
    }
    for (i, name) in MAJOR.iter().enumerate() {
        if s.eq_ignore_ascii_case(name) {
            return Some((i as u8, "major".into()));
        }
    }
    None
}

pub fn format_key_label(pitch_class: u8, mode: &str) -> String {
    let i = (pitch_class % 12) as usize;
    if mode.eq_ignore_ascii_case("minor") {
        MINOR[i].to_string()
    } else {
        MAJOR[i].to_string()
    }
}

pub fn sync_key_fields(key: &mut Option<String>, pitch: &mut Option<u8>, mode: &mut Option<String>) {
    if let Some(k) = key.as_ref() {
        if let Some((pc, m)) = parse_key_label(k) {
            *pitch = Some(pc);
            *mode = Some(m);
        }
    } else if let (Some(pc), Some(m)) = (pitch, mode) {
        *key = Some(format_key_label(*pc, m));
    }
}

const SHARP: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Key name after transposing, e.g. ("E", -2) → "D", ("E", -1) → "D#", ("Bb", 1) → "B".
/// Flat keys stay flat; everything else is spelled with sharps.
pub fn transposed_name(key: Option<&str>, semitones: i8) -> String {
    let Some(k) = key.filter(|k| !k.trim().is_empty()) else {
        return if semitones == 0 { "original key".into() } else { format!("{semitones:+}") };
    };
    let Some((pc, mode)) = parse_key_label(k) else { return format!("{k} {semitones:+}") };
    let i = ((pc as i32 + semitones as i32).rem_euclid(12)) as usize;
    let minor = mode == "minor";
    let flats = k.contains('b');
    let name = if flats {
        format_key_label(i as u8, "major")
    } else {
        SHARP[i].to_string()
    };
    if minor {
        format!("{name}m")
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transposed_names_follow_the_original_spelling() {
        let names: Vec<String> = (-2..=2).map(|s| transposed_name(Some("E"), s)).collect();
        assert_eq!(names, ["D", "D#", "E", "F", "F#"]);
        assert_eq!(transposed_name(Some("Bb"), 1), "B");
        assert_eq!(transposed_name(Some("Eb"), -1), "D");
        assert_eq!(transposed_name(Some("Am"), 2), "Bm");
        assert_eq!(transposed_name(None, 2), "+2");
    }

    #[test]
    fn round_trip_g_major() {
        let (pc, m) = parse_key_label("G").unwrap();
        assert_eq!(pc, 7);
        assert_eq!(m, "major");
        assert_eq!(format_key_label(pc, &m), "G");
    }
}
