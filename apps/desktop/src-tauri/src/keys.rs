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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_g_major() {
        let (pc, m) = parse_key_label("G").unwrap();
        assert_eq!(pc, 7);
        assert_eq!(m, "major");
        assert_eq!(format_key_label(pc, &m), "G");
    }
}
