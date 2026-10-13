//! Procedural click engine. Every sound is generated mathematically and deterministically,
//! so no downloaded samples are required for a usable click.
//!
//! The click walks a beat table built from the song's tempo map. Each entry carries a
//! frame, an accent, a beat index, and the BPM at that beat. Count-in and cues read the
//! same table, so a change on the tempo lane moves the click with it. Audio is never
//! time-stretched; the map only decides when the clicks fall.

use super::buffer::AudioBuffer;
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickSound {
    Classic,
    Woodblock,
    HiHat,
    Cowbell,
}

impl ClickSound {
    pub fn parse(s: &str) -> Self {
        match s {
            "woodblock" => Self::Woodblock,
            "hihat" => Self::HiHat,
            "cowbell" => Self::Cowbell,
            _ => Self::Classic,
        }
    }
    pub fn all() -> &'static [(&'static str, &'static str)] {
        &[
            ("classic", "Classic Beep"),
            ("woodblock", "Woodblock"),
            ("hihat", "Hi-Hat Tick"),
            ("cowbell", "Cowbell"),
        ]
    }
}

/// Deterministic white noise (xorshift), so renders are bit-identical across runs.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Render a single click (mono) at `sr`.
pub fn render_click(sound: ClickSound, accent: bool, sr: u32) -> Vec<f32> {
    let sr_f = sr as f32;
    let len = (sr_f * 0.06) as usize;
    let mut out = vec![0f32; len];
    let mut noise = Noise(0x9E37_79B9);
    match sound {
        ClickSound::Classic => {
            // A short tick, not a ringing beep. The old 60 ms sine read as a
            // constant tone sitting on top of the track.
            let f = if accent { 1800.0 } else { 1320.0 };
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / sr_f;
                let env = (-t * 260.0).exp() * (1.0 - (-t * 6000.0).exp());
                let tick = noise.next() * (-t * 1400.0).exp();
                *s = (TAU * f * t).sin() * env * 0.65 + tick * 0.35;
            }
        }
        ClickSound::Woodblock => {
            let f = if accent { 1250.0 } else { 880.0 };
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / sr_f;
                let env = (-t * 120.0).exp();
                let tone = (TAU * f * t).sin() * 0.8 + (TAU * f * 2.71 * t).sin() * 0.25;
                let click = noise.next() * (-t * 900.0).exp() * 0.4;
                *s = (tone * env + click) * 0.9;
            }
        }
        ClickSound::HiHat => {
            // high-passed noise via first-difference
            let mut prev = 0.0;
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / sr_f;
                let n = noise.next();
                let hp = n - prev;
                prev = n;
                let decay = if accent { 70.0 } else { 140.0 };
                *s = hp * (-t * decay).exp() * 0.6;
            }
        }
        ClickSound::Cowbell => {
            let (f1, f2) = if accent { (800.0, 1180.0) } else { (562.0, 845.0) };
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / sr_f;
                let env = (-t * 45.0).exp();
                let sq = |f: f32| if (TAU * f * t).sin() >= 0.0 { 1.0 } else { -1.0 };
                *s = (sq(f1) + sq(f2)) * 0.25 * env;
            }
        }
    }
    let gain = if accent { 1.0 } else { 0.72 };
    let peak = out.iter().fold(0f32, |m, s| m.max(s.abs())).max(1e-6);
    for s in out.iter_mut() {
        *s = *s / peak * 0.9 * gain;
    }
    out
}

/// Pre-rendered pair of click voices used by the mixer.
#[derive(Debug, Clone)]
pub struct ClickVoices {
    pub accent: Vec<f32>,
    pub beat: Vec<f32>,
}

impl ClickVoices {
    pub fn new(sound: ClickSound, sr: u32) -> Self {
        Self { accent: render_click(sound, true, sr), beat: render_click(sound, false, sr) }
    }
}

// ---------------------------------------------------------------------------
// Beat grid — supports both fixed BPM and explicit beat timestamps
// ---------------------------------------------------------------------------

