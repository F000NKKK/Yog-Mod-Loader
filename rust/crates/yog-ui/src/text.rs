//! Text wrapping utilities shared by layout (measuring) and render (drawing).
//!
//! Widths use Minecraft's default (ASCII) font advance table so that
//! wrapping and centering match what the MC text renderer actually draws.
//! Other fonts (`Style::font`, e.g. `"minecraft:uniform"` or a resource-pack
//! font) are measured by the host's text renderer once [`use_host_metrics`]
//! has been called (`UiRoot::render` does it); before that, and outside a
//! game, they fall back to the default font's table.

use std::collections::HashMap;
use std::sync::Mutex;

use yog_gfx::{GfxContext, TextWidthFn};

/// Approximate pixel width per character at font_scale=1.0 (fallback for
/// non-ASCII chars and legacy callers).
pub const CHAR_W: f32 = 6.0;
/// Line height at font_scale=1.0 (MC font is 9px; Patchouli uses 9-10px lines).
pub const LINE_H: f32 = 9.0;
/// Gap between wrapped lines.
pub const LINE_GAP: f32 = 1.0;
/// Minecraft's default font id — what an empty `Style::font` means.
pub const DEFAULT_FONT: &str = "minecraft:default";

/// Advance width in pixels (including 1px inter-glyph spacing) of one char
/// in Minecraft's default ASCII font at GUI scale 1.
pub fn char_width(c: char) -> f32 {
    match c {
        ' ' => 4.0,
        '!' | ',' | '.' | ':' | ';' | '|' | '\'' => 2.0,
        'i' => 2.0,
        'l' => 3.0,
        '`' => 3.0,
        '(' | ')' | '*' | '<' | '>' | '{' | '}' => 5.0,
        'f' | 'k' => 5.0,
        '"' => 5.0,
        't' | '[' | ']' | 'I' => 4.0,
        '@' | '~' => 7.0,
        c if c.is_ascii() => 6.0,
        _ => 6.0, // non-ASCII: MC unicode glyphs vary; 6 is a fair average
    }
}

/// Whether `font` means Minecraft's default font (empty or `minecraft:default`).
pub fn is_default_font(font: &str) -> bool {
    font.is_empty() || font == DEFAULT_FONT
}

struct HostMetrics {
    width: Option<TextWidthFn>,
    /// `(font, char)` → advance; MC advances are additive, so per-char
    /// widths sum to the string's width.
    cache: Option<HashMap<(String, char), f32>>,
}

static HOST: Mutex<HostMetrics> = Mutex::new(HostMetrics {
    width: None,
    cache: None,
});

/// Lets non-default fonts be measured by the host's text renderer (see the
/// module docs). Called by `UiRoot::render`; call it yourself before the
/// first layout to get exact metrics from the very first frame.
pub fn use_host_metrics(ctx: &GfxContext) {
    if let Ok(mut h) = HOST.lock() {
        h.width = Some(ctx.text_width_fn());
    }
}

/// Advance width of `c` in `font` at scale 1 — the default font's table, or
/// the host's measurement (cached) for any other font.
pub fn char_width_in(c: char, font: &str) -> f32 {
    if is_default_font(font) {
        return char_width(c);
    }
    let Ok(mut h) = HOST.lock() else {
        return char_width(c);
    };
    let Some(width) = h.width else {
        return char_width(c);
    };
    let cache = h.cache.get_or_insert_with(HashMap::new);
    if let Some(w) = cache.get(&(font.to_owned(), c)) {
        return *w;
    }
    let mut buf = [0u8; 4];
    let s: &str = c.encode_utf8(&mut buf);
    let w = unsafe { width(yog_abi::YogStr::from_str(s), yog_abi::YogStr::from_str(font)) };
    let cache = h.cache.get_or_insert_with(HashMap::new);
    if cache.len() > 8192 {
        cache.clear();
    }
    cache.insert((font.to_owned(), c), w);
    w
}

