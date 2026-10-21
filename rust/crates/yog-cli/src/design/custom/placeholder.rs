//! What stands in for a component the designer can't draw yet: its name,
//! and why (building, or the build failed).
use crate::design::raster::DrawOp;

/// A placeholder's size when its element gives none, and its look: a dark
/// box, its border and message bluish while the library builds, red when
/// it failed; text padded, a line apart, wrapped at a few columns at least.
pub(super) const PLACEHOLDER_W: f32 = 80.0;
pub(super) const PLACEHOLDER_H: f32 = 20.0;
/// A placeholder with a problem to tell: a little taller.
pub(super) const PROBLEM_H: f32 = 24.0;
const PLACEHOLDER_BG: u32 = 0xD0_202028;
const BUILDING_BORDER: u32 = 0xFF_8080B0;
const FAILED_BORDER: u32 = 0xFF_E05050;
const BUILDING_TEXT: u32 = 0xFF_B0B0D0;
const FAILED_TEXT: u32 = 0xFF_FF9090;
const PAD: f32 = 3.0;
const LINE: f32 = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
const GLYPH_H: f32 = yog_ui::text::GLYPH_H;
const CHAR_W: f32 = yog_ui::text::CHAR_W;
const MIN_COLS: usize = 4;
/// What the message of a library still being built starts with.
pub(super) const BUILDING: &str = "Building";

/// The box shown for a component that can't be drawn: its name and why.
pub fn placeholder(name: &str, message: &str, w: f32, h: f32) -> Vec<DrawOp> {
    let building = message.starts_with(BUILDING);
    let border = if building {
        BUILDING_BORDER
    } else {
        FAILED_BORDER
    };
    let mut ops = vec![
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: w,
            y2: h,
            color: PLACEHOLDER_BG,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: w,
            y2: 1.0,
            color: border,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: h - 1.0,
            x2: w,
            y2: h,
            color: border,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: h,
            color: border,
        },
        DrawOp::Rect {
            x1: w - 1.0,
            y1: 0.0,
            x2: w,
            y2: h,
            color: border,
        },
    ];
    let cols = (((w - 2.0 * PAD) / CHAR_W).floor() as usize).max(MIN_COLS);
    let mut lines = vec![format!("<{name}>")];
    for l in message.lines() {
        let chars: Vec<char> = l.chars().collect();
        lines.extend(chars.chunks(cols).map(|c| c.iter().collect::<String>()));
    }
    for (i, line) in lines.iter().enumerate() {
        let y = PAD + i as f32 * LINE;
        if y + GLYPH_H > h {
            break;
        }
        let color = if i == 0 {
            yog_gfx::Color::WHITE.0
        } else if building {
            BUILDING_TEXT
        } else {
            FAILED_TEXT
        };
        ops.push(DrawOp::Text {
            text: line.clone(),
            font: yog_ids::yog::DEFAULT_FONT.into(),
            x: PAD,
            y,
            color,
            shadow: false,
        });
    }
    ops
}