/// An explicit beat entry from the Python analysis.
#[derive(Debug, Clone, Copy)]
pub struct BeatEntry {
    /// Frame position of this beat.
    pub frame: u64,
    /// Whether this beat is a downbeat (beat 1 of a bar, or beat 4 in 6/8).
    pub is_accent: bool,
    /// Beat index from Beat 1. Negative values are the count-in.
    pub q: i32,
    /// Instantaneous tempo at this beat, from the tempo map.
    pub bpm: f32,
}

/// A beat grid that can operate in two modes:
///
/// * **Fixed** — classic constant-tempo arithmetic.  Fast, zero allocation.
///   Used when the song is studio-recorded or the user has confirmed a fixed BPM.
///
/// * **Explicit** — an ordered array of `BeatEntry` produced by the Python beat
///   tracker.  Accurate for live recordings with tempo drift.  The mixer walks
///   forward through the array instead of computing the next beat from a formula.
#[derive(Debug, Clone)]
pub struct BeatGrid {
    /// Frames per beat (constant-tempo fallback / count-in).
    pub frames_per_beat: f64,
    /// Frame of the first downbeat (constant-tempo mode).
    pub offset_frames: f64,
    pub beats_per_bar: u32,

    /// Optional explicit beat table.  When present and non-empty, the mixer
    /// uses these positions instead of the formula.
    pub explicit_beats: Vec<BeatEntry>,
}

/// Beat 1 of every bar is the accent. In 6/8, beat 4 is accented too.
pub(crate) fn is_bar_accent(beat_in_bar: i64, beats_per_bar: u32) -> bool {
    beat_in_bar == 0 || (beats_per_bar == 6 && beat_in_bar == 3)
}

impl BeatGrid {
    /// Build a fixed-tempo grid (legacy path, also used for count-in generation).
    pub fn new(bpm: f64, offset_secs: f64, beats_per_bar: u32, sr: u32) -> Option<Self> {
        if !(20.0..=300.0).contains(&bpm) {
            return None;
        }
        Some(Self {
            frames_per_beat: sr as f64 * 60.0 / bpm,
            offset_frames: offset_secs * sr as f64,
            beats_per_bar: beats_per_bar.max(1),
            explicit_beats: vec![],
        })
    }

