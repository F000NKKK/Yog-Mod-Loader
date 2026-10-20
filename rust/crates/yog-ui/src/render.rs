//! GPU rendering for the UI tree via `yog-gfx` draw2d.
//! Only valid inside `on_hud_render`.

use crate::layout::LayoutNode;
use crate::text;
use crate::widget::{self, FocusStyle, SLOT_SIZE, Style, Widget, WidgetKind};
use yog_gfx::draw2d::{Draw2D, TextStyle};
use yog_gfx::{Color, GfxContext, Rect, Vec2};

/// Scrollbar track and knob colors.
const TRACK_COLOR: Color = Color(0x44_000000);
const KNOB_COLOR: Color = Color(0xAA_888888);
/// Item slot: border and face.
const ITEM_SLOT_BORDER: Color = Color(0xFF_444444);
const ITEM_SLOT_FACE: Color = Color(0xFF_888888);
/// Inventory slot (vanilla's): face, top/left shadow, bottom/right light.
pub const INV_SLOT_FACE: Color = Color(0xFF_8B8B8B);
pub const INV_SLOT_SHADOW: Color = Color(0xFF_373737);
pub const INV_SLOT_LIGHT: Color = Color(0xFF_FFFFFF);
/// Stack count text color.
const COUNT_COLOR: Color = Color(0xFF_FFFFFF);
/// Where vanilla draws a stack's count, from the item's top-left: its
/// right edge, and its top.
const COUNT_RIGHT: f32 = ICON_SIZE + 1.0;
const COUNT_TOP: f32 = 9.0;
/// A surface nothing draws (a designer previewing the markup): its face,
/// edge and name, and the room the name needs around it.
const SURFACE_FACE: Color = Color(0xFF_1E2622);
const SURFACE_EDGE: Color = Color(0xFF_4A5A52);
const SURFACE_NAME: Color = Color(0xFF_8FA399);
const SURFACE_NAME_ROOM: f32 = 4.0;
/// Side of an item icon inside a slot (1 px inset).
pub const ICON_SIZE: f32 = yog_ids::units::ITEM_PIXELS as f32;
/// Slack for "drawn whole or not at all" clip checks — absorbs rounding.
pub(crate) const CLIP_SLACK: f32 = 0.5;

/// Recursively render a widget tree starting from the computed layout.
pub fn render_node(gfx: &GfxContext, d2d: &Draw2D, widget: &Widget, node: &LayoutNode) {
    render_clipped(gfx, d2d, widget, node, None);
}

/// Fills `r`, cut to `clip`.
fn fill(d2d: &Draw2D, clip: Option<Rect>, r: Rect, color: Color) {
    let r = match clip {
        Some(c) => match r.intersect(&c) {
            Some(r) => r,
            None => return,
        },
        None => r,
    };
    d2d.rect(r, color);
}

/// A 1 px border just inside `r`, cut to `clip`.
fn outline(d2d: &Draw2D, clip: Option<Rect>, r: Rect, color: Color) {
    fill(
        d2d,
        clip,
        Rect::from_edges(r.x, r.y, r.x + r.w, r.y + 1.0),
        color,
    );
    fill(
        d2d,
        clip,
        Rect::from_edges(r.x, r.y + r.h - 1.0, r.x + r.w, r.y + r.h),
        color,
    );
    fill(
        d2d,
        clip,
        Rect::from_edges(r.x, r.y, r.x + 1.0, r.y + r.h),
        color,
    );
    fill(
        d2d,
        clip,
        Rect::from_edges(r.x + r.w - 1.0, r.y, r.x + r.w, r.y + r.h),
        color,
    );
}

/// Whether a box spanning `y..y + h` lies inside `clip` vertically (text
/// lines and item icons can't be cut, so they're drawn whole or not at all).
/// [`CLIP_SLACK`] absorbs rounding.
fn fits(clip: Option<Rect>, y: f32, h: f32) -> bool {
    clip.is_none_or(|c| y >= c.y - CLIP_SLACK && y + h <= c.y + c.h + CLIP_SLACK)
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

fn render_clipped(
    gfx: &GfxContext,
    d2d: &Draw2D,
    widget: &Widget,
    node: &LayoutNode,
    clip: Option<Rect>,
) {
    if !widget.visible || !node.visible {
        return;
    }
    let r = &node.rect;
    let style = widget.effective_style();
    let s: &Style = &style;
    if let Some(c) = clip
        && r.intersect(&c).is_none()
    {
        return;
    }

    draw_back(d2d, widget, node, s, clip);
    draw_kind((gfx, d2d), widget, node, s, clip);

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
            render_clipped(gfx, d2d, child_widget, child_node, child_clip);
        }
    }

    if let Some((track, knob)) = node.scrollbar_rects() {
        fill(d2d, clip, track, TRACK_COLOR);
        fill(d2d, clip, knob, KNOB_COLOR);
    }
}

