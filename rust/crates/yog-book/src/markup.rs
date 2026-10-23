//! Book texts are Yog text (`yog_ui::rich`: `{b|…}`, `{#rrggbb|…}`, …)
//! with words of the book's own, turned into Yog text's when the book is
//! prepared:
//!
//! | Write | Shows |
//! |---|---|
//! | `{accent\|term}` | a term, in the book's accent color |
//! | `{link=entry\|text}` | a link to an entry: underlined, in the link color |
//! | `{key\|sprint}` | the key a binding is bound to (`sprint`, a mod's `mymod:xray`) |

use yog_sync::RwLock;

/// Links' color.
const LINK: &str = "#2f5fbf";
/// A key's span, up to its binding: `{key|sprint}`.
const KEY_OPEN: &str = "{key|";
const CLOSE: char = '}';

/// The key a binding is bound to, as the game's controls show it; none
/// when there's no such binding (or no game).
pub type KeyNamer = fn(&str) -> Option<String>;

static KEY_NAMER: RwLock<Option<KeyNamer>> = RwLock::new(None);

/// Where `{key|…}` gets its keys' names (the game's; the binding itself without one).
pub fn set_key_namer(f: KeyNamer) {
    *KEY_NAMER.write() = Some(f);
}

/// `text` with each `{key|binding}` the key that binding is bound to.
fn keys(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(KEY_OPEN) {
        let after = &rest[at + KEY_OPEN.len()..];
        let Some(end) = after.find(CLOSE) else { break };
        let binding = &after[..end];
        let name = (*KEY_NAMER.read())
            .and_then(|f| f(binding))
            .unwrap_or_else(|| binding.to_owned());
        out.push_str(&rest[..at]);
        out.push_str(&name.replace('{', "{{").replace('}', "}}"));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `text` with the book's words (`accent`, `link=…`, `key`) as Yog text's.
pub fn resolve(text: &str, accent: &str) -> String {
    yog_ui::rich::map_attrs(&keys(text), |w| match w {
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
        let r = resolve(
            "{accent|Media} is {b|{link=hexcasting:staff|staves}} {{x}}",
            "#8d6acc",
        );
        assert_eq!(r, "{#8d6acc|Media} is {b|{u #2f5fbf|staves}} {{x}}");
        let runs = yog_ui::rich::runs(&r);
        assert_eq!(runs[0].1.color, Some(0x8d6acc));
        assert!(
            runs.iter()
                .any(|(t, s)| t == "staves" && s.bold && s.underline)
        );
    }

    #[test]
    fn a_key_is_named_as_the_game_binds_it() {
        assert_eq!(resolve("hold {key|sprint}.", "#000000"), "hold sprint.");
        set_key_namer(|b| (b == "sprint").then(|| "Left Control".to_owned()));
        assert_eq!(
            resolve("hold {key|sprint}.", "#000000"),
            "hold Left Control."
        );
    }
}