/// Pixel width of a string at the given font scale.
pub fn str_width(s: &str, font_scale: f32) -> f32 {
    s.chars().map(char_width).sum::<f32>() * font_scale
}

/// [`str_width`] in `font` (see [`char_width_in`]).
pub fn str_width_in(s: &str, font_scale: f32, font: &str) -> f32 {
    if is_default_font(font) {
        return str_width(s, font_scale);
    }
    s.chars().map(|c| char_width_in(c, font)).sum::<f32>() * font_scale
}

/// Break `text` into lines that fit within `max_w` pixels at `font_scale`.
/// Width accounting is per-glyph (MC default font advances).
pub fn wrap_text(text: &str, max_w: f32, font_scale: f32) -> Vec<String> {
    wrap_text_in(text, max_w, font_scale, "")
}

/// [`wrap_text`] in `font` (see [`char_width_in`]).
pub fn wrap_text_in(text: &str, max_w: f32, font_scale: f32, font: &str) -> Vec<String> {
    if font_scale <= 0.0 || max_w <= 0.0 {
        return vec![text.to_owned()];
    }
    let space_w = char_width_in(' ', font) * font_scale;
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0f32;

    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }
        let word_w = str_width_in(word, font_scale, font);

        if cur_w == 0.0 {
            append_word(
                &mut lines, &mut cur, &mut cur_w, word, word_w, max_w, font_scale, font,
            );
        } else if cur_w + space_w + word_w <= max_w {
            cur.push(' ');
            cur.push_str(word);
            cur_w += space_w + word_w;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
            append_word(
                &mut lines, &mut cur, &mut cur_w, word, word_w, max_w, font_scale, font,
            );
        }
    }
    if cur_w > 0.0 || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Append a word, hard-breaking if it's wider than one line.
#[allow(clippy::too_many_arguments)]
fn append_word(
    lines: &mut Vec<String>,
    cur: &mut String,
    cur_w: &mut f32,
    word: &str,
    word_w: f32,
    max_w: f32,
    font_scale: f32,
    font: &str,
) {
    if word_w <= max_w {
        cur.push_str(word);
        *cur_w = word_w;
        return;
    }
    // Word is wider than one line — hard-break at glyph boundaries.
    let mut line = String::new();
    let mut w = 0.0f32;
    for ch in word.chars() {
        let cw = char_width_in(ch, font) * font_scale;
        if w + cw > max_w && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            w = 0.0;
        }
        line.push(ch);
        w += cw;
    }
    *cur = line;
    *cur_w = w;
}

/// Number of lines that `text` wraps to.
pub fn line_count(text: &str, max_w: f32, font_scale: f32) -> usize {
    wrap_text(text, max_w, font_scale).len().max(1)
}

/// Total pixel height of wrapped `text`.
pub fn text_height(text: &str, max_w: f32, font_scale: f32) -> f32 {
    text_height_in(text, max_w, font_scale, "")
}

/// [`text_height`] in `font` (see [`char_width_in`]).
pub fn text_height_in(text: &str, max_w: f32, font_scale: f32, font: &str) -> f32 {
    let n = wrap_text_in(text, max_w, font_scale, font).len().max(1);
    n as f32 * LINE_H * font_scale + (n.saturating_sub(1)) as f32 * LINE_GAP
}

/// Split `text` into page-sized chunks that fit within `max_h` pixels.
/// Each page is a `Vec<String>` of ready-to-render lines.
pub fn paginate_text(text: &str, max_w: f32, max_h: f32, font_scale: f32) -> Vec<Vec<String>> {
    let all_lines: Vec<String> = text
        .split('\n')
        .flat_map(|para| {
            if para.is_empty() {
                vec![String::new()]
            } else {
                wrap_text(para, max_w, font_scale)
            }
        })
        .collect();

    let line_h = LINE_H * font_scale + LINE_GAP;
    let per_page = ((max_h + LINE_GAP) / line_h).floor() as usize;
    let per_page = per_page.max(1);

    all_lines.chunks(per_page).map(|c| c.to_vec()).collect()
}
