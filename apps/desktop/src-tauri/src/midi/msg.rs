//! Raw MIDI bytes → small typed messages, plus the value maths for knobs:
//! absolute values with Pick-Up takeover and relative encoders (Arturia Relative #1/#2/#3).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Note,
    Cc,
    Pc,
    Key,
}

/// One incoming control event. `value` is 0..=127 (velocity, CC value; 127 for PC / key down).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlMsg {
    pub kind: SourceKind,
    /// 0..=15 for MIDI, 0 for the computer keyboard.
    pub channel: u8,
    pub number: u8,
    pub value: u8,
    /// Note on / key down / PC = true. Note off / key up = false. For CC: value >= 64.
    pub pressed: bool,
    /// KeyboardEvent.code for the computer keyboard.
    pub key: Option<String>,
}

/// Parse one MIDI message. System messages (clock, sysex, active sensing…) return None.
pub fn parse(bytes: &[u8]) -> Option<ControlMsg> {
    let status = *bytes.first()?;
    if status < 0x80 || status >= 0xF0 {
        return None;
    }
    let channel = status & 0x0F;
    let d1 = bytes.get(1).copied().unwrap_or(0) & 0x7F;
    let d2 = bytes.get(2).copied().unwrap_or(0) & 0x7F;
    let m = |kind, value: u8, pressed| Some(ControlMsg { kind, channel, number: d1, value, pressed, key: None });
    match status & 0xF0 {
        0x90 if bytes.len() >= 3 => m(SourceKind::Note, d2, d2 > 0),
        0x80 if bytes.len() >= 3 => m(SourceKind::Note, 0, false),
        0xB0 if bytes.len() >= 3 => m(SourceKind::Cc, d2, d2 >= 64),
        0xC0 if bytes.len() >= 2 => m(SourceKind::Pc, 127, true),
        _ => None,
    }
}

pub fn key_msg(code: &str, pressed: bool) -> ControlMsg {
    ControlMsg {
        kind: SourceKind::Key,
        channel: 0,
        number: 0,
        value: if pressed { 127 } else { 0 },
        pressed,
        key: Some(code.to_string()),
    }
}

/// How a knob / fader sends its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Encoder {
    /// 0–127 position. Uses Pick-Up so the level never jumps.
    #[default]
    Absolute,
    /// Arturia "Relative #1" (Ableton Bin Offset): 61–63 down, 65–67 up, 64 neutral.
    Relative1,
    /// Arturia "Relative #2" (Ableton Two's Complement): 125–127 down, 1–3 up.
    Relative2,
    /// Arturia "Relative #3": 13–15 down, 17–19 up, 16 neutral.
    Relative3,
}

/// Signed step from a relative encoder. `None` = neutral / out of range (ignored, never a jump).
pub fn relative_delta(enc: Encoder, v: u8) -> Option<i32> {
    let v = v as i32;
    let d = match enc {
        Encoder::Absolute => return None,
        Encoder::Relative1 => v - 64,
        Encoder::Relative2 => {
            if v == 0 || v == 64 {
                return None;
            }
            if v < 64 {
                v
            } else {
                v - 128
            }
        }
        Encoder::Relative3 => v - 16,
    };
    // 0 = neutral. Large values mean the knob is actually sending absolute positions:
    // ignore them instead of jumping the level.
    if v == 0 || d == 0 || d.abs() > 8 {
        None
    } else {
        Some(d)
    }
}

/// One relative click moves a 0..1 parameter by this much (≈ 0.3 dB on a track fader).
pub const RELATIVE_STEP: f32 = 0.01;

/// Pick-Up takeover for absolute knobs (Ableton): the knob only takes over once it reaches or
/// crosses the current value. Prevents a sudden jump when the knob and the app disagree.
#[derive(Debug, Clone, Default)]
pub struct Pickup {
    last_in: Option<f32>,
    last_out: Option<f32>,
    picked: bool,
}

/// Tolerance for "the knob is at the current value" (≈ 2 MIDI steps).
const PICKUP_TOLERANCE: f32 = 2.0 / 127.0;

