//! GPU rendering for the UI tree via `yog-gfx` draw2d.
//! Only valid inside `on_hud_render`.

use crate::layout::{LayoutNode, Rect};
use crate::text;
use crate::widget::{FocusStyle, Widget, WidgetKind};
use yog_gfx::draw2d::Draw2D;

/// Scrollbar track and knob colors.
const TRACK_COLOR: u32 = 0x44_000000;
const KNOB_COLOR: u32 = 0xAA_888888;

/// Recursively render a widget tree starting from the computed layout.
pub fn render_node(d2d: &Draw2D, widget: &Widget, node: &LayoutNode) {
    render_clipped(d2d, widget, node, None);
}

/// Fills `x0..x1 × y0..y1`, cut to `clip`.
fn fill(d2d: &Draw2D, clip: Option<Rect>, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
    let r = Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    };
    let r = match clip {
        Some(c) => match r.intersect(&c) {
            Some(r) => r,
            None => return,
        },
        None => r,
    };
    d2d.rect(r.x, r.y, r.x + r.w, r.y + r.h, color);
}

/// Whether a `w`×`h` box at `(x, y)` lies inside `clip` vertically (text
/// lines and item icons can't be cut, so they're drawn whole or not at all).
/// Half a pixel of slack absorbs rounding.
fn fits(clip: Option<Rect>, y: f32, h: f32) -> bool {
    clip.is_none_or(|c| y >= c.y - 0.5 && y + h <= c.y + c.h + 0.5)
}