/// The background, its image, and the focus indicator — style controlled
/// per widget. Text boxes draw their own (a white border).
fn draw_back(d2d: &Draw2D, widget: &Widget, node: &LayoutNode, s: &Style, clip: Option<Rect>) {
    let r = &node.rect;
    if s.bg != Color::TRANSPARENT {
        fill(d2d, clip, *r, s.bg);
    }
    // An image can't be cut: drawn while it's inside the clip.
    if !s.bg_image.is_empty() && fits(clip, r.y, r.h) {
        d2d.image(&s.bg_image, *r);
    }
    if node.focused && !matches!(widget.kind, WidgetKind::TextBox(_)) {
        let fc = if s.focus_color != Color::TRANSPARENT {
            s.focus_color
        } else {
            widget::DEFAULT_FOCUS_COLOR
        };
        match s.focus_style {
            FocusStyle::Outline => outline(d2d, clip, *r, fc),
            FocusStyle::Fill => fill(d2d, clip, *r, fc),
            FocusStyle::None => {}
        }
    }
}

/// What the widget's kind draws (containers just draw their children).
/// `to`: where it's drawn — the context (a surface paints with it) and 2D.
fn draw_kind(
    to: (&GfxContext, &Draw2D),
    widget: &Widget,
    node: &LayoutNode,
    s: &Style,
    clip: Option<Rect>,
) {
    let (gfx, d2d) = to;
    let r = node.rect;
    match &widget.kind {
        WidgetKind::Panel(_) | WidgetKind::Grid(_) | WidgetKind::Spacer => {}
        // `no_wrap`: always a single line, even wider than the box — it
        // overflows horizontally (still centered) rather than wrapping.
        // No `.max(0.0)` when centered: wrapped lines never exceed the box
        // so this never goes negative for them; a no_wrap line wider than
        // the box legitimately needs a negative offset to stay centered.
        WidgetKind::Label(t) => draw_lines(d2d, t, s, r, clip, |free| match s.align {
            crate::Align::Center => free / 2.0,
            crate::Align::End => free.max(0.0),
            crate::Align::Start => 0.0,
        }),
        // Each line centered horizontally inside the button.
        WidgetKind::Button(t) => draw_button_text(d2d, t, s, r, clip),
        WidgetKind::ItemSlot(item_id) => {
            let at = Vec2::new(r.x + s.pad[3], r.y + s.pad[0]);
            if fits(clip, at.y, SLOT_SIZE) {
                item_slot(d2d, item_id, at);
            }
        }
        WidgetKind::InvSlot { item_id, count } => {
            let at = Vec2::new(r.x + s.pad[3], r.y + s.pad[0]);
            if fits(clip, at.y, SLOT_SIZE) {
                inv_slot(d2d, item_id, *count, at);
            }
        }
        WidgetKind::Image(image) => crate::image::render(d2d, image, r.inset(s.pad), clip),
        WidgetKind::Surface(id) => {
            let at = r.inset(s.pad);
            match crate::surface::get(id) {
                Some(surface) => surface.paint(gfx, at),
                None => unpainted_surface(d2d, id, at),
            }
        }
        WidgetKind::TextBox(tb) => render_text_box(d2d, tb, s, node, clip),
    }
}

/// The square a slot's item sits in, and the item's corner (1 px in).
fn slot_square(at: Vec2) -> (Rect, Vec2) {
    (
        Rect::new(at.x, at.y, SLOT_SIZE, SLOT_SIZE),
        Vec2::new(at.x + 1.0, at.y + 1.0),
    )
}

