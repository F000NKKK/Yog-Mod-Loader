//! Text wrapping utilities shared by layout (measuring) and render (drawing).

/// Approximate pixel width per character at font_scale=1.0 in Minecraft's default font.
pub const CHAR_W: f32 = 6.0;
/// Line height at font_scale=1.0.
pub const LINE_H: f32 = 10.0;
/// Gap between wrapped lines.
pub const LINE_GAP: f32 = 2.0;

/// Break `text` into lines that fit within `max_w` pixels at `font_scale`.
/// All comparisons are char-count based (Unicode-safe).
pub fn wrap_text(text: &str, max_w: f32, font_scale: f32) -> Vec<String> {
    let char_w = CHAR_W * font_scale;
    if char_w <= 0.0 || max_w <= 0.0 {
        return vec![text.to_owned()];
    }
    let max_chars = ((max_w / char_w).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_chars = 0usize;

    for word in text.split(' ') {
        if word.is_empty() { continue; }
        let word_chars = word.chars().count();

        if cur_chars == 0 {
            append_word(&mut lines, &mut cur, &mut cur_chars, word, word_chars, max_chars);
        } else if cur_chars + 1 + word_chars <= max_chars {
            cur.push(' ');
            cur.push_str(word);
            cur_chars += 1 + word_chars;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur_chars = 0;
            append_word(&mut lines, &mut cur, &mut cur_chars, word, word_chars, max_chars);
        }
    }
    if cur_chars > 0 || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Append a word, hard-breaking if it's longer than max_chars.
fn append_word(
    lines: &mut Vec<String>, cur: &mut String, cur_chars: &mut usize,
    word: &str, word_chars: usize, max_chars: usize,
) {
    if word_chars <= max_chars {
        cur.push_str(word);
        *cur_chars = word_chars;
        return;
    }
    // Word is longer than one line — hard-break at char boundaries.
    let mut remaining = word;
    let mut rem_chars = word_chars;
    while rem_chars > max_chars {
        let split = char_boundary(remaining, max_chars);
        lines.push(remaining[..split].to_owned());
        remaining = &remaining[split..];
        rem_chars -= max_chars;
    }
    cur.push_str(remaining);
    *cur_chars = rem_chars;
}

/// Byte index of the `n`-th char boundary in `s` (Unicode-safe).
fn char_boundary(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
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
    let all_lines: Vec<String> = text.split('\n').flat_map(|para| {
        if para.is_empty() { vec![String::new()] } else { wrap_text(para, max_w, font_scale) }
    }).collect();

    let line_h = LINE_H * font_scale + LINE_GAP;
    let per_page = ((max_h + LINE_GAP) / line_h).floor() as usize;
    let per_page = per_page.max(1);

    all_lines.chunks(per_page).map(|c| c.to_vec()).collect()
}
