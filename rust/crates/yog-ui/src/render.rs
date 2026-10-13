//! GPU rendering for the UI tree via `yog-gfx` draw2d.
//! Only valid inside `on_hud_render`.

use crate::layout::{LayoutNode, Rect};
use crate::text;
use crate::widget::{self, FocusStyle, Widget, WidgetKind};
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

fn fill_rect(d2d: &Draw2D, clip: Option<Rect>, r: Rect, color: u32) {
    fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + r.h, color);
}

/// Whether a box spanning `y..y + h` lies inside `clip` vertically (text
/// lines and item icons can't be cut, so they're drawn whole or not at all).
/// Half a pixel of slack absorbs rounding.
fn fits(clip: Option<Rect>, y: f32, h: f32) -> bool {
    clip.is_none_or(|c| y >= c.y - 0.5 && y + h <= c.y + c.h + 0.5)
}

/// `line` cut with `...` to fit `max_w` pixels.
fn ellipsize(line: &str, max_w: f32, font_scale: f32, font: &str) -> String {
    if text::str_width_in(line, font_scale, font) <= max_w {
        return line.to_owned();
    }
    let budget = max_w - text::str_width_in("...", font_scale, font);
    let mut out = String::new();
    let mut w = 0.0;
    for c in line.chars() {
        let cw = text::char_width_in(c, font) * font_scale;
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
    if !widget.visible || !node.visible {
        return;
    }
    let r = &node.rect;
    let style = widget.effective_style();
    let s: &crate::widget::Style = &style;
    if let Some(c) = clip {
        if r.intersect(&c).is_none() {
            return;
        }
    }

    // Background
    if s.bg != 0 {
        fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + r.h, s.bg);
    }
    // Focus indicator — style controlled per-widget. Text boxes draw their
    // own (a white border).
    if node.focused && !matches!(widget.kind, WidgetKind::TextBox(_)) {
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
                let line_w = text::str_width_in(line, s.font_scale, &s.font);
                // No `.max(0.0)`: wrapped lines never exceed avail_w so this
                // never goes negative for them; a no_wrap line wider than the
                // box legitimately needs a negative offset to stay centered.
                let tx = match s.align {
                    crate::Align::Center => r.x + s.pad[3] + (avail_w - line_w) / 2.0,
                    crate::Align::End => r.x + s.pad[3] + (avail_w - line_w).max(0.0),
                    _ => r.x + s.pad[3],
                };
                draw_text(d2d, line, &s.font, tx, ly, s.color, s.text_shadow);
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
                let line_w = text::str_width_in(line, s.font_scale, &s.font);
                let tx = r.x + s.pad[3] + ((avail_w - line_w) / 2.0).max(0.0);
                draw_text(d2d, line, &s.font, tx, ly, s.color, s.text_shadow);
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
            // The item renderer: modded items, 3D block items, any version.
            if !item_id.is_empty() {
                d2d.item(item_id, ix + 1.0, iy + 1.0, 16.0);
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
                d2d.item(item_id, ix + 1.0, iy + 1.0, 16.0);
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
            // A raw texture of `img_w × img_h` pixels, stretched over the
            // content box (it used to be blitted 1:1 at the top-left).
            let image = widget::Image {
                source: id.clone(),
                stretch: crate::Stretch::Fill,
                region: Some([0.0, 0.0, *img_w, *img_h]),
                tint: 0xFFFF_FFFF,
            };
            crate::image::render(d2d, &image, content_box(r, s), clip);
        }
        WidgetKind::Image(image) => crate::image::render(d2d, image, content_box(r, s), clip),
        WidgetKind::Spacer => {} // invisible
        WidgetKind::TextBox(tb) => render_text_box(d2d, tb, s, *r, clip, node.focused),
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
        fill_rect(d2d, clip, track, TRACK_COLOR);
        fill_rect(d2d, clip, knob, KNOB_COLOR);
    }
}

/// `r` without the widget's padding.
fn content_box(r: &Rect, s: &crate::widget::Style) -> Rect {
    Rect {
        x: r.x + s.pad[3],
        y: r.y + s.pad[0],
        w: (r.w - s.pad[1] - s.pad[3]).max(0.0),
        h: (r.h - s.pad[0] - s.pad[2]).max(0.0),
    }
}

/// Text in the widget's font — the plain call for the default font.
fn draw_text(d2d: &Draw2D, line: &str, font: &str, x: f32, y: f32, color: u32, shadow: bool) {
    if text::is_default_font(font) {
        d2d.text(line, x, y, color, shadow);
    } else {
        d2d.text_font(line, font, x, y, color, shadow);
    }
}

