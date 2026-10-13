//! The raw UI events a Yog screen's `register_ui` handler receives, parsed
//! once into [`UiEvent`].
//!
//! | String | Event |
//! |---|---|
//! | `click:X:Y` | [`UiEvent::Click`] |
//! | `drag:X:Y` | [`UiEvent::Drag`] |
//! | `release:X:Y` | [`UiEvent::Release`] |
//! | `move:X:Y` | [`UiEvent::Move`] (the cursor moved; newer hosts) |
//! | `scroll:DY` | [`UiEvent::Scroll`] (positive = up) |
//! | `key:KEY:MODS` | [`UiEvent::Key`] (GLFW key code and modifier bits) |
//! | `char:CODEPOINT` | [`UiEvent::Char`] |
//! | `open:W:H` | [`UiEvent::Open`] (the screen was opened; newer hosts) |
//! | `close` | [`UiEvent::Close`] (the screen was closed; newer hosts) |
//!
//! Anything else is [`UiEvent::Other`].

/// One parsed UI event — see the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEvent<'a> {
    Click { x: f32, y: f32 },
    Drag { x: f32, y: f32 },
    Release { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Scroll { dy: f32 },
    Key { key: i32, mods: i32 },
    Char(char),
    Open { w: f32, h: f32 },
    Close,
    Other(&'a str),
}

/// GLFW key codes and modifier bits.
pub mod keys {
    pub const SPACE: i32 = 32;
    pub const A: i32 = 65;
    pub const ESCAPE: i32 = 256;
    pub const ENTER: i32 = 257;
    pub const TAB: i32 = 258;
    pub const BACKSPACE: i32 = 259;
    pub const DELETE: i32 = 261;
    pub const RIGHT: i32 = 262;
    pub const LEFT: i32 = 263;
    pub const DOWN: i32 = 264;
    pub const UP: i32 = 265;
    pub const PAGE_UP: i32 = 266;
    pub const PAGE_DOWN: i32 = 267;
    pub const HOME: i32 = 268;
    pub const END: i32 = 269;
    pub const KP_ENTER: i32 = 335;
    pub const MOD_SHIFT: i32 = 0x1;
    pub const MOD_CONTROL: i32 = 0x2;
    pub const MOD_ALT: i32 = 0x4;
    pub const MOD_SUPER: i32 = 0x8;
}

fn xy(rest: &str) -> Option<(f32, f32)> {
    let mut it = rest.split(':');
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    it.next().is_none().then_some((x, y))
}

impl<'a> UiEvent<'a> {
    /// Parses a host event string; malformed or unknown ones are `Other`.
    pub fn parse(raw: &'a str) -> Self {
        Self::try_parse(raw).unwrap_or(UiEvent::Other(raw))
    }

    fn try_parse(raw: &'a str) -> Option<Self> {
        if raw == "close" {
            return Some(UiEvent::Close);
        }
        let (head, rest) = raw.split_once(':')?;
        Some(match head {
            "click" => xy(rest).map(|(x, y)| UiEvent::Click { x, y })?,
            "drag" => xy(rest).map(|(x, y)| UiEvent::Drag { x, y })?,
            "release" => xy(rest).map(|(x, y)| UiEvent::Release { x, y })?,
            "move" => xy(rest).map(|(x, y)| UiEvent::Move { x, y })?,
            "open" => xy(rest).map(|(w, h)| UiEvent::Open { w, h })?,
            "scroll" => UiEvent::Scroll {
                dy: rest.parse().ok()?,
            },
            "key" => {
                let mut it = rest.split(':');
                let key = it.next()?.parse().ok()?;
                let mods = match it.next() {
                    Some(m) => m.parse().ok()?,
                    None => 0,
                };
                UiEvent::Key { key, mods }
            }
            "char" => UiEvent::Char(rest.parse::<u32>().ok().and_then(char::from_u32)?),
            _ => return None,
        })
    }

    /// The cursor position of a pointer event.
    pub fn pos(&self) -> Option<(f32, f32)> {
        match *self {
            UiEvent::Click { x, y }
            | UiEvent::Drag { x, y }
            | UiEvent::Release { x, y }
            | UiEvent::Move { x, y } => Some((x, y)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_host_format() {
        assert_eq!(UiEvent::parse("click:1.5:2.0"), UiEvent::Click { x: 1.5, y: 2.0 });
        assert_eq!(UiEvent::parse("drag:3:4"), UiEvent::Drag { x: 3.0, y: 4.0 });
        assert_eq!(UiEvent::parse("release:3:4"), UiEvent::Release { x: 3.0, y: 4.0 });
        assert_eq!(UiEvent::parse("move:3.0:4.5"), UiEvent::Move { x: 3.0, y: 4.5 });
        assert_eq!(UiEvent::parse("scroll:-2.00"), UiEvent::Scroll { dy: -2.0 });
        assert_eq!(UiEvent::parse("key:256:0"), UiEvent::Key { key: 256, mods: 0 });
        assert_eq!(UiEvent::parse("key:65"), UiEvent::Key { key: 65, mods: 0 });
        assert_eq!(UiEvent::parse("char:1078"), UiEvent::Char('ж'));
        assert_eq!(UiEvent::parse("open:320:240"), UiEvent::Open { w: 320.0, h: 240.0 });
        assert_eq!(UiEvent::parse("close"), UiEvent::Close);
    }

    #[test]
    fn garbage_is_other() {
        for raw in ["", "click", "click:x:1", "click:1:2:3", "scroll:", "char:-1", "done", "key:a:0"] {
            assert_eq!(UiEvent::parse(raw), UiEvent::Other(raw), "{raw}");
        }
    }
}