/// `line` cut with `...` to fit `max_w` pixels.
fn ellipsize(line: &str, max_w: f32, font_scale: f32) -> String {
    if text::str_width(line, font_scale) <= max_w {
        return line.to_owned();
    }
    let budget = max_w - text::str_width("...", font_scale);
    let mut out = String::new();
    let mut w = 0.0;
    for c in line.chars() {
        let cw = text::char_width(c) * font_scale;
        if w + cw > budget {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.truncate(out.trim_end().len());
    out + "..."
}

fn render_clipped(d2d: &Draw2D, widget: &Widget, node: &LayoutNode, clip: Option<Rect>) {
    let r = &node.rect;
    let s = &widget.style;
    if let Some(c) = clip {
        if r.intersect(&c).is_none() {
            return;
        }
    }

    // Background
    if s.bg != 0 {
        fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + r.h, s.bg);
    }
    // Focus indicator — style controlled per-widget.
    if node.focused {
        let fc = if s.focus_color != 0 {
            s.focus_color
        } else {
            0xFF_FFE040
        };
        match s.focus_style {
            FocusStyle::Outline => {
                fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + 1.0, fc);
                fill(d2d, clip, r.x, r.y + r.h - 1.0, r.x + r.w, r.y + r.h, fc);
                fill(d2d, clip, r.x, r.y, r.x + 1.0, r.y + r.h, fc);
                fill(d2d, clip, r.x + r.w - 1.0, r.y, r.x + r.w, r.y + r.h, fc);
            }
            FocusStyle::Fill => {
                fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + r.h, fc);
            }
            FocusStyle::None => {}
        }
    }

    match &widget.kind {
        WidgetKind::Panel(_) => {
            // Panel just draws children
        }
        WidgetKind::Label(t) => {
            let avail_w = (r.w - s.pad[1] - s.pad[3]).max(0.0);
            // `no_wrap`: always a single line, even wider than the box — it
            // overflows horizontally (still centered) rather than wrapping.
            let lines = text_lines(t, s, avail_w);
            let line_h = text::LINE_H * s.font_scale + text::LINE_GAP;
            let total_h = lines.len() as f32 * line_h - text::LINE_GAP;
            let content_h = r.h - s.pad[0] - s.pad[2];
            let start_y = r.y + s.pad[0] + ((content_h - total_h) / 2.0).max(0.0);
            for (i, line) in lines.iter().enumerate() {
                let ly = start_y + i as f32 * line_h;
                if !fits(clip, ly, text::LINE_H * s.font_scale) {
                    continue;
                }
                let line_w = text::str_width(line, s.font_scale);
                // No `.max(0.0)`: wrapped lines never exceed avail_w so this
                // never goes negative for them; a no_wrap line wider than the
                // box legitimately needs a negative offset to stay centered.
                let tx = match s.align {
                    crate::Align::Center => r.x + s.pad[3] + (avail_w - line_w) / 2.0,
                    crate::Align::End => r.x + s.pad[3] + (avail_w - line_w).max(0.0),
                    _ => r.x + s.pad[3],
                };
                d2d.text(line, tx, ly, s.color, s.text_shadow);
            }
        }
        WidgetKind::Button(t) => {
            let avail_w = (r.w - s.pad[1] - s.pad[3]).max(0.0);
            let lines = text_lines(t, s, avail_w);
            let line_h = text::LINE_H * s.font_scale + text::LINE_GAP;
            let total_h = lines.len() as f32 * line_h - text::LINE_GAP;
            let content_h = r.h - s.pad[0] - s.pad[2];
            let start_y = r.y + s.pad[0] + ((content_h - total_h) / 2.0).max(0.0);
            for (i, line) in lines.iter().enumerate() {
                let ly = start_y + i as f32 * line_h;
                if !fits(clip, ly, text::LINE_H * s.font_scale) {
                    continue;
                }
                // Center each line horizontally inside the button.
                let line_w = text::str_width(line, s.font_scale);
                let tx = r.x + s.pad[3] + ((avail_w - line_w) / 2.0).max(0.0);
                d2d.text(line, tx, ly, s.color, s.text_shadow);
            }
        }
        WidgetKind::ItemSlot(item_id) => {
            let ix = r.x + s.pad[3];
            let iy = r.y + s.pad[0];
            if !fits(clip, iy, 18.0) {
                return;
            }
            let sz = 18.0;
            d2d.rect(ix, iy, ix + sz, iy + sz, 0xFF_444444);
            d2d.rect(
                ix + 1.0,
                iy + 1.0,
                ix + sz - 1.0,
                iy + sz - 1.0,
                0xFF_888888,
            );
            if let Some((ns, name)) = item_id.split_once(':') {
                d2d.mc_texture(
                    &format!("{ns}:textures/item/{name}.png"),
                    ix + 2.0,
                    iy + 2.0,
                    0.0,
                    0.0,
                    14.0,
                    14.0,
                    16.0,
                    16.0,
                );
            }
        }
        WidgetKind::InvSlot { item_id, count } => {
            let ix = r.x + s.pad[3];
            let iy = r.y + s.pad[0];
            if !fits(clip, iy, 18.0) {
                return;
            }
            let sz: f32 = 18.0;
            // Slot background
            d2d.rect(ix, iy, ix + sz, iy + sz, 0xFF_8B8B8B);
            d2d.rect(ix, iy, ix + sz, iy + 1.0, 0xFF_373737);
            d2d.rect(ix, iy + sz - 1.0, ix + sz, iy + sz, 0xFF_FFFFFF);
            d2d.rect(ix, iy, ix + 1.0, iy + sz, 0xFF_373737);
            d2d.rect(ix + sz - 1.0, iy, ix + sz, iy + sz, 0xFF_FFFFFF);
            // Item icon + count
            if !item_id.is_empty() {
                if let Some((ns, name)) = item_id.split_once(':') {
                    d2d.mc_texture(
                        &format!("{ns}:textures/item/{name}.png"),
                        ix + 2.0,
                        iy + 2.0,
                        0.0,
                        0.0,
                        14.0,
                        14.0,
                        16.0,
                        16.0,
                    );
                }
                if *count > 1 {
                    let count_str = count.to_string();
                    let cw = crate::text::str_width(&count_str, 0.7);
                    d2d.text(
                        &count_str,
                        ix + sz - cw - 1.0,
                        iy + sz - 10.0,
                        0xFF_FFFFFF,
                        true,
                    );
                }
            }
        }
        WidgetKind::McImage { id, img_w, img_h } => {
            if !fits(clip, r.y + s.pad[0], *img_h) {
                return;
            }
            d2d.mc_texture(
                id,
                r.x + s.pad[3],
                r.y + s.pad[0],
                0.0,
                0.0,
                *img_w,
                *img_h,
                *img_w,
                *img_h,
            );
        }
        WidgetKind::Spacer => {} // invisible
    }

    // Render children — cut to this widget's rect when it clips.
    let child_clip = if s.clip {
        match clip {
            Some(c) => match r.intersect(&c) {
                Some(i) => Some(i),
                None => return,
            },
            None => Some(*r),
        }
    } else {
        clip
    };
    for (i, child_widget) in widget.children.iter().enumerate() {
        if let Some(child_node) = node.children.get(i) {
            render_clipped(d2d, child_widget, child_node, child_clip);
        }
    }

    if let Some((track, knob)) = node.scrollbar_rects() {
        fill(d2d, clip, track.x, track.y, track.x + track.w, track.y + track.h, TRACK_COLOR);
        fill(d2d, clip, knob.x, knob.y, knob.x + knob.w, knob.y + knob.h, KNOB_COLOR);
    }
}

/// The lines a label/button draws: wrapped to `avail_w`, or one line
/// (`no_wrap`), cut with `...` when `ellipsis` is set.
fn text_lines(t: &str, s: &crate::widget::Style, avail_w: f32) -> Vec<String> {
    if s.ellipsis {
        vec![ellipsize(t, avail_w, s.font_scale)]
    } else if s.no_wrap {
        vec![t.to_owned()]
    } else {
        text::wrap_text(t, avail_w, s.font_scale)
    }
}
