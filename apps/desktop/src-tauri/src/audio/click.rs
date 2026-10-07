//! Procedural click engine. Every sound is generated mathematically and deterministically,
//! so no downloaded samples are required for a usable click.

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
            let f = if accent { 1760.0 } else { 1175.0 };
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / sr_f;
                let env = (-t * 90.0).exp() * (1.0 - (-t * 4000.0).exp());
                *s = (TAU * f * t).sin() * env;
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

/// A constant-tempo beat grid anchored at `offset_frames` (the first downbeat).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    pub frames_per_beat: f64,
    pub offset_frames: f64,
    pub beats_per_bar: u32,
}

impl BeatGrid {
    pub fn new(bpm: f64, offset_secs: f64, beats_per_bar: u32, sr: u32) -> Option<Self> {
        if !(20.0..=300.0).contains(&bpm) {
            return None;
        }
        Some(Self {
            frames_per_beat: sr as f64 * 60.0 / bpm,
            offset_frames: offset_secs * sr as f64,
            beats_per_bar: beats_per_bar.max(1),
        })
    }

    /// Index of the first beat at or after `frame`.
    pub fn beat_at_or_after(&self, frame: u64) -> i64 {
        ((frame as f64 - self.offset_frames) / self.frames_per_beat).ceil() as i64
    }

    pub fn beat_frame(&self, index: i64) -> f64 {
        self.offset_frames + index as f64 * self.frames_per_beat
    }

    pub fn is_accent(&self, index: i64) -> bool {
        index.rem_euclid(self.beats_per_bar as i64) == 0
    }
}

/// Offline click track render (used for tests, previews and exports).
pub fn render_click_track(grid: &BeatGrid, frames: u64, voices: &ClickVoices, sr: u32) -> AudioBuffer {
    let mut mono = vec![0f32; frames as usize];
    let mut k = grid.beat_at_or_after(0);
    loop {
        let start = grid.beat_frame(k).round() as i64;
        if start >= frames as i64 {
            break;
        }
        if start >= 0 {
            let v = if grid.is_accent(k) { &voices.accent } else { &voices.beat };
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
    fn grid_math() {
        let g = BeatGrid::new(120.0, 0.25, 4, 48000).unwrap();
        assert_eq!(g.frames_per_beat, 24000.0);
        assert_eq!(g.beat_at_or_after(0), 0);
        assert_eq!(g.beat_at_or_after(12001), 1);
        assert!(g.is_accent(0) && g.is_accent(4) && !g.is_accent(1) && g.is_accent(-4));
        assert!(BeatGrid::new(5.0, 0.0, 4, 48000).is_none());
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
}
