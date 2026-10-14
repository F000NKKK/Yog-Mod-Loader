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

/// Most `(font, char)` advances cached before the cache starts over.
const ADVANCE_CACHE_CAP: usize = 8192;

struct HostMetrics {
    width: Option<TextWidthFn>,
    /// font → char → advance; MC advances are additive, so per-char
    /// widths sum to the string's width.
    cache: Option<HashMap<String, HashMap<char, f32>>>,
    /// Advances in `cache`, all fonts together.
    cached: usize,
}

impl HostMetrics {
    /// Advance of `c` in the non-default `font` as the host measures it
    /// (cached); `None` without a host.
    fn advance(&mut self, c: char, font: &str) -> Option<f32> {
        let width = self.width?;
        let cache = self.cache.get_or_insert_with(HashMap::new);
        if let Some(w) = cache.get(font).and_then(|m| m.get(&c)) {
            return Some(*w);
        }
        let mut buf = [0u8; 4];
        let s: &str = c.encode_utf8(&mut buf);
        let w = unsafe {
            width(
                yog_abi::YogStr::from_str(s),
                yog_abi::YogStr::from_str(font),
            )
        };
        if self.cached > ADVANCE_CACHE_CAP {
            cache.clear();
            self.cached = 0;
        }
        cache.entry(font.to_owned()).or_default().insert(c, w);
        self.cached += 1;
        Some(w)
    }
}

static HOST: Mutex<HostMetrics> = Mutex::new(HostMetrics {
    width: None,
    cache: None,
    cached: 0,
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
    HOST.lock()
        .ok()
        .and_then(|mut h| h.advance(c, font))
        .unwrap_or_else(|| char_width(c))
}

/// Pixel width of a string at the given font scale.
pub fn str_width(s: &str, font_scale: f32) -> f32 {
    crate::rich::width(s, char_width) * font_scale
}

/// [`str_width`] in `font` (see [`char_width_in`]).
pub fn str_width_in(s: &str, font_scale: f32, font: &str) -> f32 {
    if is_default_font(font) {
        return str_width(s, font_scale);
    }
    let Ok(mut h) = HOST.lock() else {
        return str_width(s, font_scale);
    };
    crate::rich::width(s, |c| h.advance(c, font).unwrap_or_else(|| char_width(c))) * font_scale
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
    let mut lines = Lines::default();
    wrap(text, max_w, font_scale, font, &mut lines);
    lines.0
}

/// Where [`wrap`] puts the text: the pieces of the current line and the
/// line breaks.
trait WrapSink {
    fn push_str(&mut self, s: &str);
    fn push_char(&mut self, c: char);
    /// Ends the current line.
    fn end_line(&mut self);
}

/// Collects the wrapped lines.
#[derive(Default)]
struct Lines(Vec<String>, String);

impl WrapSink for Lines {
    fn push_str(&mut self, s: &str) {
        self.1.push_str(s);
    }
    fn push_char(&mut self, c: char) {
        self.1.push(c);
    }
    fn end_line(&mut self) {
        self.0.push(std::mem::take(&mut self.1));
    }
}

/// Counts the wrapped lines without building them.
#[derive(Default)]
struct LineCount(usize);

impl WrapSink for LineCount {
    fn push_str(&mut self, _: &str) {}
    fn push_char(&mut self, _: char) {}
    fn end_line(&mut self) {
        self.0 += 1;
    }
}

/// The word-wrapping behind [`wrap_text_in`]: each `\n`-separated
/// paragraph on lines of its own (see [`wrap_paragraph`]).
fn wrap(text: &str, max_w: f32, font_scale: f32, font: &str, out: &mut impl WrapSink) {
    for para in text.split('\n') {
        wrap_paragraph(para, max_w, font_scale, font, out);
    }
}

/// One paragraph: words joined by single spaces, a word wider than a line
/// hard-broken at glyph boundaries. Always ends at least one line.
fn wrap_paragraph(text: &str, max_w: f32, font_scale: f32, font: &str, out: &mut impl WrapSink) {
    let space_w = char_width_in(' ', font) * font_scale;
    let mut lines = 0usize;
    let mut cur_w = 0.0f32;

    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }
        let word_w = str_width_in(word, font_scale, font);
        if cur_w > 0.0 && cur_w + space_w + word_w <= max_w {
            out.push_char(' ');
            out.push_str(word);
            cur_w += space_w + word_w;
            continue;
        }
        if cur_w > 0.0 {
            out.end_line();
            lines += 1;
        }
        if word_w <= max_w {
            out.push_str(word);
            cur_w = word_w;
            continue;
        }
        // Word is wider than one line — hard-break at glyph boundaries.
        let (mut w, mut line_empty) = (0.0f32, true);
        let mut code = false;
        let mut opening = false;
        for ch in word.chars() {
            // Markup takes no room: a `§` code, a span's `{attrs|` and `}`.
            if code || ch == '§' {
                code = ch == '§';
                out.push_char(ch);
                continue;
            }
            if opening || ch == '{' || ch == '}' {
                opening = (opening || ch == '{') && ch != '|';
                out.push_char(ch);
                continue;
            }
            let cw = char_width_in(ch, font) * font_scale;
            if w + cw > max_w && !line_empty {
                out.end_line();
                lines += 1;
                w = 0.0;
            }
            out.push_char(ch);
            line_empty = false;
            w += cw;
        }
        cur_w = w;
    }
    if cur_w > 0.0 || lines == 0 {
        out.end_line();
    }
}