impl Pickup {
    /// `incoming` and `current` are 0..1. Returns the new value to apply, or None while waiting.
    pub fn feed(&mut self, incoming: f32, current: f32) -> Option<f32> {
        // The value moved elsewhere (mouse, another controller): pick up again.
        if let Some(out) = self.last_out {
            if (out - current).abs() > 0.005 {
                self.picked = false;
            }
        }
        if !self.picked {
            let near = (incoming - current).abs() <= PICKUP_TOLERANCE;
            let crossed = self
                .last_in
                .map(|prev| (prev - current) * (incoming - current) <= 0.0)
                .unwrap_or(false);
            self.last_in = Some(incoming);
            if !(near || crossed) {
                return None;
            }
            self.picked = true;
        }
        self.last_in = Some(incoming);
        self.last_out = Some(incoming);
        Some(incoming)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_cc_pc_and_ignores_system() {
        let n = parse(&[0x99, 36, 100]).unwrap();
        assert_eq!((n.kind, n.channel, n.number, n.pressed), (SourceKind::Note, 9, 36, true));
        let off = parse(&[0x99, 36, 0]).unwrap();
        assert!(!off.pressed, "note on with velocity 0 is a release");
        assert!(!parse(&[0x89, 36, 64]).unwrap().pressed);
        let cc = parse(&[0xB0, 112, 65]).unwrap();
        assert_eq!((cc.kind, cc.number, cc.value), (SourceKind::Cc, 112, 65));
        let pc = parse(&[0xC2, 5]).unwrap();
        assert_eq!((pc.kind, pc.channel, pc.number, pc.pressed), (SourceKind::Pc, 2, 5, true));
        assert!(parse(&[0xF8]).is_none(), "clock");
        assert!(parse(&[0xFE]).is_none(), "active sensing");
        assert!(parse(&[0xF0, 0x00, 0x20, 0x6B, 0xF7]).is_none(), "sysex");
        assert!(parse(&[0x40]).is_none(), "running status data byte");
    }

    #[test]
    fn relative1_bin_offset() {
        assert_eq!(relative_delta(Encoder::Relative1, 65), Some(1));
        assert_eq!(relative_delta(Encoder::Relative1, 67), Some(3));
        assert_eq!(relative_delta(Encoder::Relative1, 63), Some(-1));
        assert_eq!(relative_delta(Encoder::Relative1, 61), Some(-3));
        assert_eq!(relative_delta(Encoder::Relative1, 64), None, "neutral");
        assert_eq!(relative_delta(Encoder::Relative1, 0), None, "neutral 0 is ignored");
        assert_eq!(relative_delta(Encoder::Relative1, 127), None, "absolute value never jumps");
    }

    #[test]
    fn relative2_twos_complement() {
        assert_eq!(relative_delta(Encoder::Relative2, 1), Some(1));
        assert_eq!(relative_delta(Encoder::Relative2, 3), Some(3));
        assert_eq!(relative_delta(Encoder::Relative2, 127), Some(-1));
        assert_eq!(relative_delta(Encoder::Relative2, 125), Some(-3));
        assert_eq!(relative_delta(Encoder::Relative2, 0), None);
        assert_eq!(relative_delta(Encoder::Relative2, 64), None);
        assert_eq!(relative_delta(Encoder::Relative2, 40), None);
    }

    #[test]
    fn relative3_offset16() {
        assert_eq!(relative_delta(Encoder::Relative3, 17), Some(1));
        assert_eq!(relative_delta(Encoder::Relative3, 13), Some(-3));
        assert_eq!(relative_delta(Encoder::Relative3, 16), None);
        assert_eq!(relative_delta(Encoder::Absolute, 65), None);
    }

    #[test]
    fn pickup_waits_until_knob_reaches_value() {
        let mut p = Pickup::default();
        // App is at 0.8, knob starts at 0.1 and moves up slowly: nothing happens…
        assert_eq!(p.feed(0.10, 0.8), None);
        assert_eq!(p.feed(0.40, 0.8), None);
        assert_eq!(p.feed(0.70, 0.8), None);
        // …until it crosses the current value, then it follows.
        assert_eq!(p.feed(0.82, 0.8), Some(0.82));
        assert_eq!(p.feed(0.60, 0.82), Some(0.60));
    }

    #[test]
    fn pickup_takes_over_when_already_matching() {
        let mut p = Pickup::default();
        assert_eq!(p.feed(0.5, 0.5), Some(0.5));
        assert_eq!(p.feed(0.55, 0.5), Some(0.55));
    }

    #[test]
    fn pickup_releases_after_value_changed_elsewhere() {
        let mut p = Pickup::default();
        assert_eq!(p.feed(0.5, 0.5), Some(0.5));
        // The mouse moved the fader to 0.2: the knob must pick up again.
        assert_eq!(p.feed(0.52, 0.2), None);
        assert_eq!(p.feed(0.19, 0.2), Some(0.19));
    }
}
