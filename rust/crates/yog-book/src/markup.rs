//! Book text markup — a small Markdown, compiled into Yog rich text
//! (`yog_ui::rich`) when the book is prepared:
//!
//! | Write | Shows |
//! |---|---|
//! | `**bold**` | bold |
//! | `*italic*` | italic |
//! | `==term==` | a term, in the book's accent color |
//! | `[text](entry)` | a link: underlined, in the link color |
//! | `- item` at a line's start | `• item` |
//! | a blank line | a new paragraph |
//! | `{#74b3f2\|text}`, `{b\|…}` | rich text, as it is |
//! | `\*` | a literal `*` (any character after `\`) |

/// Links' color.
const LINK: &str = "#2f5fbf";

/// `text` with its markup as rich text; `accent` (`#rrggbb`) colors terms.
pub fn to_rich(text: &str, accent: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    // Open markers, innermost last: "**", "*", "==".
    let mut open: Vec<&'static str> = Vec::new();
    let toggle = |out: &mut String, open: &mut Vec<&'static str>, m: &'static str, span: String| {
        if let Some(at) = open.iter().rposition(|x| *x == m) {
            // Close it (and anything opened inside it).
            for _ in at..open.len() {
                out.push('}');
            }
            open.truncate(at);
        } else {
            out.push('{');
            out.push_str(&span);
            out.push('|');
            open.push(m);
        }
    };
    let lines: Vec<&str> = text.split('\n').collect();
    for (n, line) in lines.iter().enumerate() {
        let line = match line.strip_prefix("- ") {
            Some(rest) => {
                out.push_str("• ");
                rest
            }
            None => line,
        };
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            match c {
                '\\' if next.is_some() => {
                    out.push('\\');
                    out.push(next.unwrap_or_default());
                    i += 2;
                    continue;
                }
                '*' if next == Some('*') => {
                    toggle(&mut out, &mut open, "**", "b".into());
                    i += 2;
                    continue;
                }
                '*' => toggle(&mut out, &mut open, "*", "i".into()),
                '=' if next == Some('=') => {
                    toggle(&mut out, &mut open, "==", accent.to_string());
                    i += 2;
                    continue;
                }
                '[' => {
                    if let Some(close) = chars[i..].iter().position(|&x| x == ']').map(|p| p + i)
                        && chars.get(close + 1) == Some(&'(')
                        && let Some(end) = chars[close..].iter().position(|&x| x == ')').map(|p| p + close)
                    {
                        out.push_str(&format!("{{u {LINK}|"));
                        out.extend(&chars[i + 1..close]);
                        out.push('}');
                        i = end + 1;
                        continue;
                    }
                    out.push(c);
                }
                c => out.push(c),
            }
            i += 1;
        }
        if n + 1 < lines.len() {
            out.push('\n');
        }
    }
    for _ in &open {
        out.push('}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "#8d6acc";

    #[test]
    fn compiles_to_rich_text() {
        assert_eq!(to_rich("plain", A), "plain");
        assert_eq!(to_rich("a **b** c", A), "a {b|b} c");
        assert_eq!(to_rich("==Media== is *mind*", A), "{#8d6acc|Media} is {i|mind}");
        assert_eq!(to_rich("**==both==**", A), "{b|{#8d6acc|both}}");
        assert_eq!(to_rich("see [the staff](hexcasting:staff).", A), "see {u #2f5fbf|the staff}.");
        assert_eq!(to_rich("- one\n- two", A), "• one\n• two");
        assert_eq!(to_rich("2 \\* 3 and {#74b3f2|media}", A), "2 \\* 3 and {#74b3f2|media}");
        assert_eq!(to_rich("**unclosed", A), "{b|unclosed}");
        let r = yog_ui::rich::runs(&to_rich("**==both==**", A));
        assert_eq!(r[0].1.color, Some(0x8d6acc));
    }
}