    /// Build a grid from explicit beat timestamps (seconds) produced by the Python
    /// beat tracker.  The first downbeat is at `offset_secs`.  `bpm` and the fixed
    /// params are still computed for count-in and UI display; the explicit array
    /// drives the actual click scheduling.
    pub fn from_beats(
        beat_times: &[f64],
        offset_secs: f64,
        beats_per_bar: u32,
        bpm_hint: f64,
        sr: u32,
    ) -> Option<Self> {
        if beat_times.len() < 4 {
            return None;
        }
        let bpb = beats_per_bar.max(1);

        // Compute median BPM from the explicit beat array for fixed-mode fallback.
        let intervals: Vec<f64> = beat_times.windows(2).map(|w| w[1] - w[0]).collect();
        let mut sorted = intervals.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_interval = sorted[sorted.len() / 2];
        let effective_bpm = if bpm_hint > 20.0 && bpm_hint < 300.0 {
            bpm_hint
        } else {
            60.0 / median_interval
        };

        // Find the first downbeat beat index: the tracked beat closest to offset_secs.
        let offset_idx = beat_times
            .iter()
            .enumerate()
            .min_by_key(|(_, &t)| ((t - offset_secs).abs() * 1e6) as i64)
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Build the explicit beat table with correct accent assignment.
        let explicit_beats: Vec<BeatEntry> = beat_times
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let beat_from_downbeat = i as i64 - offset_idx as i64;
                // Accent beat 1 of the bar. 6/8 also accents beat 4, the second group of three.
                let is_accent = is_bar_accent(beat_from_downbeat.rem_euclid(bpb as i64), bpb);
                BeatEntry {
                    frame: (t * sr as f64).round() as u64,
                    is_accent,
                    q: beat_from_downbeat as i32,
                    bpm: effective_bpm as f32,
                }
            })
            .collect();

        Some(Self {
            frames_per_beat: sr as f64 * 60.0 / effective_bpm,
            offset_frames: offset_secs * sr as f64,
            beats_per_bar: bpb,
            explicit_beats,
        })
    }

    /// Whether this grid has explicit beat data (and therefore follows the music
    /// rather than a fixed formula).
    pub fn has_explicit_beats(&self) -> bool {
        !self.explicit_beats.is_empty()
    }

    /// Frame where the band's beat 1 sits. Count-in ends on this frame so the
    /// next click is the downbeat, not a click somewhere in the lead-in.
    pub fn first_downbeat_frame(&self) -> u64 {
        if let Some(b) = self.explicit_beats.iter().find(|b| b.q == 0) {
            return b.frame;
        }
        if let Some(b) = self.explicit_beats.iter().find(|b| b.is_accent) {
            return b.frame;
        }
        self.offset_frames.round().max(0.0) as u64
    }

    /// Local tempo at `frame`, taken from the tempo-map beat table.
    pub fn bpm_at_frame(&self, frame: u64) -> Option<f64> {
        if self.explicit_beats.is_empty() {
            return None;
        }
        let i = self.explicit_beat_at_or_after(frame);
        let entry = if i == 0 {
            &self.explicit_beats[0]
        } else if i >= self.explicit_beats.len() {
            self.explicit_beats.last().unwrap()
        } else {
            let prev = &self.explicit_beats[i - 1];
            let next = &self.explicit_beats[i];
            if frame - prev.frame <= next.frame - frame { prev } else { next }
        };
        if entry.bpm > 1.0 { Some(entry.bpm as f64) } else { None }
    }

    // -----------------------------------------------------------------------
    // Fixed-tempo arithmetic (used for count-in and as fallback)
    // -----------------------------------------------------------------------

    /// Index of the first beat at or after `frame` (fixed-tempo arithmetic).
    pub fn beat_at_or_after(&self, frame: u64) -> i64 {
        ((frame as f64 - self.offset_frames) / self.frames_per_beat).ceil() as i64
    }

    pub fn beat_frame(&self, index: i64) -> f64 {
        self.offset_frames + index as f64 * self.frames_per_beat
    }

    pub fn is_accent_fixed(&self, index: i64) -> bool {
        let bpb = self.beats_per_bar.max(1);
        is_bar_accent(index.rem_euclid(bpb as i64), bpb)
    }

    // -----------------------------------------------------------------------
    // Explicit-beat lookup (used by the mixer when explicit_beats is non-empty)
    // -----------------------------------------------------------------------

    /// Return the index into `explicit_beats` of the first beat at or after `frame`.
    /// Returns `explicit_beats.len()` if all beats are before `frame`.
    pub fn explicit_beat_at_or_after(&self, frame: u64) -> usize {
        match self.explicit_beats.partition_point(|b| b.frame < frame) {
            i => i,
        }
    }
}

// ---------------------------------------------------------------------------
// Offline click track render (used for tests, previews)
// ---------------------------------------------------------------------------

