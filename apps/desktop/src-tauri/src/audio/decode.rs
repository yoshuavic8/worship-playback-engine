//! File decoding (Symphonia) and sample-rate conversion (Rubato).
//! Decoding always happens off the audio thread.

use super::buffer::AudioBuffer;
use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "m4a", "aac", "aif", "aiff", "ogg"];

pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct ProbeInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub duration: f64,
}

/// Decode an entire file into interleaved stereo at its native sample rate.
pub fn decode_file(path: &Path) -> Result<(AudioBuffer, ProbeInfo)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| anyhow!("Unsupported or corrupt audio file ({e})"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("No audio track found"))?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(48000);
    let src_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u32;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow!("No decoder for this format ({e})"))?;

    let mut out: Vec<f32> = Vec::new();
    if let Some(n) = track.codec_params.n_frames {
        out.reserve((n as usize).saturating_mul(2));
    }
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    let mut sbuf_frames: u64 = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(anyhow!("Read error: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let ch = spec.channels.count().max(1);
                let cap = decoded.capacity() as u64;
                if sbuf.is_none() || sbuf_frames < cap {
                    sbuf = Some(SampleBuffer::<f32>::new(cap, spec));
                    sbuf_frames = cap;
                }
                let b = sbuf.as_mut().unwrap();
                b.copy_interleaved_ref(decoded);
                for frame in b.samples().chunks_exact(ch) {
                    let l = frame[0];
                    let r = if ch > 1 { frame[1] } else { l };
                    out.push(l);
                    out.push(r);
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(anyhow!("Decode error: {e}")),
        }
    }
    if out.is_empty() {
        return Err(anyhow!("The file contains no decodable audio"));
    }
    let buf = AudioBuffer::new(out, sample_rate);
    let info = ProbeInfo { sample_rate, channels: src_channels, duration: buf.duration() };
    Ok((buf, info))
}

/// Decode and convert to `target_rate` if needed.
pub fn load_at_rate(path: &Path, target_rate: u32) -> Result<AudioBuffer> {
    let (buf, _) = decode_file(path)?;
    resample(&buf, target_rate)
}

pub fn resample(buf: &AudioBuffer, target_rate: u32) -> Result<AudioBuffer> {
    use rubato::{FftFixedIn, Resampler};
    if buf.sample_rate == target_rate || buf.data.is_empty() {
        return Ok(AudioBuffer::new(buf.data.clone(), target_rate));
    }
    let frames = buf.frames() as usize;
    let mut l = Vec::with_capacity(frames);
    let mut r = Vec::with_capacity(frames);
    for f in buf.data.chunks_exact(2) {
        l.push(f[0]);
        r.push(f[1]);
    }
    let chunk = 4096;
    let mut rs = FftFixedIn::<f32>::new(buf.sample_rate as usize, target_rate as usize, chunk, 2, 2)
        .map_err(|e| anyhow!("resampler: {e}"))?;
    let delay = rs.output_delay();
    let expected = (frames as f64 * target_rate as f64 / buf.sample_rate as f64).round() as usize;
    let mut ol: Vec<f32> = Vec::with_capacity(expected + chunk);
    let mut or: Vec<f32> = Vec::with_capacity(expected + chunk);
    let mut pos = 0;
    while pos < frames {
        let need = rs.input_frames_next();
        let out = if pos + need <= frames {
            let o = rs.process(&[&l[pos..pos + need], &r[pos..pos + need]], None);
            pos += need;
            o
        } else {
            let o = rs.process_partial(Some(&[&l[pos..], &r[pos..]]), None);
            pos = frames;
            o
        }
        .map_err(|e| anyhow!("resample: {e}"))?;
        ol.extend_from_slice(&out[0]);
        or.extend_from_slice(&out[1]);
    }
    // Flush the resampler's internal delay line.
    while ol.len() < expected + delay {
        let out = rs.process_partial::<&[f32]>(None, None).map_err(|e| anyhow!("resample: {e}"))?;
        if out[0].is_empty() {
            break;
        }
        ol.extend_from_slice(&out[0]);
        or.extend_from_slice(&out[1]);
    }
    let start = delay.min(ol.len());
    let end = (start + expected).min(ol.len());
    let mut data = Vec::with_capacity((end - start) * 2);
    for i in start..end {
        data.push(ol[i]);
        data.push(or[i]);
    }
    Ok(AudioBuffer::new(data, target_rate))
}

/// Lightweight peak data for waveform display (stored once in analysis/waveform.json).
pub fn waveform_peaks(buf: &AudioBuffer, peaks_per_second: u32) -> serde_json::Value {
    let frames = buf.frames() as usize;
    let per = (buf.sample_rate / peaks_per_second.max(1)).max(1) as usize;
    let n = frames.div_ceil(per);
    let mut peaks: Vec<u8> = Vec::with_capacity(n);
    let mut rms: Vec<u8> = Vec::with_capacity(n);
    for i in 0..n {
        let s = i * per;
        let e = ((i + 1) * per).min(frames);
        let mut m = 0f32;
        let mut sq = 0f64;
        for f in s..e {
            let a = buf.data[f * 2].abs().max(buf.data[f * 2 + 1].abs());
            m = m.max(a);
            sq += (a as f64) * (a as f64);
        }
        let r = (sq / (e - s).max(1) as f64).sqrt() as f32;
        peaks.push((m.min(1.0) * 255.0).round() as u8);
        rms.push((r.min(1.0) * 255.0).round() as u8);
    }
    serde_json::json!({
        "version": 1,
        "peaksPerSecond": peaks_per_second,
        "duration": buf.duration(),
        "peaks": peaks,
        "rms": rms,
    })
}

pub fn write_wav_f32(path: &Path, buf: &AudioBuffer) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: buf.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for s in &buf.data {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, sr: u32, secs: f32) -> AudioBuffer {
        let n = (sr as f32 * secs) as usize;
        let mono: Vec<f32> =
            (0..n).map(|i| (i as f32 / sr as f32 * freq * std::f32::consts::TAU).sin() * 0.5).collect();
        AudioBuffer::from_mono(&mono, sr)
    }

    #[test]
    fn resample_preserves_duration() {
        let b = sine(440.0, 44100, 2.0);
        let r = resample(&b, 48000).unwrap();
        assert_eq!(r.sample_rate, 48000);
        assert!((r.duration() - 2.0).abs() < 0.01, "duration {}", r.duration());
        // amplitude roughly preserved in the middle
        let peak = r.data[48000..96000].iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.05, "peak {peak}");
    }

    #[test]
    fn wav_roundtrip_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        write_wav_f32(&p, &sine(220.0, 48000, 0.5)).unwrap();
        assert!(is_supported_audio(&p));
        assert!(!is_supported_audio(Path::new("x.txt")));
        let (b, info) = decode_file(&p).unwrap();
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(b.frames(), 24000);
        let w = waveform_peaks(&b, 50);
        assert_eq!(w["peaks"].as_array().unwrap().len(), 25);
    }

    #[test]
    fn corrupt_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.mp3");
        std::fs::write(&p, b"definitely not audio").unwrap();
        assert!(decode_file(&p).is_err());
    }
}