/// A text box: border, then the visible part of the text (or the
/// placeholder), the selection behind it and the caret.
fn render_text_box(
    d2d: &Draw2D,
    tb: &widget::TextBox,
    s: &crate::widget::Style,
    r: Rect,
    clip: Option<Rect>,
    focused: bool,
) {
    let border = if focused {
        if s.focus_color != 0 {
            s.focus_color
        } else {
            widget::TEXT_BOX_FOCUS_BORDER
        }
    } else {
        widget::TEXT_BOX_BORDER
    };
    fill(d2d, clip, r.x, r.y, r.x + r.w, r.y + 1.0, border);
    fill(d2d, clip, r.x, r.y + r.h - 1.0, r.x + r.w, r.y + r.h, border);
    fill(d2d, clip, r.x, r.y, r.x + 1.0, r.y + r.h, border);
    fill(d2d, clip, r.x + r.w - 1.0, r.y, r.x + r.w, r.y + r.h, border);

    let fs = s.font_scale;
    let line_h = text::LINE_H * fs;
    let x0 = r.x + 1.0 + s.pad[3];
    let inner_w = (r.w - 2.0 - s.pad[1] - s.pad[3]).max(0.0);
    let content_h = r.h - s.pad[0] - s.pad[2];
    let ty = (r.y + s.pad[0] + ((content_h - line_h) / 2.0).max(0.0)).round();
    if !fits(clip, ty, line_h) {
        return;
    }
    let cw = |c: char| text::char_width_in(c, &s.font) * fs;

    if tb.text.is_empty() {
        if !tb.placeholder.is_empty() {
            let shown = ellipsize(&tb.placeholder, inner_w, fs, &s.font);
            draw_text(d2d, &shown, &s.font, x0, ty, widget::PLACEHOLDER_COLOR, false);
        }
    } else {
        let caret = tb.caret.unwrap_or(0);
        let first = crate::textbox::text_box_first(&tb.text, tb.first, caret, inner_w, cw);
        // The characters that fit, from `first`.
        let mut xs = vec![x0];
        let mut visible = String::new();
        for c in tb.text.chars().skip(first) {
            let next = xs[xs.len() - 1] + cw(c);
            if next > x0 + inner_w + 0.5 {
                break;
            }
            visible.push(c);
            xs.push(next);
        }
        let last = first + xs.len() - 1;
        let (a, b) = tb.selection;
        if a < b && b > first && a < last {
            let sx0 = xs[a.max(first) - first];
            let sx1 = xs[b.min(last) - first];
            fill(d2d, clip, sx0, ty - 1.0, sx1, ty + line_h, widget::SELECTION_COLOR);
        }
        draw_text(d2d, &visible, &s.font, x0, ty, s.color, s.text_shadow);
    }
    if let Some(caret) = tb.caret {
        if tb.caret_visible {
            let first =
                crate::textbox::text_box_first(&tb.text, tb.first, caret, inner_w, cw);
            let cx = x0
                + tb
                    .text
                    .chars()
                    .skip(first)
                    .take(caret.saturating_sub(first))
                    .map(cw)
                    .sum::<f32>();
            let cx = cx.min(x0 + inner_w);
            fill(d2d, clip, cx, ty - 1.0, cx + 1.0, ty + line_h, s.color);
        }
    }
}

/// The lines a label/button draws: wrapped to `avail_w`, or one line
/// (`no_wrap`), cut with `...` when `ellipsis` is set.
fn text_lines(t: &str, s: &crate::widget::Style, avail_w: f32) -> Vec<String> {
    if s.ellipsis {
        vec![ellipsize(t, avail_w, s.font_scale, &s.font)]
    } else if s.no_wrap {
        vec![t.to_owned()]
    } else {
        text::wrap_text_in(t, avail_w, s.font_scale, &s.font)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_fits_the_width() {
        assert_eq!(ellipsize("short", 100.0, 1.0, ""), "short");
        let cut = ellipsize("A rather long description line", 60.0, 1.0, "");
        assert!(cut.ends_with("..."), "{cut}");
        assert!(text::str_width(&cut, 1.0) <= 60.0, "{cut}");
    }

    #[test]
    fn fits_checks_the_clip_vertically() {
        let clip = Some(Rect {
            x: 0.0,
            y: 10.0,
            w: 100.0,
            h: 20.0,
        });
        assert!(fits(clip, 10.0, 9.0));
        assert!(!fits(clip, 5.0, 9.0));
        assert!(!fits(clip, 25.0, 9.0));
        assert!(fits(None, -100.0, 9.0));
    }
}
