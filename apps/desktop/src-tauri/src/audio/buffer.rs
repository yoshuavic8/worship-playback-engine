/// Interleaved stereo f32 audio held fully in memory for glitch-free playback.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub data: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(data: Vec<f32>, sample_rate: u32) -> Self {
        Self { data, sample_rate }
    }

    pub fn silent(frames: usize, sample_rate: u32) -> Self {
        Self { data: vec![0.0; frames * 2], sample_rate }
    }

    #[inline]
    pub fn frames(&self) -> u64 {
        (self.data.len() / 2) as u64
    }

    #[inline]
    pub fn frame(&self, i: u64) -> (f32, f32) {
        let idx = (i as usize) * 2;
        if idx + 1 < self.data.len() {
            (self.data[idx], self.data[idx + 1])
        } else {
            (0.0, 0.0)
        }
    }

    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }

    pub fn bytes(&self) -> usize {
        self.data.len() * 4
    }

    pub fn from_mono(mono: &[f32], sample_rate: u32) -> Self {
        let mut data = Vec::with_capacity(mono.len() * 2);
        for &s in mono {
            data.push(s);
            data.push(s);
        }
        Self { data, sample_rate }
    }

    /// Spoken-cue prep: drop the leading silence TTS puts before a word (100-230 ms
    /// with `say`) so the word starts on the frame it is triggered, keeping a 2 ms
    /// pre-roll. With `max_frames`, the tail is cut with a 10 ms fade so a word
    /// never runs into the next beat.
    pub fn trimmed_onset(&self, max_frames: Option<usize>) -> Self {
        let n = self.data.len() / 2;
        let peak = self.data.iter().fold(0f32, |m, v| m.max(v.abs()));
        if n == 0 || peak <= 0.0 {
            return self.clone();
        }
        let thr = peak * 0.05;
        let onset = (0..n).find(|&i| self.data[2 * i].abs().max(self.data[2 * i + 1].abs()) > thr).unwrap_or(0);
        let pre = (self.sample_rate as usize / 500).min(onset);
        let start = onset - pre;
        let mut data = self.data[start * 2..].to_vec();
        if let Some(m) = max_frames {
            let m = m.max(1);
            if data.len() / 2 > m {
                data.truncate(m * 2);
                let fade = (self.sample_rate as usize / 100).min(m);
                for k in 0..fade {
                    let g = (fade - k) as f32 / fade as f32;
                    let i = m - fade + k;
                    data[2 * i] *= g;
                    data[2 * i + 1] *= g;
                }
            }
        }
        Self { data, sample_rate: self.sample_rate }
    }
}

#[cfg(test)]
mod trim_tests {
    use super::*;

    #[test]
    fn trims_leading_silence_and_caps_length() {
        let sr = 48000;
        let mut m = vec![0.0f32; 9600]; // 200 ms silence
        m.extend(vec![0.5f32; 48000]);
        let b = AudioBuffer::from_mono(&m, sr).trimmed_onset(Some(24000));
        let first = (0..b.frames()).find(|&i| b.frame(i).0.abs() > 0.025).unwrap();
        assert!(first <= 96 + 1, "onset at {first} frames");
        assert_eq!(b.frames(), 24000);
        assert!(b.frame(23999).0.abs() < 0.01);
    }
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -96.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}