/// An item slot: a framed square and the item (the item renderer:
/// modded items, 3D block items, any version).
fn item_slot(d2d: &Draw2D, item_id: &str, at: Vec2) {
    let (square, icon) = slot_square(at);
    d2d.rect(square, ITEM_SLOT_BORDER);
    d2d.rect(square.inset([1.0; 4]), ITEM_SLOT_FACE);
    if !item_id.is_empty() {
        d2d.item(item_id, icon, ICON_SIZE);
    }
}

/// Vanilla's inventory slot: sunk (shadow top-left, light bottom-right),
/// the item and its count.
fn inv_slot(d2d: &Draw2D, item_id: &str, count: u32, at: Vec2) {
    let (square, icon) = slot_square(at);
    let Rect { x, y, w, h } = square;
    d2d.rect(square, INV_SLOT_FACE);
    d2d.rect(Rect::new(x, y, w, 1.0), INV_SLOT_SHADOW);
    d2d.rect(Rect::new(x, y + h - 1.0, w, 1.0), INV_SLOT_LIGHT);
    d2d.rect(Rect::new(x, y, 1.0, h), INV_SLOT_SHADOW);
    d2d.rect(Rect::new(x + w - 1.0, y, 1.0, h), INV_SLOT_LIGHT);
    if item_id.is_empty() {
        return;
    }
    d2d.item(item_id, icon, ICON_SIZE);
    if count > 1 {
        let shown = count.to_string();
        let cw = text::str_width(&shown, 1.0);
        d2d.text(
            &shown,
            Vec2::new(icon.x + COUNT_RIGHT - cw, icon.y + COUNT_TOP),
            COUNT_COLOR,
            true,
        );
    }
}

/// A surface nothing's registered to draw (a designer previewing the
/// markup, a mod that didn't): its place and its name.
fn unpainted_surface(d2d: &Draw2D, id: &str, at: Rect) {
    d2d.rect(at, SURFACE_FACE);
    outline(d2d, None, at, SURFACE_EDGE);
    let label = format!("Surface {id}");
    let w = d2d.text_width(&label, "");
    if w < at.w - SURFACE_NAME_ROOM && at.h >= text::LINE_H + 1.0 {
        d2d.text(
            &label,
            Vec2::new(at.x + (at.w - w) / 2.0, at.y + (at.h - text::GLYPH_H) / 2.0),
            SURFACE_NAME,
            false,
        );
    }
}

/// Draws a label's / button's text lines inside `r` (less padding), the
/// block centered vertically and each line `place(free width)` from the left.
/// A button's text: centred like vanilla's buttons — one line at
/// `y + (h - GLYPH_H) / 2` in whole pixels (the glyphs', not the line's), so it
/// keeps clear of the frame; several lines centred as a block.
fn draw_button_text(d2d: &Draw2D, t: &str, s: &Style, r: Rect, clip: Option<Rect>) {
    let avail_w = (r.w - s.pad[1] - s.pad[3]).max(0.0);
    let lines = text_lines(t, s, avail_w);
    if lines.len() != 1 || s.font_scale != 1.0 || !text::is_default_font(&s.font) {
        draw_lines(d2d, t, s, r, clip, |free| (free / 2.0).max(0.0));
        return;
    }
    let content_h = r.h - s.pad[0] - s.pad[2];
    let y = r.y + s.pad[0] + ((content_h - text::GLYPH_H) / 2.0).max(0.0).floor();
    let line = &lines[0];
    if !fits(clip, y, text::LINE_H) {
        return;
    }
    let ink_w = (text::str_width_in(line, 1.0, &s.font) - 1.0).max(0.0);
    let x = r.x + s.pad[3] + ((avail_w - ink_w) / 2.0).max(0.0);
    draw_text(d2d, line, Vec2::new(x, y), &style_of(s, 1.0));
}

