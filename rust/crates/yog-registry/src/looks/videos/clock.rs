//! Playing a video: where it is in time, and which packets to decode to
//! show a frame — the same for the game and the editors, whatever decodes.

use std::ops::Range;

use super::VideoDef;

/// The packets to decode, in order, to show a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The decoder forgets what it has first (they start at a keyframe).
    pub reset: bool,
    /// The frames whose packets go in; the last is the one shown.
    pub frames: Range<u32>,
}

impl VideoDef {
    /// How to get from showing `shown` (none yet) to showing `target`:
    /// onward from `shown` when no keyframe is closer, else from the
    /// keyframe before `target`.
    #[must_use]
    pub fn plan(&self, shown: Option<u32>, target: u32) -> Plan {
        let target = target.min(self.frames().saturating_sub(1));
        let key = self.keyframe_before(target);
        match shown {
            Some(s) if s == target => Plan {
                reset: false,
                frames: target..target,
            },
            Some(s) if s < target && key <= s => Plan {
                reset: false,
                frames: s + 1..target + 1,
            },
            _ => Plan {
                reset: true,
                frames: key..target + 1,
            },
        }
    }
}

/// A video's clock: where it is, whether it runs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clock {
    /// Seconds from its start.
    pub at: f64,
    pub playing: bool,
}

impl Clock {
    /// `dt` seconds later, for `def`: wrapped round when it loops, else
    /// stopped at its end. The frame it's on.
    pub fn advance(&mut self, def: &VideoDef, dt: f64) -> u32 {
        if self.playing {
            self.at += dt;
        }
        let length = def.seconds();
        if self.at >= length {
            if def.looping && length > 0.0 {
                self.at %= length;
            } else {
                self.at = length;
                self.playing = false;
            }
        }
        self.frame(def)
    }

    /// Goes to `seconds` from the start (kept inside the video).
    pub fn seek(&mut self, def: &VideoDef, seconds: f64) {
        self.at = seconds.clamp(0.0, def.seconds());
    }

    /// The frame it's on.
    #[must_use]
    pub fn frame(&self, def: &VideoDef) -> u32 {
        let last = def.frames().saturating_sub(1);
        ((self.at / def.frame_seconds()) as u32).min(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 frames at 10 a second, keyframes at 0 and 5.
    fn def(looping: bool) -> VideoDef {
        VideoDef {
            id: "m:v".into(),
            width: 2,
            height: 2,
            fps_num: 10,
            fps_den: 1,
            sizes: vec![1; 10],
            packets: vec![0; 10],
            keyframes: vec![0, 5],
            sound: None,
            volume: 1.0,
            looping,
        }
    }

    #[test]
    fn a_plan_goes_on_or_starts_at_the_keyframe_before() {
        let v = def(false);
        let plan = |shown, target| v.plan(shown, target);
        assert_eq!(
            plan(None, 3),
            Plan {
                reset: true,
                frames: 0..4
            }
        );
        assert_eq!(
            plan(Some(1), 3),
            Plan {
                reset: false,
                frames: 2..4
            }
        );
        assert_eq!(plan(Some(3), 3).frames, 3..3);
        assert_eq!(
            plan(Some(3), 7),
            Plan {
                reset: true,
                frames: 5..8
            }
        );
        assert_eq!(
            plan(Some(7), 2),
            Plan {
                reset: true,
                frames: 0..3
            }
        );
        assert_eq!(plan(None, 99).frames, 5..10);
    }

    #[test]
    fn a_clock_loops_or_stops_at_the_end() {
        let v = def(true);
        let mut c = Clock {
            at: 0.0,
            playing: true,
        };
        assert_eq!(c.advance(&v, 0.35), 3);
        assert_eq!(c.advance(&v, 0.8), 1, "wrapped round");
        let v = def(false);
        let mut c = Clock {
            at: 0.9,
            playing: true,
        };
        assert_eq!(c.advance(&v, 5.0), 9);
        assert!(!c.playing);
        c.seek(&v, -1.0);
        assert_eq!(c.frame(&v), 0);
    }
}