/// Render a click track using explicit beats when available, fixed formula otherwise.
pub fn render_click_track(grid: &BeatGrid, frames: u64, voices: &ClickVoices, sr: u32) -> AudioBuffer {
    let mut mono = vec![0f32; frames as usize];

    if grid.has_explicit_beats() {
        // Explicit mode: iterate the beat table.
        for entry in &grid.explicit_beats {
            if entry.frame >= frames {
                break;
            }
            let v = if entry.is_accent { &voices.accent } else { &voices.beat };
            for (i, &s) in v.iter().enumerate() {
                let idx = entry.frame as usize + i;
                if idx >= mono.len() {
                    break;
                }
                mono[idx] += s;
            }
        }
    } else {
        // Fixed-tempo mode (original algorithm).
        let mut k = grid.beat_at_or_after(0);
        loop {
            let start = grid.beat_frame(k).round() as i64;
            if start >= frames as i64 {
                break;
            }
            if start >= 0 {
                let v = if grid.is_accent_fixed(k) { &voices.accent } else { &voices.beat };
                for (i, s) in v.iter().enumerate() {
                    let idx = start as usize + i;
                    if idx >= mono.len() {
                        break;
                    }
                    mono[idx] += *s;
                }
            }
            k += 1;
        }
    }

    AudioBuffer::from_mono(&mono, sr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_is_deterministic() {
        for (id, _) in ClickSound::all() {
            let s = ClickSound::parse(id);
            assert_eq!(render_click(s, true, 48000), render_click(s, true, 48000));
            assert!(render_click(s, false, 48000).iter().all(|v| v.abs() <= 1.0));
        }
    }

    #[test]
    fn grid_math_fixed() {
        let g = BeatGrid::new(120.0, 0.25, 4, 48000).unwrap();
        assert_eq!(g.frames_per_beat, 24000.0);
        assert_eq!(g.beat_at_or_after(0), 0);
        assert_eq!(g.beat_at_or_after(12001), 1);
        assert!(g.is_accent_fixed(0) && g.is_accent_fixed(4) && !g.is_accent_fixed(1) && g.is_accent_fixed(-4));
        assert!(BeatGrid::new(5.0, 0.0, 4, 48000).is_none());
    }

    #[test]
    fn six_eight_accents_both_groups() {
        let g = BeatGrid::new(180.0, 0.0, 6, 48000).unwrap();
        assert!(g.is_accent_fixed(0));
        assert!(!g.is_accent_fixed(1) && !g.is_accent_fixed(2));
        assert!(g.is_accent_fixed(3));
        assert!(!g.is_accent_fixed(4) && !g.is_accent_fixed(5));
        assert!(g.is_accent_fixed(6));
    }

    #[test]
    fn explicit_beats_accent_assignment() {
        // 4 beats at 120 BPM, downbeat at beat 0
        let times = vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5];
        let g = BeatGrid::from_beats(&times, 0.0, 4, 120.0, 48000).unwrap();
        assert!(g.has_explicit_beats());
        // beats 0, 4 should be accents (downbeats); beats 1,2,3 should not
        assert!(g.explicit_beats[0].is_accent);
        assert!(!g.explicit_beats[1].is_accent);
        assert!(!g.explicit_beats[2].is_accent);
        assert!(!g.explicit_beats[3].is_accent);
        assert!(g.explicit_beats[4].is_accent);
    }

    #[test]
    fn explicit_beats_offset_downbeat() {
        // downbeat starts at beat index 2 (t=1.0s), not at t=0
        let times = vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5];
        let g = BeatGrid::from_beats(&times, 1.0, 4, 120.0, 48000).unwrap();
        assert_eq!(g.first_downbeat_frame(), 48_000);
        // beat at idx 2 is the downbeat
        assert!(g.explicit_beats[2].is_accent);
        assert!(!g.explicit_beats[3].is_accent);
        // beat at idx 6 (2 + 4) is next downbeat
        assert!(g.explicit_beats[6].is_accent);
    }

    #[test]
    fn click_track_has_beats_at_grid_positions() {
        let sr = 48000;
        let g = BeatGrid::new(120.0, 0.0, 4, sr).unwrap();
        let v = ClickVoices::new(ClickSound::Classic, sr);
        let t = render_click_track(&g, sr as u64 * 2, &v, sr);
        // energy right after beat 1 (0.5 s) and silence just before it
        let (a, _) = t.frame(24000 + 100);
        let (b, _) = t.frame(24000 - 100);
        assert!(a.abs() > 0.01);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn explicit_beat_lookup() {
        let times = vec![0.0, 0.5, 1.0, 1.5];
        let g = BeatGrid::from_beats(&times, 0.0, 4, 120.0, 48000).unwrap();
        // frame 24000 = 0.5 s → beat index 1
        assert_eq!(g.explicit_beat_at_or_after(24000), 1);
        assert_eq!(g.explicit_beat_at_or_after(0), 0);
    }
}
