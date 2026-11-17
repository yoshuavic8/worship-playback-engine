//! Native audio host: owns the CoreAudio (via CPAL) output stream on a dedicated thread.
//! The UI and control code only ever talk to the mixer through a lock-free command channel.

use super::click::{ClickSound, ClickVoices};
use super::mixer::{BusMap, LoadedSong, MixLevels, Mixer, MixerCmd, SharedStatus};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, unbounded, Sender};
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub name: String,
    pub channels: u16,
    pub sample_rates: Vec<u32>,
    pub default_sample_rate: u32,
    pub is_default: bool,
    pub buffer_min: Option<u32>,
    pub buffer_max: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningInfo {
    pub device_name: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub buffer_size: Option<u32>,
    pub latency_ms: Option<f32>,
}

const STANDARD_RATES: [u32; 5] = [44100, 48000, 88200, 96000, 192000];

pub fn list_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut out = vec![];
    let Ok(devices) = host.output_devices() else { return out };
    for d in devices {
        let Ok(name) = d.name() else { continue };
        let mut channels = 0u16;
        let mut rates = vec![];
        let mut bmin = None;
        let mut bmax = None;
        if let Ok(cfgs) = d.supported_output_configs() {
            for c in cfgs {
                channels = channels.max(c.channels());
                for r in STANDARD_RATES {
                    if c.min_sample_rate().0 <= r && r <= c.max_sample_rate().0 && !rates.contains(&r) {
                        rates.push(r);
                    }
                }
                if let cpal::SupportedBufferSize::Range { min, max } = c.buffer_size() {
                    bmin = Some(bmin.map_or(*min, |m: u32| m.min(*min)));
                    bmax = Some(bmax.map_or(*max, |m: u32| m.max(*max)));
                }
            }
        }
        if channels == 0 {
            continue;
        }
        rates.sort();
        let default_sample_rate = d.default_output_config().map(|c| c.sample_rate().0).unwrap_or(48000);
        out.push(DeviceInfo {
            is_default: Some(&name) == default_name.as_ref(),
            name,
            channels,
            sample_rates: rates,
            default_sample_rate,
            buffer_min: bmin,
            buffer_max: bmax,
        });
    }
    out
}

enum HostCmd {
    Open {
        device: Option<String>,
        sample_rate: u32,
        buffer_size: u32,
        mixer: Box<dyn FnOnce(u32) -> Mixer + Send>,
        reply: Sender<Result<RunningInfo, String>>,
    },
    Close,
}

fn open_stream(
    device: Option<String>,
    sample_rate: u32,
    buffer_size: u32,
    make_mixer: Box<dyn FnOnce(u32) -> Mixer + Send>,
    status: Arc<SharedStatus>,
) -> Result<(cpal::Stream, RunningInfo)> {
    let host = cpal::default_host();
    let dev = match &device {
        Some(n) => host
            .output_devices()?
            .find(|d| d.name().map(|x| &x == n).unwrap_or(false))
            .ok_or_else(|| anyhow!("Audio device \"{n}\" is not connected"))?,
        None => host.default_output_device().ok_or_else(|| anyhow!("No audio output device found"))?,
    };
    let name = dev.name().unwrap_or_else(|_| "Unknown".into());
    let mut best: Option<cpal::SupportedStreamConfigRange> = None;
    for c in dev.supported_output_configs()? {
        if c.sample_format() != cpal::SampleFormat::F32 {
            continue;
        }
        if c.min_sample_rate().0 <= sample_rate && sample_rate <= c.max_sample_rate().0 {
            if best.as_ref().map(|b| c.channels() > b.channels()).unwrap_or(true) {
                best = Some(c);
            }
        }
    }
    let (cfg_range, rate) = match best {
        Some(b) => (b, sample_rate),
        None => {
            let d = dev.default_output_config()?;
            let rate = d.sample_rate().0;
            let range = dev
                .supported_output_configs()?
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
                .filter(|c| c.min_sample_rate().0 <= rate && rate <= c.max_sample_rate().0)
                .max_by_key(|c| c.channels())
                .ok_or_else(|| anyhow!("Device has no 32-bit float output configuration"))?;
            (range, rate)
        }
    };
    let channels = cfg_range.channels();
    let buf = match cfg_range.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            Some(buffer_size.clamp(*min, *max))
        }
        cpal::SupportedBufferSize::Unknown => None,
    };
    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(rate),
        buffer_size: match buf {
            Some(b) => cpal::BufferSize::Fixed(b),
            None => cpal::BufferSize::Default,
        },
    };
    status.sample_rate.store(rate, Ordering::Relaxed);
    status.stream_error.store(false, Ordering::Relaxed);
    let mut mixer = make_mixer(rate);
    let ch = channels as usize;
    let err_status = status.clone();
    let stream = dev.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| mixer.render(data, ch),
        move |err| {
            eprintln!("[audio] stream error: {err}");
            err_status.stream_error.store(true, Ordering::Relaxed);
        },
        None,
    )?;
    stream.play()?;
    Ok((
        stream,
        RunningInfo {
            device_name: name,
            channels,
            sample_rate: rate,
            buffer_size: buf,
            latency_ms: buf.map(|b| b as f32 / rate as f32 * 1000.0),
        },
    ))
}

