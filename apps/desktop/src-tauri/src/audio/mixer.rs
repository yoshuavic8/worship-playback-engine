//! Real-time mixer running inside the audio callback.
//!
//! Signal flow (bus based, never hard-coded L/R):
//!
//!   tracks (backing / stems) ──► FOH bus      ──► OutputTarget (device channels)
//!                          └──► MONITOR bus  ──► OutputTarget
//!   click ───────────────────► MONITOR bus only
//!   cue   ───────────────────► MONITOR bus only
//!
//! Click and cue have no code path into the FOH accumulator. This is structural, not a setting.
//! The callback never allocates, locks or blocks; commands arrive through a lock-free channel.

use super::buffer::AudioBuffer;
use super::click::{BeatGrid, ClickVoices};
use crate::models::OutputTarget;
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackRole {
    Backing,
    Backup,
    Original,
    Vocals,
    Drums,
    Bass,
    Other,
}

pub struct Track {
    pub role: TrackRole,
    pub buf: Arc<AudioBuffer>,
    pub gain: f32,
    pub muted: bool,
    pub to_foh: bool,
    pub to_mon: bool,
    cur: f32,
}

impl Track {
    pub fn new(role: TrackRole, buf: Arc<AudioBuffer>, gain: f32, to_foh: bool, to_mon: bool) -> Self {
        Self { role, buf, gain, muted: false, to_foh, to_mon, cur: gain }
    }
}

pub struct Segment {
    pub start: u64,
    pub end: u64,
    /// Pre-rendered spoken cue for this segment (played before the segment begins).
    pub cue: Option<Arc<AudioBuffer>>,
    /// Song frame at which this segment's cue should start (N beats before it, from the tempo map).
    pub cue_at: Option<u64>,
}

