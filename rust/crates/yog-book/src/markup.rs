//! Book text markup — a small Markdown, turned into Minecraft's formatting
//! codes when the book is prepared:
//!
//! | Write | Shows |
//! |---|---|
//! | `**bold**` | **bold** |
//! | `*italic*` | *italic* |
//! | `==term==` | a highlighted term (Patchouli's "thing" purple) |
//! | `[text](entry)` | a link: blue, underlined |
//! | `- item` at a line's start | `• item` |
//! | a blank line | a new paragraph |
//! | `\*` | a literal `*` (any character after `\`) |

const HIGHLIGHT: char = '5';
const LINK: char = '9';

#[derive(Default, Clone, Copy)]
struct State {
    bold: bool,
    italic: bool,
    highlight: bool,
    link: bool,
}

impl State {
    fn color(self) -> Option<char> {
        if self.link {
            Some(LINK)
        } else if self.highlight {
            Some(HIGHLIGHT)
        } else {
            None
        }
    }

    fn styles(self) -> String {
        let mut s = String::new();
        if self.bold {
            s.push_str("§l");
        }
        if self.italic {
            s.push_str("§o");
        }
        if self.link {
            s.push_str("§n");
        }
        s
    }

    /// The codes that go from `self` to `next`: just the styles added, or —
    /// something dropped, the color changed (a color code clears the
    /// styles) — `next` written out whole.
    fn to(self, next: State) -> String {
        let dropped = (self.bold && !next.bold) || (self.italic && !next.italic) || (self.link && !next.link);
        if !dropped && self.color() == next.color() {
            let mut s = String::new();
            if next.bold && !self.bold {
                s.push_str("§l");
            }
            if next.italic && !self.italic {
                s.push_str("§o");
            }
            return s;
        }
        let mut s = match next.color() {
            Some(c) => format!("§{c}"),
            None => "§r".to_string(),
        };
        s.push_str(&next.styles());
        s
    }
}

/// `text` with its markup as formatting codes.
pub fn to_minecraft(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut st = State::default();
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
                    out.push(next.unwrap_or_default());
                    i += 2;
                    continue;
                }
                '*' if next == Some('*') => {
                    let to = State { bold: !st.bold, ..st };
                    out.push_str(&st.to(to));
                    st = to;
                    i += 2;
                    continue;
                }
                '*' => {
                    let to = State { italic: !st.italic, ..st };
                    out.push_str(&st.to(to));
                    st = to;
                }
                '=' if next == Some('=') => {
                    let to = State { highlight: !st.highlight, ..st };
                    out.push_str(&st.to(to));
                    st = to;
                    i += 2;
                    continue;
                }
                '[' => {
                    // `[text](target)`: the text, styled as a link.
                    if let Some(close) = chars[i..].iter().position(|&x| x == ']').map(|p| p + i)
                        && chars.get(close + 1) == Some(&'(')
                        && let Some(end) = chars[close..].iter().position(|&x| x == ')').map(|p| p + close)
                    {
                        let link = State { link: true, ..st };
                        out.push_str(&st.to(link));
                        out.extend(&chars[i + 1..close]);
                        out.push_str(&link.to(st));
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
    // Nothing set stays set past the text.
    if st.bold || st.italic || st.highlight {
        out.push_str("§r");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles() {
        assert_eq!(to_minecraft("plain"), "plain");
        assert_eq!(to_minecraft("a **b** c"), "a §lb§r c");
        assert_eq!(to_minecraft("==Media== is *mind*"), "§5Media§r is §omind§r");
        assert_eq!(to_minecraft("**==both==**"), "§l§5§lboth§r§l§r");
        assert_eq!(to_minecraft("see [the staff](hexcasting:staff)."), "see §9§nthe staff§r.");
        assert_eq!(to_minecraft("- one\n- two"), "• one\n• two");
        assert_eq!(to_minecraft("2 \\* 3"), "2 * 3");
    }
}
