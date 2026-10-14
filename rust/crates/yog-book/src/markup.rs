//! Book texts are Yog text (`yog_ui::rich`: `{b|…}`, `{#rrggbb|…}`, …)
//! with two words of the book's own, turned into Yog text's when the book
//! is prepared:
//!
//! | Write | Shows |
//! |---|---|
//! | `{accent\|term}` | a term, in the book's accent color |
//! | `{link=entry\|text}` | a link to an entry: underlined, in the link color |

/// Links' color.
const LINK: &str = "#2f5fbf";

/// `text` with the book's words (`accent`, `link=…`) as Yog text's.
pub fn resolve(text: &str, accent: &str) -> String {
    yog_ui::rich::map_attrs(text, |w| match w {
        "accent" => Some(accent.to_string()),
        w if w.starts_with("link=") => Some(format!("u {LINK}")),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_books_words() {
        let r = resolve("{accent|Media} is {b|{link=hexcasting:staff|staves}} {{x}}", "#8d6acc");
        assert_eq!(r, "{#8d6acc|Media} is {b|{u #2f5fbf|staves}} {{x}}");
        let runs = yog_ui::rich::runs(&r);
        assert_eq!(runs[0].1.color, Some(0x8d6acc));
        assert!(runs.iter().any(|(t, s)| t == "staves" && s.bold && s.underline));
    }
}