fn draw_lines(
    d2d: &Draw2D,
    t: &str,
    s: &Style,
    r: Rect,
    clip: Option<Rect>,
    place: impl Fn(f32) -> f32,
) {
    let avail_w = (r.w - s.pad[1] - s.pad[3]).max(0.0);
    let lines = text_lines(t, s, avail_w);
    let line_h = text::LINE_H * s.font_scale + text::LINE_GAP;
    let total_h = lines.len() as f32 * line_h - text::LINE_GAP;
    let content_h = r.h - s.pad[0] - s.pad[2];
    let start_y = r.y + s.pad[0] + ((content_h - total_h) / 2.0).max(0.0);
    // A span going on past a line goes on into the next: each line is
    // drawn on its own, its spans closed and the next one's reopened.
    let mut open: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut shown = format!("{}{line}", crate::rich::reopen(&open));
        open = crate::rich::open_spans(&shown);
        shown.push_str(&"}".repeat(open.len()));
        let ly = start_y + i as f32 * line_h;
        if !fits(clip, ly, text::LINE_H * s.font_scale) {
            continue;
        }
        // Placed by the ink: a Minecraft font's advances end in a pixel of
        // spacing after the last glyph, which would pull centred text half
        // a pixel left (a lone `i` in a 9-pixel badge sits off-centre).
        let mut line_w = text::str_width_in(&shown, s.font_scale, &s.font);
        if text::is_default_font(&s.font) && line_w > 0.0 {
            line_w -= s.font_scale;
        }
        let tx = r.x + s.pad[3] + place(avail_w - line_w);
        draw_text(d2d, &shown, Vec2::new(tx, ly), &style_of(s, s.font_scale));
    }
}

/// Text in `style` at `at` — the plain call for the default font at its size.
fn draw_text(d2d: &Draw2D, line: &str, at: Vec2, style: &TextStyle<'_>) {
    if style.scale != 1.0 {
        d2d.text_scaled(line, at, style);
    } else if text::is_default_font(style.font) {
        d2d.text(line, at, style.color, style.shadow);
    } else {
        d2d.text_font(line, style.font, at, style.color, style.shadow);
    }
}

/// A widget's text style at `scale`.
fn style_of(s: &Style, scale: f32) -> TextStyle<'_> {
    TextStyle {
        font: &s.font,
        scale,
        color: s.color,
        shadow: s.text_shadow,
    }
}

/// A text box: border, then the visible part of the text (or the
/// placeholder), the selection behind it and the caret.
fn render_text_box(
    d2d: &Draw2D,
    tb: &widget::TextBox,
    s: &Style,
    node: &LayoutNode,
    clip: Option<Rect>,
) {
    let r = node.rect;
    let border = match (node.focused, s.focus_color != Color::TRANSPARENT) {
        (true, true) => s.focus_color,
        (true, false) => widget::TEXT_BOX_FOCUS_BORDER,
        (false, _) => widget::TEXT_BOX_BORDER,
    };
    outline(d2d, clip, r, border);
    let fs = s.font_scale;
    let line_h = text::LINE_H * fs;
    let content_h = r.h - s.pad[0] - s.pad[2];
    // Inside the 1 px border and the padding.
    let line = TextLine {
        x0: r.x + 1.0 + s.pad[3],
        y: (r.y + s.pad[0] + ((content_h - line_h) / 2.0).max(0.0)).round(),
        w: (r.w - 2.0 - s.pad[1] - s.pad[3]).max(0.0),
        h: line_h,
    };
    if !fits(clip, line.y, line_h) {
        return;
    }
    if tb.text.is_empty() {
        if !tb.placeholder.is_empty() {
            let shown = ellipsize(&tb.placeholder, line.w, fs, &s.font);
            let placeholder = TextStyle {
                color: widget::PLACEHOLDER_COLOR,
                shadow: false,
                ..style_of(s, fs)
            };
            draw_text(d2d, &shown, Vec2::new(line.x0, line.y), &placeholder);
        }
    } else {
        text_box_text(d2d, tb, s, &line, clip);
    }
    if let Some(caret) = tb.caret
        && tb.caret_visible
    {
        let cw = |c: char| text::char_width_in(c, &s.font) * fs;
        let first = crate::textbox::text_box_first(&tb.text, tb.first, caret, line.w, cw);
        let cx = line.x0
            + tb.text
                .chars()
                .skip(first)
                .take(caret.saturating_sub(first))
                .map(cw)
                .sum::<f32>();
        let cx = cx.min(line.x0 + line.w);
        fill(
            d2d,
            clip,
            Rect::from_edges(cx, line.y - 1.0, cx + 1.0, line.y + line.h),
            s.color,
        );
    }
}

