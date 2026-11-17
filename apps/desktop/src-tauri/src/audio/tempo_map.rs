//! Tempo map. The same formulas live in `src/lib/tempoMap.ts` and
//! `audio-worker/.../tempo_map.py`. Time is linear; a slower tempo widens bars.
//! The audio is never stretched. One step point is a constant tempo.

use crate::audio::click::{is_bar_accent, BeatEntry, BeatGrid};
use crate::models::{Curve, Grid, TempoPoint};
#[cfg(test)]
use serde::Deserialize;

const FLAT: f64 = 1e-9;

struct Knot {
    q: f64,
    t: f64,
    bpm: f64,
    curve: Curve,
}

fn q_of(bar: f64, beat: f64, bpb: u32) -> f64 {
    let bpb = bpb.max(1) as f64;
    (bar - 1.0) * bpb + (beat - 1.0)
}

pub fn q_of_point(bar: u32, beat: u32, bpb: u32) -> f64 {
    q_of(bar.max(1) as f64, beat.max(1) as f64, bpb)
}

fn knots(grid: &Grid) -> Vec<Knot> {
    let bpb = grid.beats_per_bar.max(1);
    let mut raw: Vec<&TempoPoint> = grid.points.iter().collect();
    raw.sort_by(|a, b| {
        q_of_point(a.bar, a.beat, bpb)
            .partial_cmp(&q_of_point(b.bar, b.beat, bpb))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if raw.is_empty() {
        return vec![Knot { q: 0.0, t: grid.beat1_sec, bpm: 120.0, curve: Curve::Step }];
    }
    let mut out: Vec<Knot> = Vec::with_capacity(raw.len());
    let mut t = grid.beat1_sec;
    for (i, p) in raw.iter().enumerate() {
        let q = if i == 0 { 0.0 } else { q_of_point(p.bar, p.beat, bpb) };
        let bpm = p.bpm.max(1e-6);
        if i > 0 {
            let prev = &out[i - 1];
            t = time_across(prev.t, prev.q, prev.bpm, q, bpm, prev.curve);
        }
        out.push(Knot { q, t, bpm, curve: p.curve });
    }
    out
}

fn time_across(t0: f64, q0: f64, v0: f64, q1: f64, v1: f64, curve: Curve) -> f64 {
    let span = q1 - q0;
    if span <= 0.0 {
        return t0;
    }
    if curve != Curve::Linear || (v1 - v0).abs() < FLAT {
        return t0 + 60.0 * span / v0;
    }
    t0 + 60.0 * span / (v1 - v0) * (v1 / v0).ln()
}

fn segment(knots: &[Knot], q: f64) -> (usize, bool) {
    if q <= knots[0].q {
        return (0, false);
    }
    for i in 0..knots.len().saturating_sub(1) {
        if knots[i].q <= q && q < knots[i + 1].q {
            return (i, false);
        }
    }
    (knots.len() - 1, true)
}

pub fn bpm_at_beat(grid: &Grid, q: f64) -> f64 {
    let knots = knots(grid);
    let (i, past) = segment(&knots, q);
    let v0 = knots[i].bpm;
    if past || i + 1 >= knots.len() || knots[i].curve != Curve::Linear {
        return v0;
    }
    let v1 = knots[i + 1].bpm;
    let span = knots[i + 1].q - knots[i].q;
    if span <= 0.0 || (v1 - v0).abs() < FLAT {
        return v0;
    }
    v0 + (v1 - v0) * (q - knots[i].q) / span
}

pub fn beat_to_time(grid: &Grid, q: f64) -> f64 {
    let knots = knots(grid);
    let (i, past) = segment(&knots, q);
    let q0 = knots[i].q;
    let t0 = knots[i].t;
    let v0 = knots[i].bpm;
    if past || i + 1 >= knots.len() || q <= q0 {
        return t0 + 60.0 * (q - q0) / v0;
    }
    let v1 = knots[i + 1].bpm;
    let span = knots[i + 1].q - q0;
    if knots[i].curve != Curve::Linear || (v1 - v0).abs() < FLAT || span <= 0.0 {
        return t0 + 60.0 * (q - q0) / v0;
    }
    let bpm = v0 + (v1 - v0) * (q - q0) / span;
    if bpm <= 0.0 || v0 <= 0.0 {
        return t0 + 60.0 * (q - q0) / v0;
    }
    t0 + 60.0 * span / (v1 - v0) * (bpm / v0).ln()
}

pub fn time_to_beat(grid: &Grid, t: f64) -> f64 {
    let knots = knots(grid);
    if t <= knots[0].t {
        return knots[0].q + (t - knots[0].t) * knots[0].bpm / 60.0;
    }
    for i in 0..knots.len().saturating_sub(1) {
        let t0 = knots[i].t;
        let t1 = knots[i + 1].t;
        let last = i + 1 == knots.len() - 1;
        if t0 <= t && (t < t1 || (last && t <= t1)) {
            let q0 = knots[i].q;
            let q1 = knots[i + 1].q;
            let v0 = knots[i].bpm;
            let v1 = knots[i + 1].bpm;
            let span = q1 - q0;
            if t >= t1 {
                return q1;
            }
            if knots[i].curve != Curve::Linear || (v1 - v0).abs() < FLAT || span <= 0.0 {
                return q0 + (t - t0) * v0 / 60.0;
            }
            let bpm = v0 * ((t - t0) * (v1 - v0) / (60.0 * span)).exp();
            return q0 + span * (bpm - v0) / (v1 - v0);
        }
    }
    let last = knots.last().unwrap();
    last.q + (t - last.t) * last.bpm / 60.0
}

pub fn bpm_at_time(grid: &Grid, t: f64) -> f64 {
    bpm_at_beat(grid, time_to_beat(grid, t))
}

/// Click times for `[from_sec, to_sec]`, one entry per beat.
pub fn beat_table(grid: &Grid, from_sec: f64, to_sec: f64, sr: u32) -> Vec<BeatEntry> {
    let q0 = time_to_beat(grid, from_sec).floor() as i64 - 1;
    let q1 = time_to_beat(grid, to_sec).ceil() as i64 + 1;
    let bpb = grid.beats_per_bar.max(1);
    let mut out = Vec::new();
    for q in q0..=q1 {
        let t = beat_to_time(grid, q as f64);
        if t < from_sec - 1e-4 || t > to_sec + 1e-4 || t < 0.0 {
            continue;
        }
        let beat_in_bar = q.rem_euclid(bpb as i64);
        out.push(BeatEntry {
            frame: (t * sr as f64).round().max(0.0) as u64,
            is_accent: is_bar_accent(beat_in_bar, bpb),
            q: q as i32,
            bpm: bpm_at_beat(grid, q as f64) as f32,
        });
        if out.len() > 20_000 {
            break;
        }
    }
    out
}

/// Beat grid whose click positions come only from the tempo map.
pub fn to_beat_grid(grid: &Grid, sr: u32, duration: f64) -> Option<BeatGrid> {
    let bpm = grid.points.first().map(|p| p.bpm).unwrap_or(0.0);
    if !(20.0..=300.0).contains(&bpm) {
        return None;
    }
    let end = duration.max(grid.beat1_sec).max(1.0);
    let explicit = beat_table(grid, 0.0, end + 1.0, sr);
    Some(BeatGrid {
        frames_per_beat: sr as f64 * 60.0 / bpm,
        offset_frames: grid.beat1_sec * sr as f64,
        beats_per_bar: grid.beats_per_bar.max(1),
        explicit_beats: explicit,
    })
}

/// Frame at which the cue for a section should start: N beats before its downbeat.
pub fn cue_frame(grid: &Grid, section_start: f64, lead_beats: u32, sr: u32) -> u64 {
    let q = time_to_beat(grid, section_start);
    let t = beat_to_time(grid, q - lead_beats.max(1) as f64);
    (t.max(0.0) * sr as f64).round() as u64
}

/// Drop empty maps, keep points in order, and pin point 1 to Beat 1.
pub fn sanitize_grid(mut grid: Grid, beats_per_bar: u32, duration: f64) -> Option<Grid> {
    let bpb = beats_per_bar.max(1);
    grid.beats_per_bar = bpb;
    grid.version = 1;
    if grid.source != "analysis" {
        grid.source = "user".into();
    }
    grid.beat1_sec = grid.beat1_sec.max(0.0);
    if duration > 0.0 {
        grid.beat1_sec = grid.beat1_sec.min(duration);
    }
    grid.points.retain(|p| (20.0..=300.0).contains(&p.bpm));
    if grid.points.is_empty() {
        return None;
    }
    grid.points.sort_by(|a, b| {
        q_of_point(a.bar, a.beat, bpb)
            .partial_cmp(&q_of_point(b.bar, b.beat, bpb))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<TempoPoint> = Vec::new();
    for (i, mut p) in grid.points.into_iter().enumerate() {
        p.bpm = (p.bpm * 1000.0).round() / 1000.0;
        if i == 0 {
            p.bar = 1;
            p.beat = 1;
        } else {
            p.bar = p.bar.max(1);
            p.beat = p.beat.clamp(1, bpb);
            let q = q_of_point(p.bar, p.beat, bpb);
            let prev_q = if kept.len() == 1 {
                0.0
            } else {
                let prev = kept.last().unwrap();
                q_of_point(prev.bar, prev.beat, bpb)
            };
            if q < prev_q + 1.0 {
                continue;
            }
        }
        if p.id.is_empty() {
            p.id = format!("p{}", kept.len() + 1);
        }
        kept.push(p);
    }
    grid.points = kept;
    Some(grid)
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct FixtureFile {
    mockup: FixtureCase,
    constant: FixtureCase,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCase {
    beat1_sec: f64,
    beats_per_bar: u32,
    points: Vec<FixturePoint>,
    expect: Vec<FixtureExpect>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct FixturePoint {
    bar: u32,
    beat: u32,
    bpm: f64,
    curve: Curve,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureExpect {
    bar: u32,
    beat: u32,
    #[serde(default)]
    #[allow(dead_code)]
    q_neg: Option<i32>,
    time: f64,
    bpm: f64,
}

#[cfg(test)]
fn grid_from(case: &FixtureCase) -> Grid {
    Grid {
        beat1_sec: case.beat1_sec,
        beats_per_bar: case.beats_per_bar,
        points: case
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| TempoPoint {
                id: format!("p{i}"),
                bar: p.bar,
                beat: p.beat,
                bpm: p.bpm,
                curve: p.curve,
            })
            .collect(),
        source: "analysis".into(),
        confidence: None,
        version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> FixtureFile {
        serde_json::from_str(include_str!("../../../../../fixtures/tempo_map_vectors.json")).unwrap()
    }

    fn check(case: &FixtureCase) {
        let g = grid_from(case);
        for e in &case.expect {
            let q = if e.bar == 1 && e.beat == 1 && e.time < g.beat1_sec {
                -4.0
            } else {
                q_of_point(e.bar, e.beat, g.beats_per_bar)
            };
            let t = beat_to_time(&g, q);
            let bpm = bpm_at_beat(&g, q);
            assert!((t - e.time).abs() < 0.0001, "time {t} vs {} at {q}", e.time);
            assert!((bpm - e.bpm).abs() < 0.0001, "bpm {bpm} vs {}", e.bpm);
            let back = time_to_beat(&g, t);
            assert!((back - q).abs() < 1e-6, "inverse {back} vs {q}");
        }
    }

    #[test]
    fn golden_vectors_match_ts_and_python() {
        let f = fixture();
        check(&f.mockup);
        check(&f.constant);
    }

    #[test]
    fn mockup_bar_45_is_one_sample_at_48k() {
        let g = grid_from(&fixture().mockup);
        let t = beat_to_time(&g, q_of_point(45, 1, 4));
        let frame = (t * 48_000.0).round() as i64;
        assert!((frame - 7_187_585).abs() <= 1, "frame {frame}");
    }

    #[test]
    fn count_in_click_is_one_beat_before_beat_one() {
        let g = grid_from(&fixture().mockup);
        let beat = beat_to_time(&g, -1.0);
        let beat1 = beat_to_time(&g, 0.0);
        let period = 60.0 / 71.8;
        assert!((beat1 - beat - period).abs() < 0.0001);
    }

    #[test]
    fn six_eight_table_accents_beat_1_and_4() {
        let g = Grid::constant(0.0, 6, 90.0);
        let table = beat_table(&g, 0.0, 8.0, 48_000);
        let accents: Vec<i32> = table.iter().filter(|b| b.is_accent).map(|b| b.q).collect();
        assert!(accents.contains(&0) && accents.contains(&3) && accents.contains(&6));
        assert!(!table.iter().any(|b| b.q == 1 && b.is_accent));
    }
}
