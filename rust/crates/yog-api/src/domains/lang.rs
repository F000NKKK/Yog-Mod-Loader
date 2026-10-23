//! Translations — text in the player's language, from the lang files the
//! game loaded (a mod's `.yres` texts become them at `yog build`).
//!
//! ```ignore
//! let title = yog_api::tr("mymod.screen.title");
//! let count = yog_api::tr!("mymod.screen.count", shown, total); // "%s of %s mods"
//! ```

use std::fmt::{Display, Write as _};

/// The text of translation `key` in the current language, unformatted —
/// `None` when no lang file defines it (or before the mod registered).
pub fn translation(key: &str) -> Option<String> {
    crate::call(&yog_event::game::Translate {
        key: key.to_owned(),
    })
    .flatten()
}

/// The text of translation `key` in the current language, or `key` itself
/// when it has none — what Minecraft shows for an unknown key, so a missing
/// translation is easy to spot. Call it each time the text is shown (e.g.
/// while building a UI tree): the player can switch language at any time.
pub fn tr(key: &str) -> String {
    match translation(key) {
        Some(text) => format_translation(&text, &[]),
        None => key.to_string(),
    }
}

/// A tag's name in the player's language — `kind` the content's
/// (`item`, `block`, `entity`, `fluid`, `biome`), `id` the tag's
/// (`yog:ingots/iron`): the translation recipe viewers show
/// (`tag.item.yog.ingots.iron`), else the id itself.
pub fn tag_name(kind: &str, id: &str) -> String {
    let registry = yog_ids::registry_of(kind).replace(yog_ids::sep::PATH, ".");
    let key = format!("tag.{registry}.{}", id.replace([':', '/'], "."));
    translation(&key).map_or_else(|| id.to_string(), |text| format_translation(&text, &[]))
}

/// [`tr`] with arguments for the text's format specifiers — see
/// [`format_translation`]. The [`tr!`](crate::tr!) macro is shorter.
pub fn tr_args(key: &str, args: &[&dyn Display]) -> String {
    format_translation(&translation(key).unwrap_or_else(|| key.to_string()), args)
}

/// Fills Minecraft-style format specifiers in `pattern` (what lang files
/// use): `%s`/`%d` take the next argument, `%1$s`/`%2$d` a numbered one
/// (from 1), `%%` is a literal `%`. A specifier without an argument stays
/// as written.
pub fn format_translation(pattern: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut next = 0;
    let mut rest = pattern;
    while let Some(pos) = rest.find('%') {
        out.push_str(&rest[..pos]);
        let spec = &rest[pos + 1..];
        if let Some(after) = spec.strip_prefix('%') {
            out.push('%');
            rest = after;
            continue;
        }
        if let Some(after) = spec.strip_prefix(['s', 'd']) {
            match args.get(next) {
                Some(arg) => {
                    let _ = write!(out, "{arg}");
                }
                None => out.push_str(&rest[pos..pos + 2]),
            }
            next += 1;
            rest = after;
            continue;
        }
        let digits = spec.bytes().take_while(u8::is_ascii_digit).count();
        let numbered = (digits > 0)
            .then(|| spec[digits..].strip_prefix('$'))
            .flatten()
            .and_then(|s| s.strip_prefix(['s', 'd']));
        match (numbered, spec[..digits].parse::<usize>()) {
            (Some(after), Ok(n)) if n >= 1 => {
                match args.get(n - 1) {
                    Some(arg) => {
                        let _ = write!(out, "{arg}");
                    }
                    None => out.push_str(&rest[pos..rest.len() - after.len()]),
                }
                rest = after;
            }
            _ => {
                out.push('%');
                rest = spec;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `tr!("key")` = [`tr`]`("key")`; `tr!("key", a, b)` fills the text's
/// `%s` specifiers with `a` and `b` (anything `Display`) — see
/// [`format_translation`].
#[macro_export]
macro_rules! tr {
    ($key:expr_2021 $(,)?) => {
        $crate::tr($key)
    };
    ($key:expr_2021, $($arg:expr_2021),+ $(,)?) => {
        $crate::tr_args($key, &[$(&$arg as &dyn ::std::fmt::Display),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_minecraft() {
        assert_eq!(
            format_translation("%s of %s mods", &[&3, &"58"]),
            "3 of 58 mods"
        );
        assert_eq!(format_translation("%2$s, %1$s", &[&"a", &"b"]), "b, a");
        assert_eq!(format_translation("100%% by %s", &[&"X"]), "100% by X");
        assert_eq!(format_translation("%d items", &[&5]), "5 items");
        assert_eq!(
            format_translation("missing %s and %3$s", &[]),
            "missing %s and %3$s"
        );
        assert_eq!(format_translation("50% off %x", &[&1]), "50% off %x");
        assert_eq!(format_translation("trailing %", &[]), "trailing %");
        assert_eq!(format_translation("Привет, %s!", &[&"мир"]), "Привет, мир!");
    }

    #[test]
    fn unknown_key_without_runtime_is_the_key() {
        assert_eq!(tr("mymod.nothing"), "mymod.nothing");
        assert_eq!(crate::tr!("count %s", 4), "count 4");
    }
}