/// Where a text box's line is: its left, top, width and height.
struct TextLine {
    x0: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// A text box's text: what fits from its first shown character, the
/// selection behind it.
fn text_box_text(
    d2d: &Draw2D,
    tb: &widget::TextBox,
    s: &Style,
    line: &TextLine,
    clip: Option<Rect>,
) {
    let fs = s.font_scale;
    let cw = |c: char| text::char_width_in(c, &s.font) * fs;
    let caret = tb.caret.unwrap_or(0);
    let first = crate::textbox::text_box_first(&tb.text, tb.first, caret, line.w, cw);
    // The characters that fit, from `first`.
    let mut xs = vec![line.x0];
    let mut visible = String::new();
    for c in tb.text.chars().skip(first) {
        let next = xs[xs.len() - 1] + cw(c);
        if next > line.x0 + line.w + CLIP_SLACK {
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
        fill(
            d2d,
            clip,
            Rect::from_edges(sx0, line.y - 1.0, sx1, line.y + line.h),
            widget::SELECTION_COLOR,
        );
    }
    draw_text(d2d, &visible, Vec2::new(line.x0, line.y), &style_of(s, fs));
}

/// The lines a label/button draws: wrapped to `avail_w`, or one line
/// (`no_wrap`), cut with `...` when `ellipsis` is set.
fn text_lines(t: &str, s: &Style, avail_w: f32) -> Vec<String> {
    if s.ellipsis {
        vec![ellipsize(t, avail_w, s.font_scale, &s.font)]
    } else if s.no_wrap {
        t.split('\n').map(str::to_owned).collect()
    } else {
        text::wrap_text_in(t, avail_w, s.font_scale, &s.font)
    }
}

/// Vanilla's tooltip box: near-black fill, a purple gradient frame, white
/// text — 12px right of and above the cursor, flipped to stay on screen.
pub(crate) fn tooltip(d2d: &Draw2D, text: &str, mx: f32, my: f32, sw: f32, sh: f32) {
    const BG: Color = Color(0xF0_100010);
    const BORDER_TOP: Color = Color(0x50_5000FF);
    const BORDER_BOTTOM: Color = Color(0x50_28007F);
    const TEXT: Color = Color(0xFF_FFFFFF);
    /// The text's corner from the cursor (right and up), and from it when
    /// flipped to the cursor's left.
    const OFFSET: f32 = 12.0;
    const FLIPPED: f32 = 16.0;
    /// The least room kept to the screen's edges.
    const MARGIN: f32 = 4.0;
    /// The box around the text.
    const PAD: f32 = 3.0;
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }
    let w = lines
        .iter()
        .map(|l| crate::text::str_width(l, 1.0))
        .fold(0.0, f32::max);
    let line_h = crate::text::LINE_H + crate::text::LINE_GAP;
    let h = lines.len() as f32 * line_h - crate::text::LINE_GAP;
    let mut x = mx + OFFSET;
    if x + w + MARGIN > sw {
        x = (mx - FLIPPED - w).max(MARGIN);
    }
    let y = (my - OFFSET).clamp(MARGIN, (sh - h - MARGIN).max(MARGIN));
    let (x0, y0, x1, y1) = (x - PAD, y - PAD, x + w + PAD, y + h + PAD);
    d2d.rect(Rect::from_edges(x0, y0 - 1.0, x1, y0), BG);
    d2d.rect(Rect::from_edges(x0, y1, x1, y1 + 1.0), BG);
    d2d.rect(Rect::from_edges(x0, y0, x1, y1), BG);
    d2d.rect(Rect::from_edges(x0 - 1.0, y0, x0, y1), BG);
    d2d.rect(Rect::from_edges(x1, y0, x1 + 1.0, y1), BG);
    d2d.gradient(
        Rect::from_edges(x0, y0 + 1.0, x0 + 1.0, y1 - 1.0),
        BORDER_TOP,
        BORDER_BOTTOM,
    );
    d2d.gradient(
        Rect::from_edges(x1 - 1.0, y0 + 1.0, x1, y1 - 1.0),
        BORDER_TOP,
        BORDER_BOTTOM,
    );
    d2d.rect(Rect::from_edges(x0, y0, x1, y0 + 1.0), BORDER_TOP);
    d2d.rect(Rect::from_edges(x0, y1 - 1.0, x1, y1), BORDER_BOTTOM);
    for (i, line) in lines.iter().enumerate() {
        d2d.text(line, Vec2::new(x, y + i as f32 * line_h), TEXT, true);
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
