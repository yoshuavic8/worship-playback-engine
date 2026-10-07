//! Worship arrangement engine: turns sections + an ordered list of section ids into a playback plan.
//! The audio file is never edited — the mixer jumps between segment boundaries sample-accurately.

use crate::models::Section;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanSegment {
    pub section_id: String,
    pub label: String,
    pub kind: String,
    pub start: f64,
    pub end: f64,
}

/// Sections sorted by start with resolved end times. The first section always starts at 0.
pub fn resolve_sections(sections: &[Section], duration: f64) -> Vec<PlanSegment> {
    let mut s: Vec<&Section> = sections.iter().filter(|s| s.start < duration).collect();
    s.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    if s.is_empty() {
        return vec![PlanSegment {
            section_id: "__song".into(),
            label: "Song".into(),
            kind: "other".into(),
            start: 0.0,
            end: duration,
        }];
    }
    let mut out = Vec::with_capacity(s.len());
    for (i, sec) in s.iter().enumerate() {
        let start = if i == 0 { 0.0 } else { sec.start };
        let end = s.get(i + 1).map(|n| n.start).unwrap_or(duration);
        if end - start < 0.05 {
            continue;
        }
        out.push(PlanSegment {
            section_id: sec.id.clone(),
            label: sec.label.clone(),
            kind: sec.kind.clone(),
            start,
            end,
        });
    }
    out
}

/// Build the ordered plan. Unknown ids in `arrangement` are ignored; an empty arrangement is linear.
pub fn build_plan(sections: &[Section], arrangement: &[String], duration: f64) -> Vec<PlanSegment> {
    let resolved = resolve_sections(sections, duration);
    if arrangement.is_empty() {
        return resolved;
    }
    let plan: Vec<PlanSegment> = arrangement
        .iter()
        .filter_map(|id| resolved.iter().find(|s| &s.section_id == id).cloned())
        .collect();
    if plan.is_empty() {
        resolved
    } else {
        plan
    }
}

/// Index of the plan segment where playback should start.
pub fn start_index(plan: &[PlanSegment], start_section_id: Option<&str>) -> usize {
    start_section_id
        .and_then(|id| plan.iter().position(|s| s.section_id == id))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs() -> Vec<Section> {
        let mk = |id: &str, label: &str, kind: &str, start: f64| Section {
            id: id.into(),
            label: label.into(),
            kind: kind.into(),
            start,
        };
        vec![
            mk("i", "Intro", "intro", 0.5),
            mk("v1", "Verse 1", "verse", 10.0),
            mk("c1", "Chorus", "chorus", 30.0),
            mk("v2", "Verse 2", "verse", 50.0),
            mk("b", "Bridge", "bridge", 70.0),
            mk("o", "Outro", "outro", 90.0),
        ]
    }

    #[test]
    fn linear_plan_covers_song() {
        let p = build_plan(&secs(), &[], 100.0);
        assert_eq!(p.len(), 6);
        assert_eq!(p[0].start, 0.0);
        assert_eq!(p[5].end, 100.0);
        assert_eq!(p[2].end, 50.0);
    }

    #[test]
    fn custom_arrangement_skips_and_repeats() {
        let arr: Vec<String> = ["i", "v1", "c1", "b", "c1", "o"].iter().map(|s| s.to_string()).collect();
        let p = build_plan(&secs(), &arr, 100.0);
        let labels: Vec<_> = p.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, vec!["Intro", "Verse 1", "Chorus", "Bridge", "Chorus", "Outro"]);
    }

    #[test]
    fn unknown_ids_fallback() {
        let p = build_plan(&secs(), &["nope".to_string()], 100.0);
        assert_eq!(p.len(), 6);
        assert_eq!(build_plan(&[], &[], 42.0)[0].end, 42.0);
        assert_eq!(start_index(&p, Some("b")), 4);
        assert_eq!(start_index(&p, Some("zzz")), 0);
    }
}