pub struct LoadedSong {
    pub generation: u64,
    pub tracks: Vec<Track>,
    pub frames: u64,
    pub segments: Vec<Segment>,
    pub start_segment: usize,
    pub grid: Option<BeatGrid>,
    pub click_enabled: bool,
    pub cue_enabled: bool,
    pub count_in_beats: u32,
    pub cue_lead_frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BusMap {
    pub foh: OutputTarget,
    pub monitor: OutputTarget,
}

#[derive(Debug, Clone, Copy)]
pub struct MixLevels {
    pub click: f32,
    pub cue: f32,
    pub monitor_backing: f32,
    pub foh_master: f32,
    pub monitor_master: f32,
}

impl Default for MixLevels {
    fn default() -> Self {
        Self { click: 0.5, cue: 0.7, monitor_backing: 0.5, foh_master: 1.0, monitor_master: 1.0 }
    }
}

pub struct TestSignal {
    pub buf: Arc<AudioBuffer>,
    pub to_foh: bool,
    pub to_mon: bool,
}

pub enum MixerCmd {
    Load(Box<LoadedSong>),
    Unload,
    Play { count_in: bool },
    Pause,
    Stop,
    Panic,
    FadeOut { seconds: f32 },
    Seek { frame: u64 },
    JumpToSegment { index: usize, immediate: bool },
    CancelJump,
    SetLoop(bool),
    SetTrack { role: TrackRole, gain: f32, muted: bool },
    SetTrackRouting { role: TrackRole, to_foh: bool, to_mon: bool },
    SetClick { enabled: bool },
    SetCue { enabled: bool },
    SetLevels(MixLevels),
    SetBusMap(BusMap),
    SetClickVoices(Box<ClickVoices>),
    /// Swap the click table. Built off the audio thread; the mixer only moves the vec.
    SetBeatTable { grid: BeatGrid, cue_at: Vec<Option<u64>> },
    Test(Box<TestSignal>),
    StopTest,
}

/// Transport values published through `SharedStatus::transport`.
pub mod transport {
    pub const IDLE: u8 = 0;
    pub const PLAYING: u8 = 1;
    pub const PAUSED: u8 = 2;
    pub const COUNT_IN: u8 = 3;
    pub const PANIC: u8 = 4;
}

/// Lock-free status shared with the control thread / UI poller.
#[derive(Default)]
pub struct SharedStatus {
    pub transport: AtomicU8,
    pub position: AtomicU64,
    pub segment: AtomicU32,
    pub queued: AtomicI32,
    pub looping: AtomicBool,
    pub preroll_remaining: AtomicU64,
    pub ended_count: AtomicU64,
    pub generation: AtomicU64,
    pub fading: AtomicBool,
    pub meters: [AtomicU32; 4],
    pub callbacks: AtomicU64,
    pub stream_error: AtomicBool,
    pub sample_rate: AtomicU32,
}

impl SharedStatus {
    pub fn take_meters(&self) -> [f32; 4] {
        let mut m = [0f32; 4];
        for (i, a) in self.meters.iter().enumerate() {
            m[i] = f32::from_bits(a.swap(0, Ordering::Relaxed));
        }
        m
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Transport {
    Idle,
    CountIn,
    Playing,
    Paused,
    Panic,
}

struct Voice {
    buf: Arc<AudioBuffer>,
    pos: u64,
}

struct CountInPlan {
    map: bool,
    period: f64,
    land_q: i64,
    land: u64,
    seek: bool,
    beats: u32,
    total: u64,
}

pub struct Mixer {
    rx: Receiver<MixerCmd>,
    garbage: Sender<Box<LoadedSong>>,
    status: Arc<SharedStatus>,
    song: Option<Box<LoadedSong>>,
    transport: Transport,
    pos: u64,
    cur_seg: usize,
    queued: Option<usize>,
    looping: bool,
    cue_fired: bool,
    /// Next beat index for fixed-tempo arithmetic mode.
    next_beat: i64,
    /// Next index into `grid.explicit_beats` for explicit-beat mode.
    next_explicit_beat: usize,
    preroll_total: u64,
    preroll_pos: u64,
    next_pre_beat: u64,
    /// Count-in driven by the tempo map. `ci_map` is false for a legacy fixed grid.
    ci_map: bool,
    ci_period: f64,
    ci_land_q: i64,
    ci_land: u64,
    ci_seek: bool,
    ci_beats: u32,
    click_voices: ClickVoices,
    click_voice: Option<(bool, usize)>,
    cue_voice: Option<Voice>,
    test_voice: Option<(Voice, bool, bool)>,
    levels: MixLevels,
    bus: BusMap,
    fade_total: u64,
    fade_pos: u64,
    panic_ramp: u32,
}

impl Mixer {
    pub fn new(
        rx: Receiver<MixerCmd>,
        garbage: Sender<Box<LoadedSong>>,
        status: Arc<SharedStatus>,
        click_voices: ClickVoices,
        bus: BusMap,
        levels: MixLevels,
    ) -> Self {
        status.queued.store(-1, Ordering::Relaxed);
        Self {
            rx,
            garbage,
            status,
            song: None,
            transport: Transport::Idle,
            pos: 0,
            cur_seg: 0,
            queued: None,
            looping: false,
            cue_fired: false,
            next_beat: 0,
            next_explicit_beat: 0,
            preroll_total: 0,
            preroll_pos: 0,
            next_pre_beat: 0,
            ci_map: false,
            ci_period: 0.0,
            ci_land_q: 0,
            ci_land: 0,
            ci_seek: false,
            ci_beats: 0,
            click_voices,
            click_voice: None,
            cue_voice: None,
            test_voice: None,
            levels,
            bus,
            fade_total: 0,
            fade_pos: 0,
            panic_ramp: 0,
        }
    }

    fn count_in_plan(&self) -> Option<CountInPlan> {
        if !matches!(self.transport, Transport::Idle | Transport::Panic | Transport::Paused) {
            return None;
        }
        let song = self.song.as_ref()?;
        let g = song.grid.as_ref()?;
        if song.count_in_beats == 0 {
            return None;
        }
        let sr = self.status.sample_rate.load(Ordering::Relaxed).max(1);
        let downbeat = g.first_downbeat_frame();
        let at_section = song.segments.iter().rev().find(|s| s.start <= self.pos).map(|s| s.start);
        let (land, seek) = if self.pos <= downbeat {
            (downbeat, self.pos < downbeat && downbeat < song.frames)
        } else if let Some(start) = at_section {
            if self.pos <= start.saturating_add(512) { (start, false) } else { (self.pos, false) }
        } else {
            (self.pos, false)
        };
        if let Some(bpm) = g.bpm_at_frame(land).filter(|b| *b >= 20.0) {
            let period = sr as f64 * 60.0 / bpm;
            let land_q = g
                .explicit_beats
                .iter()
                .min_by_key(|b| (b.frame as i64 - land as i64).unsigned_abs())
                .map(|b| b.q as i64)
                .unwrap_or(0);
            Some(CountInPlan {
                map: true,
                period,
                land_q,
                land,
                seek,
                beats: song.count_in_beats,
                total: (period * song.count_in_beats as f64).round() as u64,
            })
        } else {
            Some(CountInPlan {
                map: false,
                period: g.frames_per_beat,
                land_q: 0,
                land,
                seek,
                beats: song.count_in_beats,
                total: (g.frames_per_beat * song.count_in_beats as f64).round() as u64,
            })
        }
    }

    fn sync_beat(&mut self) {
        if let Some(g) = self.song.as_ref().map(|s| &s.grid).and_then(|g| g.as_ref()) {
            if g.has_explicit_beats() {
                // Explicit mode: seek to the first beat at or after current position.
                self.next_explicit_beat = g.explicit_beat_at_or_after(self.pos);
            } else {
                self.next_beat = g.beat_at_or_after(self.pos);
            }
        }
    }

    fn rewind(&mut self) {
        if let Some(s) = &self.song {
            self.cur_seg = s.start_segment.min(s.segments.len().saturating_sub(1));
            self.pos = s.segments.get(self.cur_seg).map(|g| g.start).unwrap_or(0);
        } else {
            self.pos = 0;
            self.cur_seg = 0;
        }
        self.queued = None;
        self.cue_fired = false;
        self.sync_beat();
    }

    fn clear_voices(&mut self) {
        self.click_voice = None;
        self.cue_voice = None;
        self.test_voice = None;
    }

    fn handle(&mut self, cmd: MixerCmd) {
        match cmd {
            MixerCmd::Load(song) => {
                self.status.generation.store(song.generation, Ordering::Relaxed);
                if let Some(old) = self.song.replace(song) {
                    let _ = self.garbage.try_send(old);
                }
                self.transport = Transport::Idle;
                self.looping = false;
                self.fade_total = 0;
                self.click_voice = None;
                self.cue_voice = None;
                self.rewind();
            }
            MixerCmd::Unload => {
                if let Some(old) = self.song.take() {
                    let _ = self.garbage.try_send(old);
                }
                self.transport = Transport::Idle;
                self.clear_voices();
                self.pos = 0;
            }
            MixerCmd::Play { count_in } => {
                let Some(frames) = self.song.as_ref().map(|s| s.frames) else { return };
                self.fade_total = 0;
                self.panic_ramp = 0;
                let fresh = matches!(self.transport, Transport::Idle | Transport::Panic);
                if fresh && self.pos >= frames {
                    self.rewind();
                }
                let plan = if count_in { self.count_in_plan() } else { None };
                let cue = self.song.as_ref().and_then(|song| {
                    if plan.is_some() && song.cue_enabled {
                        song.segments.get(self.cur_seg).and_then(|s| s.cue.clone())
                    } else {
                        None
                    }
                });
                if let Some(plan) = plan {
                    self.ci_map = plan.map;
                    self.ci_period = plan.period;
                    self.ci_land_q = plan.land_q;
                    self.ci_land = plan.land;
                    self.ci_seek = plan.seek;
                    self.ci_beats = plan.beats;
                    self.preroll_total = plan.total;
                    self.preroll_pos = 0;
                    self.next_pre_beat = 0;
                    self.transport = Transport::CountIn;
                    if let Some(cue) = cue {
                        self.cue_voice = Some(Voice { buf: cue, pos: 0 });
                    }
                } else {
                    self.transport = Transport::Playing;
                }
                self.sync_beat();
            }
            MixerCmd::Pause => {
                if matches!(self.transport, Transport::Playing | Transport::CountIn) {
                    self.transport = Transport::Paused;
                    self.click_voice = None;
                    self.cue_voice = None;
                }
            }
            MixerCmd::Stop => {
                self.transport = Transport::Idle;
                self.fade_total = 0;
                self.clear_voices();
                self.rewind();
            }
            MixerCmd::Panic => {
                self.panic_ramp = 64;
                self.transport = Transport::Panic;
                self.fade_total = 0;
                self.preroll_total = 0;
            }
            MixerCmd::FadeOut { seconds } => {
                if matches!(self.transport, Transport::Playing | Transport::CountIn) {
                    let sr = self.status.sample_rate.load(Ordering::Relaxed).max(1) as f32;
                    self.fade_total = (seconds.max(0.05) * sr) as u64;
                    self.fade_pos = 0;
                }
            }
            MixerCmd::Seek { frame } => {
                let Some(song) = &self.song else { return };
                let frame = frame.min(song.frames);
                let in_cur = song.segments.get(self.cur_seg).map(|s| frame >= s.start && frame < s.end);
                if in_cur != Some(true) {
                    if let Some(i) = song.segments.iter().position(|s| frame >= s.start && frame < s.end) {
                        self.cur_seg = i;
                    }
                }
                self.pos = frame;
                self.cue_fired = false;
                self.cue_voice = None;
                self.sync_beat();
            }
            MixerCmd::JumpToSegment { index, immediate } => {
                let Some(song) = &self.song else { return };
                if index >= song.segments.len() {
                    return;
                }
                if immediate || !matches!(self.transport, Transport::Playing | Transport::CountIn) {
                    self.cur_seg = index;
                    self.pos = song.segments[index].start;
                    self.queued = None;
                    self.cue_voice = None;
                    self.sync_beat();
                } else {
                    self.queued = Some(index);
                }
                self.cue_fired = false;
            }
            MixerCmd::CancelJump => {
                self.queued = None;
                self.cue_fired = false;
            }
            MixerCmd::SetLoop(v) => {
                self.looping = v;
                self.cue_fired = false;
            }
            MixerCmd::SetTrack { role, gain, muted } => {
                if let Some(s) = self.song.as_mut() {
                    for t in s.tracks.iter_mut().filter(|t| t.role == role) {
                        t.gain = gain;
                        t.muted = muted;
                    }
                }
            }
            MixerCmd::SetTrackRouting { role, to_foh, to_mon } => {
                if let Some(s) = self.song.as_mut() {
                    for t in s.tracks.iter_mut().filter(|t| t.role == role) {
                        // Guide vocals are never allowed on FOH.
                        t.to_foh = to_foh && role != TrackRole::Vocals;
                        t.to_mon = to_mon;
                    }
                }
            }
            MixerCmd::SetClick { enabled } => {
                if let Some(s) = self.song.as_mut() {
                    s.click_enabled = enabled;
                }
                if !enabled {
                    self.click_voice = None;
                }
            }
            MixerCmd::SetCue { enabled } => {
                if let Some(s) = self.song.as_mut() {
                    s.cue_enabled = enabled;
                }
                if !enabled {
                    self.cue_voice = None;
                }
            }
            MixerCmd::SetLevels(l) => self.levels = l,
            MixerCmd::SetBusMap(b) => self.bus = b,
            MixerCmd::SetClickVoices(v) => {
                self.click_voice = None;
                self.click_voices = *v;
            }
            MixerCmd::SetBeatTable { grid, cue_at } => {
                if let Some(song) = self.song.as_mut() {
                    for (seg, at) in song.segments.iter_mut().zip(cue_at.into_iter()) {
                        seg.cue_at = at;
                    }
                    song.grid = Some(grid);
                }
                self.sync_beat();
            }
            MixerCmd::Test(t) => {
                let t = *t;
                self.test_voice = Some((Voice { buf: t.buf, pos: 0 }, t.to_foh, t.to_mon));
            }
            MixerCmd::StopTest => self.test_voice = None,
        }
    }

    fn publish(&self) {
        let s = &self.status;
        let t = match self.transport {
            Transport::Idle => transport::IDLE,
            Transport::Playing => transport::PLAYING,
            Transport::Paused => transport::PAUSED,
            Transport::CountIn => transport::COUNT_IN,
            Transport::Panic => transport::PANIC,
        };
        s.transport.store(t, Ordering::Relaxed);
        s.position.store(self.pos, Ordering::Relaxed);
        s.segment.store(self.cur_seg as u32, Ordering::Relaxed);
        s.queued.store(self.queued.map(|q| q as i32).unwrap_or(-1), Ordering::Relaxed);
        s.looping.store(self.looping, Ordering::Relaxed);
        let rem = if self.transport == Transport::CountIn {
            self.preroll_total.saturating_sub(self.preroll_pos)
        } else {
            0
        };
        s.preroll_remaining.store(rem, Ordering::Relaxed);
        s.fading.store(self.fade_total > 0, Ordering::Relaxed);
    }

    #[inline]
    fn write_bus(out: &mut [f32], base: usize, channels: usize, target: &OutputTarget, l: f32, r: f32) {
        match *target {
            OutputTarget::None => {}
            OutputTarget::Mono { channel } => {
                let c = channel as usize;
                if c < channels {
                    out[base + c] += (l + r) * 0.5;
                }
            }
            OutputTarget::Stereo { left, right } => {
                let (lc, rc) = (left as usize, right as usize);
                if lc < channels {
                    out[base + lc] += l;
                }
                if rc < channels {
                    out[base + rc] += r;
                }
            }
        }
    }

    /// Fill an interleaved output buffer with `channels` channels.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        for _ in 0..64 {
            match self.rx.try_recv() {
                Ok(cmd) => self.handle(cmd),
                Err(_) => break,
            }
        }
        self.status.callbacks.fetch_add(1, Ordering::Relaxed);
        out.iter_mut().for_each(|s| *s = 0.0);
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut peaks = [0f32; 4];
        let lv = self.levels;

        for f in 0..frames {
            let (mut fl, mut fr, mut ml, mut mr) = (0f32, 0f32, 0f32, 0f32);
            let mut click_trigger: Option<bool> = None;

            if let Some(song) = self.song.as_mut() {
                match self.transport {
                    Transport::CountIn => {
                        if let Some(g) = song.grid.as_ref() {
                            if self.preroll_pos >= self.next_pre_beat {
                                let period = if self.ci_map { self.ci_period } else { g.frames_per_beat };
                                let k = (self.next_pre_beat as f64 / period.max(1.0)).round() as i64;
                                let accent = if self.ci_map {
                                    let q = self.ci_land_q - (self.ci_beats as i64 - k);
                                    crate::audio::click::is_bar_accent(q.rem_euclid(g.beats_per_bar.max(1) as i64), g.beats_per_bar)
                                } else {
                                    g.is_accent_fixed(k)
                                };
                                click_trigger = Some(accent);
                                self.next_pre_beat = (period * (k + 1) as f64).round() as u64;
                            }
                        }
                        self.preroll_pos += 1;
                        if self.preroll_pos >= self.preroll_total {
                            self.transport = Transport::Playing;
                            // Count-in ends on Beat 1 (or the section downbeat). Audio before
                            // that downbeat is skipped so the drummer's "1" is the music's "1".
                            if self.ci_map {
                                if self.ci_seek && self.ci_land < song.frames {
                                    self.pos = self.ci_land;
                                    if let Some(idx) = song.segments.iter().position(|s| self.pos < s.end) {
                                        if idx != self.cur_seg {
                                            self.cur_seg = idx;
                                            self.cue_fired = false;
                                        }
                                    }
                                }
                            } else if let Some(g) = song.grid.as_ref() {
                                let downbeat = g.first_downbeat_frame();
                                if self.pos < downbeat && downbeat < song.frames {
                                    self.pos = downbeat;
                                    if let Some(idx) = song.segments.iter().position(|s| downbeat < s.end) {
                                        if idx != self.cur_seg {
                                            self.cur_seg = idx;
                                            self.cue_fired = false;
                                        }
                                    }
                                }
                            }
                            if let Some(g) = song.grid.as_ref() {
                                if g.has_explicit_beats() {
                                    self.next_explicit_beat = g.explicit_beat_at_or_after(self.pos);
                                } else {
                                    self.next_beat = g.beat_at_or_after(self.pos);
                                }
                            }
                        }
                    }
                    Transport::Playing => {
                        // Segment boundary: follow the arrangement plan.
                        if let Some(seg) = song.segments.get(self.cur_seg) {
                            if self.pos >= seg.end || self.pos >= song.frames {
                                let next = if self.looping {
                                    Some(self.cur_seg)
                                } else if let Some(q) = self.queued.take() {
                                    Some(q)
                                } else if self.cur_seg + 1 < song.segments.len() {
                                    Some(self.cur_seg + 1)
                                } else {
                                    None
                                };
                                match next {
                                    Some(n) => {
                                        let target = song.segments[n].start;
                                        if target != self.pos {
                                            self.pos = target;
                                            // Re-sync beat index for both fixed and explicit modes
                                            if let Some(g) = song.grid.as_ref() {
                                                if g.has_explicit_beats() {
                                                    self.next_explicit_beat = g.explicit_beat_at_or_after(self.pos);
                                                } else {
                                                    self.next_beat = g.beat_at_or_after(self.pos);
                                                }
                                            }
                                        }
                                        self.cur_seg = n;
                                        self.cue_fired = false;
                                    }
                                    None => {
                                        self.transport = Transport::Idle;
                                        self.status.ended_count.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                        if self.transport == Transport::Playing {
                            // Cue for the upcoming segment.
                            if song.cue_enabled && !self.cue_fired {
                                let seg = &song.segments[self.cur_seg];
                                let next = if self.looping {
                                    Some(self.cur_seg)
                                } else {
                                    self.queued.or(if self.cur_seg + 1 < song.segments.len() {
                                        Some(self.cur_seg + 1)
                                    } else {
                                        None
                                    })
                                };
                                let due = match next.and_then(|n| song.segments.get(n).and_then(|s| s.cue_at)) {
                                    Some(at) => self.pos >= at,
                                    None => self.pos + song.cue_lead_frames >= seg.end,
                                };
                                if due {
                                    self.cue_fired = true;
                                    if let Some(c) = next.and_then(|n| song.segments[n].cue.clone()) {
                                        self.cue_voice = Some(Voice { buf: c, pos: 0 });
                                    }
                                }
                            }
                            // Click on the beat grid (song time, so it survives arrangement jumps).
                            // Explicit beats mode is preferred for live recordings (follows music);
                            // fixed formula is used for studio recordings.
                            if let Some(g) = &song.grid {
                                if g.has_explicit_beats() {
                                    // Walk forward through explicit beats table.
                                    while self.next_explicit_beat < g.explicit_beats.len() {
                                        let entry = g.explicit_beats[self.next_explicit_beat];
                                        if self.pos >= entry.frame {
                                            if song.click_enabled {
                                                click_trigger = Some(entry.is_accent);
                                            }
                                            self.next_explicit_beat += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                } else {
                                    // Fixed-tempo arithmetic (original algorithm).
                                    let bf = g.beat_frame(self.next_beat);
                                    if (self.pos as f64) >= bf.round() {
                                        if song.click_enabled {
                                            click_trigger = Some(g.is_accent_fixed(self.next_beat));
                                        }
                                        self.next_beat += 1;
                                    }
                                }
                            }
                            for t in song.tracks.iter_mut() {
                                let target = if t.muted { 0.0 } else { t.gain };
                                t.cur += (target - t.cur) * 0.0015;
                                if t.cur.abs() < 1e-5 && target == 0.0 {
                                    continue;
                                }
                                let (l, r) = t.buf.frame(self.pos);
                                if t.to_foh {
                                    fl += l * t.cur;
                                    fr += r * t.cur;
                                }
                                if t.to_mon {
                                    ml += l * t.cur * lv.monitor_backing;
                                    mr += r * t.cur * lv.monitor_backing;
                                }
                            }
                            self.pos += 1;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(accent) = click_trigger {
                self.click_voice = Some((accent, 0));
            }
            // Click → MONITOR only.
            if let Some((accent, ref mut p)) = self.click_voice {
                let v = if accent { &self.click_voices.accent } else { &self.click_voices.beat };
                if *p < v.len() {
                    let s = v[*p] * lv.click;
                    ml += s;
                    mr += s;
                    *p += 1;
                } else {
                    self.click_voice = None;
                }
            }
            // Cue → MONITOR only.
            if let Some(cv) = self.cue_voice.as_mut() {
                if cv.pos < cv.buf.frames() {
                    let (l, r) = cv.buf.frame(cv.pos);
                    ml += l * lv.cue;
                    mr += r * lv.cue;
                    cv.pos += 1;
                } else {
                    self.cue_voice = None;
                }
            }
            if let Some((tv, to_foh, to_mon)) = self.test_voice.as_mut() {
                if tv.pos < tv.buf.frames() {
                    let (l, r) = tv.buf.frame(tv.pos);
                    if *to_foh {
                        fl += l;
                        fr += r;
                    }
                    if *to_mon {
                        ml += l;
                        mr += r;
                    }
                    tv.pos += 1;
                } else {
                    self.test_voice = None;
                }
            }

            // Fade out (applies to everything).
            let mut g = 1.0;
            if self.fade_total > 0 {
                g = 1.0 - self.fade_pos as f32 / self.fade_total as f32;
                self.fade_pos += 1;
                if self.fade_pos >= self.fade_total {
                    self.fade_total = 0;
                    self.transport = Transport::Paused;
                    self.click_voice = None;
                    self.cue_voice = None;
                }
            }
            if self.transport == Transport::Panic {
                if self.panic_ramp > 0 {
                    g *= self.panic_ramp as f32 / 64.0;
                    self.panic_ramp -= 1;
                } else {
                    g = 0.0;
                    self.clear_voices();
                }
            }
            let gf = g * lv.foh_master;
            let gm = g * lv.monitor_master;
            let (fl, fr, ml, mr) = (fl * gf, fr * gf, ml * gm, mr * gm);
            peaks[0] = peaks[0].max(fl.abs());
            peaks[1] = peaks[1].max(fr.abs());
            peaks[2] = peaks[2].max(ml.abs());
            peaks[3] = peaks[3].max(mr.abs());
            let base = f * channels;
            Self::write_bus(out, base, channels, &self.bus.foh, fl, fr);
            Self::write_bus(out, base, channels, &self.bus.monitor, ml, mr);
        }
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
        for (i, p) in peaks.iter().enumerate() {
            let a = &self.status.meters[i];
            let prev = f32::from_bits(a.load(Ordering::Relaxed));
            if *p > prev {
                a.store(p.to_bits(), Ordering::Relaxed);
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::click::ClickSound;
    use crossbeam_channel::{bounded, unbounded};

    const SR: u32 = 48000;

    fn setup(bus: BusMap) -> (Mixer, Sender<MixerCmd>, Arc<SharedStatus>) {
        let (tx, rx) = unbounded();
        let (gtx, _grx) = bounded(8);
        let status = Arc::new(SharedStatus::default());
        status.sample_rate.store(SR, Ordering::Relaxed);
        let m = Mixer::new(rx, gtx, status.clone(), ClickVoices::new(ClickSound::Classic, SR), bus, MixLevels::default());
        (m, tx, status)
    }

    fn song(backing: Arc<AudioBuffer>, segments: Vec<(u64, u64)>, cue: Option<Arc<AudioBuffer>>) -> Box<LoadedSong> {
        let frames = backing.frames();
        Box::new(LoadedSong {
            generation: 1,
            tracks: vec![Track::new(TrackRole::Backing, backing, 1.0, true, true)],
            frames,
            segments: segments.into_iter().map(|(s, e)| Segment { start: s, end: e, cue: cue.clone(), cue_at: None }).collect(),
            start_segment: 0,
            grid: BeatGrid::new(120.0, 0.0, 4, SR),
            click_enabled: true,
            cue_enabled: true,
            count_in_beats: 4,
            cue_lead_frames: SR as u64,
        })
    }

    fn run(m: &mut Mixer, frames: usize, ch: usize) -> Vec<f32> {
        let mut all = vec![];
        let mut buf = vec![0f32; 512 * ch];
        let mut done = 0;
        while done < frames {
            m.render(&mut buf, ch);
            all.extend_from_slice(&buf);
            done += 512;
        }
        all
    }

    fn channel_energy(data: &[f32], ch: usize, c: usize) -> f32 {
        data.chunks(ch).map(|f| f[c] * f[c]).sum()
    }

    /// THIRD ACCEPTANCE TEST: click and cue must never be present in FOH.
    #[test]
    fn click_and_cue_never_reach_foh_multi() {
        let bus = BusMap {
            foh: OutputTarget::Stereo { left: 0, right: 1 },
            monitor: OutputTarget::Stereo { left: 2, right: 3 },
        };
        let (mut m, tx, _) = setup(bus);
        let silence = Arc::new(AudioBuffer::silent(SR as usize * 4, SR));
        let cue = Arc::new(AudioBuffer::from_mono(&vec![0.5; 4800], SR));
        tx.send(MixerCmd::Load(song(silence, vec![(0, SR as u64 * 2), (SR as u64 * 2, SR as u64 * 4)], Some(cue))))
            .unwrap();
        tx.send(MixerCmd::Play { count_in: true }).unwrap();
        let out = run(&mut m, SR as usize * 5, 4);
        assert_eq!(channel_energy(&out, 4, 0), 0.0, "FOH L must be silent");
        assert_eq!(channel_energy(&out, 4, 1), 0.0, "FOH R must be silent");
        assert!(channel_energy(&out, 4, 2) > 1.0, "monitor must contain click/cue");
    }

    #[test]
    fn click_and_cue_never_reach_foh_split() {
        let bus = BusMap { foh: OutputTarget::Mono { channel: 0 }, monitor: OutputTarget::Mono { channel: 1 } };
        let (mut m, tx, _) = setup(bus);
        // backing is a constant 0.25 DC signal so FOH content is exactly predictable
        let backing = Arc::new(AudioBuffer::new(vec![0.25; SR as usize * 2 * 3], SR));
        tx.send(MixerCmd::Load(song(backing, vec![(0, SR as u64 * 3)], None))).unwrap();
        tx.send(MixerCmd::Play { count_in: false }).unwrap();
        let out = run(&mut m, SR as usize * 2, 2);
        // after gain smoothing settles, FOH must be exactly the backing level — no click transients
        let foh: Vec<f32> = out.chunks(2).skip(SR as usize / 2).take(SR as usize).map(|f| f[0]).collect();
        let (mn, mx) = foh.iter().fold((1f32, -1f32), |(a, b), &v| (a.min(v), b.max(v)));
        assert!((mx - mn) < 1e-3, "FOH varied ({mn}..{mx}) — click leaked");
        // monitor has click transients on top of backing
        let mon: Vec<f32> = out.chunks(2).skip(SR as usize / 2).take(SR as usize).map(|f| f[1]).collect();
        let (mn, mx) = mon.iter().fold((1f32, -1f32), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(mx - mn > 0.1);
    }

    #[test]
    fn arrangement_jump_and_end() {
        let bus = BusMap { foh: OutputTarget::Stereo { left: 0, right: 1 }, monitor: OutputTarget::None };
        let (mut m, tx, status) = setup(bus);
        let b = Arc::new(AudioBuffer::silent(SR as usize, SR));
        // plan: [0..1000) then jump back to [0..1000) again, then [2000..3000)
        let mut s = song(b, vec![(0, 1000), (0, 1000), (2000, 3000)], None);
        s.grid = None;
        tx.send(MixerCmd::Load(s)).unwrap();
        tx.send(MixerCmd::Play { count_in: false }).unwrap();
        let mut buf = vec![0f32; 2 * 1500];
        m.render(&mut buf, 2);
        assert_eq!(status.segment.load(Ordering::Relaxed), 1);
        assert_eq!(status.position.load(Ordering::Relaxed), 500);
        let mut buf = vec![0f32; 2 * 1000];
        m.render(&mut buf, 2);
        assert_eq!(status.segment.load(Ordering::Relaxed), 2);
        assert_eq!(status.position.load(Ordering::Relaxed), 2500);
        let mut buf = vec![0f32; 2 * 1000];
        m.render(&mut buf, 2);
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::IDLE);
        assert_eq!(status.ended_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn panic_silences_everything_immediately() {
        let bus = BusMap { foh: OutputTarget::Stereo { left: 0, right: 1 }, monitor: OutputTarget::None };
        let (mut m, tx, status) = setup(bus);
        let b = Arc::new(AudioBuffer::new(vec![0.5; SR as usize * 2], SR));
        tx.send(MixerCmd::Load(song(b, vec![(0, SR as u64)], None))).unwrap();
        tx.send(MixerCmd::Play { count_in: false }).unwrap();
        run(&mut m, 2048, 2);
        tx.send(MixerCmd::Panic).unwrap();
        let mut buf = vec![0f32; 2 * 512];
        m.render(&mut buf, 2);
        // only the 64-frame anti-pop ramp may contain signal
        assert!(buf[2 * 64..].iter().all(|s| *s == 0.0));
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::PANIC);
        // restart works after panic
        tx.send(MixerCmd::Play { count_in: false }).unwrap();
        m.render(&mut buf, 2);
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::PLAYING);
    }

    #[test]
    fn queued_jump_happens_at_segment_end_and_loop_repeats() {
        let bus = BusMap { foh: OutputTarget::Stereo { left: 0, right: 1 }, monitor: OutputTarget::None };
        let (mut m, tx, status) = setup(bus);
        let b = Arc::new(AudioBuffer::silent(SR as usize, SR));
        let mut s = song(b, vec![(0, 1000), (1000, 2000), (2000, 3000)], None);
        s.grid = None;
        tx.send(MixerCmd::Load(s)).unwrap();
        tx.send(MixerCmd::Play { count_in: false }).unwrap();
        tx.send(MixerCmd::JumpToSegment { index: 2, immediate: false }).unwrap();
        let mut buf = vec![0f32; 2 * 1200];
        m.render(&mut buf, 2);
        assert_eq!(status.segment.load(Ordering::Relaxed), 2);
        assert_eq!(status.position.load(Ordering::Relaxed), 2200);
        tx.send(MixerCmd::SetLoop(true)).unwrap();
        let mut buf = vec![0f32; 2 * 1000];
        m.render(&mut buf, 2);
        assert_eq!(status.segment.load(Ordering::Relaxed), 2);
        assert_eq!(status.position.load(Ordering::Relaxed), 2200);
    }

    #[test]
    fn count_in_precedes_song() {
        let bus = BusMap { foh: OutputTarget::Stereo { left: 0, right: 1 }, monitor: OutputTarget::None };
        let (mut m, tx, status) = setup(bus);
        let b = Arc::new(AudioBuffer::silent(SR as usize * 4, SR));
        tx.send(MixerCmd::Load(song(b, vec![(0, SR as u64 * 4)], None))).unwrap();
        tx.send(MixerCmd::Play { count_in: true }).unwrap();
        run(&mut m, SR as usize, 2); // 1 s of a 2 s count-in (4 beats @120)
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::COUNT_IN);
        assert_eq!(status.position.load(Ordering::Relaxed), 0);
        run(&mut m, SR as usize * 2, 2);
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::PLAYING);
        assert!(status.position.load(Ordering::Relaxed) > 0);
    }

    /// Count-in is whole bars of click, then the song enters on beat 1.
    /// Audio before the downbeat must not play, or the drummer comes in early.
    #[test]
    fn count_in_lands_on_first_downbeat() {
        let bus = BusMap { foh: OutputTarget::Stereo { left: 0, right: 1 }, monitor: OutputTarget::None };
        let (mut m, tx, status) = setup(bus);
        let b = Arc::new(AudioBuffer::silent(SR as usize * 8, SR));
        let mut s = song(b, vec![(0, SR as u64 * 8)], None);
        // Beat 1 is one second in. At 120 BPM a beat is 0.5 s, so the old path
        // would play two weak clicks before the downbeat and the entrance would be late.
        s.grid = BeatGrid::new(120.0, 1.0, 4, SR);
        tx.send(MixerCmd::Load(s)).unwrap();
        tx.send(MixerCmd::Play { count_in: true }).unwrap();
        run(&mut m, SR as usize, 2);
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::COUNT_IN);
        assert_eq!(status.position.load(Ordering::Relaxed), 0);
        // 4 beats of count-in is 2 s. Three seconds total means one second of music
        // that must start at the downbeat (frame 48000), not at frame 0.
        run(&mut m, SR as usize * 2, 2);
        assert_eq!(status.transport.load(Ordering::Relaxed), transport::PLAYING);
        let pos = status.position.load(Ordering::Relaxed);
        let downbeat = SR as u64;
        assert!(
            (downbeat..downbeat + SR as u64 + 512).contains(&pos),
            "song entered at frame {pos}, expected beat 1 ({downbeat}) plus about one second"
        );
    }
}
