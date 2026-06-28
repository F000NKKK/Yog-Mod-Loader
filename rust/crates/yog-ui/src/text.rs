//! Text wrapping utilities shared by layout (measuring) and render (drawing).

/// Approximate pixel width per character at font_scale=1.0 in Minecraft's default font.
pub const CHAR_W: f32 = 6.0;
/// Line height at font_scale=1.0.
pub const LINE_H: f32 = 10.0;
/// Gap between wrapped lines.
pub const LINE_GAP: f32 = 2.0;

/// Break `text` into lines that fit within `max_w` pixels at `font_scale`.
/// Words that are longer than one line get hard-broken at the character boundary.
pub fn wrap_text(text: &str, max_w: f32, font_scale: f32) -> Vec<String> {
    let char_w = CHAR_W * font_scale;
    if char_w <= 0.0 || max_w <= 0.0 {
        return vec![text.to_owned()];
    }
    let max_chars = ((max_w / char_w).floor() as usize).max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();

    for word in text.split(' ') {
        if word.is_empty() { continue; }
        if cur.is_empty() {
            // First word on a line — may still need hard-breaking.
            let mut w = word;
            while w.len() > max_chars {
                lines.push(w[..max_chars].to_owned());
                w = &w[max_chars..];
            }
            cur.push_str(w);
        } else if cur.len() + 1 + word.len() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            let mut w = word;
            while w.len() > max_chars {
                lines.push(w[..max_chars].to_owned());
                w = &w[max_chars..];
            }
            cur.push_str(w);
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Number of lines that `text` wraps to, at the given scale in `max_w` pixels.
pub fn line_count(text: &str, max_w: f32, font_scale: f32) -> usize {
    wrap_text(text, max_w, font_scale).len().max(1)
}

/// Total height (px) of wrapped `text`.
pub fn text_height(text: &str, max_w: f32, font_scale: f32) -> f32 {
    let n = line_count(text, max_w, font_scale);
    n as f32 * LINE_H * font_scale + (n.saturating_sub(1)) as f32 * LINE_GAP
}