/// Number of lines that `text` wraps to.
pub fn line_count(text: &str, max_w: f32, font_scale: f32) -> usize {
    line_count_in(text, max_w, font_scale, "")
}

/// [`line_count`] in `font`, without building the lines.
fn line_count_in(text: &str, max_w: f32, font_scale: f32, font: &str) -> usize {
    if font_scale <= 0.0 || max_w <= 0.0 {
        return 1;
    }
    let mut count = LineCount::default();
    wrap(text, max_w, font_scale, font, &mut count);
    count.0
}

/// Total pixel height of wrapped `text`.
pub fn text_height(text: &str, max_w: f32, font_scale: f32) -> f32 {
    text_height_in(text, max_w, font_scale, "")
}

/// [`text_height`] in `font` (see [`char_width_in`]).
pub fn text_height_in(text: &str, max_w: f32, font_scale: f32, font: &str) -> f32 {
    let n = line_count_in(text, max_w, font_scale, font);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words_and_hard_breaks_long_ones() {
        assert_eq!(wrap_text("aaa  bbb ccc", 40.0, 1.0), ["aaa bbb", "ccc"]);
        assert_eq!(wrap_text("", 40.0, 1.0), [""]);
        assert_eq!(wrap_text("aaa\nbbb", 400.0, 1.0), ["aaa", "bbb"]);
        assert_eq!(wrap_text("aaa\n\nbbb", 400.0, 1.0), ["aaa", "", "bbb"]);
        assert_eq!(str_width("§5ab§r", 1.0), str_width("ab", 1.0), "codes take no room");
        assert_eq!(str_width("§lab", 1.0), str_width("ab", 1.0) + 2.0, "bold is wider");

        assert_eq!(wrap_text("mmmmmmmmmm", 30.0, 1.0), ["mmmmm", "mmmmm"]);
        assert_eq!(
            wrap_text("a mmmmmmmmmm b", 30.0, 1.0),
            ["a", "mmmmm", "mmmmm", "b"]
        );
    }

    #[test]
    fn line_count_matches_the_wrapped_lines() {
        let text = "The quick brown fox jumps over a supercalifragilistic dog";
        for w in [0.0, 5.0, 20.0, 50.0, 116.0, 1000.0] {
            for scale in [0.0, 0.5, 1.0, 2.0] {
                let lines = if scale <= 0.0 || w <= 0.0 {
                    1
                } else {
                    wrap_text(text, w, scale).len()
                };
                assert_eq!(line_count(text, w, scale), lines, "w={w} scale={scale}");
            }
        }
    }
}
