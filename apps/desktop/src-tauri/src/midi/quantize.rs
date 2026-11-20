//! The section click rule, shared by pads, footswitches and the computer keyboard.
//! Mirrors `nextJumpMode` in `src/lib/transport.ts` (mouse taps on the Live page):
//!   1st press → switch when the current part ends (slow blink)
//!   2nd press on the same section → switch at the end of this bar (fast blink)
//!   3rd press → cancel

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpMode {
    Section,
    Bar,
    Cancel,
}

/// A press counts as "the second press" this soon after the first, even before the
/// audio thread has published the queued jump.
pub const RECENT: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, Copy)]
pub struct Recent {
    pub index: usize,
    pub bar: bool,
    pub at: Instant,
}

pub fn next_jump_mode(index: usize, queued: Option<(usize, bool)>, recent: Option<(usize, bool)>) -> JumpMode {
    let q = recent.or(queued);
    match q {
        Some((i, bar)) if i == index => {
            if bar {
                JumpMode::Cancel
            } else {
                JumpMode::Bar
            }
        }
        _ => JumpMode::Section,
    }
}

/// Remembers the last press so a quick double press is recognised.
#[derive(Debug, Default)]
pub struct JumpMemory {
    last: Option<Recent>,
}

impl JumpMemory {
    /// Decide the mode for a press on `index` while rolling, and remember it.
    pub fn press(&mut self, index: usize, queued: Option<(usize, bool)>, now: Instant) -> JumpMode {
        let recent = self
            .last
            .filter(|r| now.duration_since(r.at) < RECENT)
            .map(|r| (r.index, r.bar));
        let mode = next_jump_mode(index, queued, recent);
        self.last = match mode {
            JumpMode::Cancel => None,
            m => Some(Recent { index, bar: m == JumpMode::Bar, at: now }),
        };
        mode
    }

    pub fn clear(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_twice_three_times() {
        let mut m = JumpMemory::default();
        let t = Instant::now();
        assert_eq!(m.press(3, None, t), JumpMode::Section);
        // The audio thread has not published the queue yet: memory still sees the first press.
        assert_eq!(m.press(3, None, t + Duration::from_millis(250)), JumpMode::Bar);
        assert_eq!(m.press(3, Some((3, true)), t + Duration::from_millis(500)), JumpMode::Cancel);
    }

    #[test]
    fn second_press_later_uses_published_queue() {
        let mut m = JumpMemory::default();
        let t = Instant::now();
        assert_eq!(m.press(2, None, t), JumpMode::Section);
        // 3 s later the jump is still queued (end of part): the next press means end of bar.
        assert_eq!(m.press(2, Some((2, false)), t + Duration::from_secs(3)), JumpMode::Bar);
    }

    #[test]
    fn a_different_section_replaces_the_queue() {
        let mut m = JumpMemory::default();
        let t = Instant::now();
        assert_eq!(m.press(1, None, t), JumpMode::Section);
        assert_eq!(m.press(4, Some((1, false)), t + Duration::from_millis(100)), JumpMode::Section);
    }

    #[test]
    fn matches_the_ui_rule() {
        assert_eq!(next_jump_mode(1, None, None), JumpMode::Section);
        assert_eq!(next_jump_mode(1, Some((1, false)), None), JumpMode::Bar);
        assert_eq!(next_jump_mode(1, Some((1, true)), None), JumpMode::Cancel);
        assert_eq!(next_jump_mode(1, Some((2, true)), None), JumpMode::Section);
        assert_eq!(next_jump_mode(1, Some((2, false)), Some((1, false))), JumpMode::Bar);
    }
}
