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
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -96.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}