struct Inner {
    mixer_tx: Option<Sender<MixerCmd>>,
    running: Option<RunningInfo>,
    bus: BusMap,
    levels: MixLevels,
    click_sound: ClickSound,
    last_error: Option<String>,
}

pub struct AudioEngine {
    host_tx: Sender<HostCmd>,
    garbage_tx: Sender<Box<LoadedSong>>,
    pub status: Arc<SharedStatus>,
    inner: Mutex<Inner>,
}

impl AudioEngine {
    pub fn new(bus: BusMap, levels: MixLevels, click_sound: ClickSound) -> Self {
        let status = Arc::new(SharedStatus::default());
        let (host_tx, host_rx) = unbounded::<HostCmd>();
        let host_status = status.clone();
        std::thread::Builder::new()
            .name("wpe-audio-host".into())
            .spawn(move || {
                let mut stream: Option<cpal::Stream> = None;
                while let Ok(cmd) = host_rx.recv() {
                    match cmd {
                        HostCmd::Open { device, sample_rate, buffer_size, mixer, reply } => {
                            stream = None;
                            match open_stream(device, sample_rate, buffer_size, mixer, host_status.clone()) {
                                Ok((s, info)) => {
                                    stream = Some(s);
                                    let _ = reply.send(Ok(info));
                                }
                                Err(e) => {
                                    let _ = reply.send(Err(e.to_string()));
                                }
                            }
                        }
                        HostCmd::Close => stream = None,
                    }
                }
                drop(stream);
            })
            .expect("spawn audio host");
        // Buffers released by the mixer are dropped here, never on the audio thread.
        let (garbage_tx, garbage_rx) = bounded::<Box<LoadedSong>>(16);
        std::thread::Builder::new()
            .name("wpe-audio-gc".into())
            .spawn(move || while let Ok(g) = garbage_rx.recv() { drop(g) })
            .expect("spawn gc");
        Self {
            host_tx,
            garbage_tx,
            status,
            inner: Mutex::new(Inner {
                mixer_tx: None,
                running: None,
                bus,
                levels,
                click_sound,
                last_error: None,
            }),
        }
    }

    pub fn open(&self, device: Option<String>, sample_rate: u32, buffer_size: u32) -> Result<RunningInfo> {
        let (bus, levels, sound) = {
            let i = self.inner.lock();
            (i.bus.clone(), i.levels, i.click_sound)
        };
        let (tx, rx) = unbounded::<MixerCmd>();
        let garbage = self.garbage_tx.clone();
        let status = self.status.clone();
        let make = Box::new(move |rate: u32| {
            Mixer::new(rx, garbage, status, ClickVoices::new(sound, rate), bus, levels)
        });
        let (rtx, rrx) = bounded(1);
        self.host_tx
            .send(HostCmd::Open { device, sample_rate, buffer_size, mixer: make, reply: rtx })
            .map_err(|_| anyhow!("audio host stopped"))?;
        let res = rrx.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| anyhow!("audio device timeout"))?;
        let mut i = self.inner.lock();
        match res {
            Ok(info) => {
                i.mixer_tx = Some(tx);
                i.running = Some(info.clone());
                i.last_error = None;
                Ok(info)
            }
            Err(e) => {
                i.mixer_tx = None;
                i.running = None;
                i.last_error = Some(e.clone());
                Err(anyhow!(e))
            }
        }
    }

    pub fn close(&self) {
        let _ = self.host_tx.send(HostCmd::Close);
        let mut i = self.inner.lock();
        i.mixer_tx = None;
        i.running = None;
    }

    pub fn send(&self, cmd: MixerCmd) -> bool {
        let i = self.inner.lock();
        match &i.mixer_tx {
            Some(tx) => tx.send(cmd).is_ok(),
            None => false,
        }
    }

    pub fn running(&self) -> Option<RunningInfo> {
        self.inner.lock().running.clone()
    }

    pub fn last_error(&self) -> Option<String> {
        self.inner.lock().last_error.clone()
    }

    pub fn mark_failed(&self, err: String) {
        let mut i = self.inner.lock();
        i.running = None;
        i.mixer_tx = None;
        i.last_error = Some(err);
    }

    pub fn sample_rate(&self) -> u32 {
        self.inner.lock().running.as_ref().map(|r| r.sample_rate).unwrap_or(48000)
    }

    pub fn set_bus(&self, bus: BusMap) {
        self.inner.lock().bus = bus.clone();
        self.send(MixerCmd::SetBusMap(bus));
    }

    pub fn bus(&self) -> BusMap {
        self.inner.lock().bus.clone()
    }

    pub fn set_levels(&self, levels: MixLevels) {
        self.inner.lock().levels = levels;
        self.send(MixerCmd::SetLevels(levels));
    }

    pub fn levels(&self) -> MixLevels {
        self.inner.lock().levels
    }

    pub fn set_click_sound(&self, sound: ClickSound) {
        let rate = self.sample_rate();
        self.inner.lock().click_sound = sound;
        self.send(MixerCmd::SetClickVoices(Box::new(ClickVoices::new(sound, rate))));
    }
}
