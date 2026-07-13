//! Text wrapping utilities shared by layout (measuring) and render (drawing).
//!
//! Widths use Minecraft's default (ASCII) font advance table so that
//! wrapping and centering match what the MC text renderer actually draws.

/// Approximate pixel width per character at font_scale=1.0 (fallback for
/// non-ASCII chars and legacy callers).
pub const CHAR_W: f32 = 6.0;
/// Line height at font_scale=1.0 (MC font is 9px; Patchouli uses 9-10px lines).
pub const LINE_H: f32 = 9.0;
/// Gap between wrapped lines.
pub const LINE_GAP: f32 = 1.0;

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

/// Pixel width of a string at the given font scale.
pub fn str_width(s: &str, font_scale: f32) -> f32 {
    s.chars().map(char_width).sum::<f32>() * font_scale
}

/// Break `text` into lines that fit within `max_w` pixels at `font_scale`.
/// Width accounting is per-glyph (MC default font advances).
pub fn wrap_text(text: &str, max_w: f32, font_scale: f32) -> Vec<String> {
    if font_scale <= 0.0 || max_w <= 0.0 {
        return vec![text.to_owned()];
    }
    let space_w = char_width(' ') * font_scale;
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0f32;

    for word in text.split(' ') {
        if word.is_empty() {
            continue;
        }
        let word_w = str_width(word, font_scale);

        if cur_w == 0.0 {
            append_word(
                &mut lines, &mut cur, &mut cur_w, word, word_w, max_w, font_scale,
            );
        } else if cur_w + space_w + word_w <= max_w {
            cur.push(' ');
            cur.push_str(word);
            cur_w += space_w + word_w;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
            append_word(
                &mut lines, &mut cur, &mut cur_w, word, word_w, max_w, font_scale,
            );
        }
    }
    if cur_w > 0.0 || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Append a word, hard-breaking if it's wider than one line.
fn append_word(
    lines: &mut Vec<String>,
    cur: &mut String,
    cur_w: &mut f32,
    word: &str,
    word_w: f32,
    max_w: f32,
    font_scale: f32,
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
        let cw = char_width(ch) * font_scale;
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
    let n = line_count(text, max_w, font_scale);
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
