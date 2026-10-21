//! The names in `.yres` files: entries, sets, locales — and what the
//! generated code and the lang files call them.

use yog_ids::code_set;
use yog_ids::kind;

use crate::rust_code::{is_ident, is_keyword};

/// An entry name: identifiers joined by `.` (`title`, `screen.title`).
pub fn valid_name(name: &str) -> bool {
    yog_data::typed::is_res_key(name)
}

/// What's wrong with entry name `name`, if anything.
pub fn name_problem(name: &str) -> Option<String> {
    (!valid_name(name)).then(|| {
        format!("{name:?} isn't a valid name — identifiers joined by `.`, like screen.title")
    })
}

/// A resource set name: one identifier (`Strings`, `Icons`).
pub fn valid_set_name(name: &str) -> bool {
    is_ident(name) && !is_keyword(name)
}

/// How long a locale's language and region are: `en`/`lol`, `us`/`419`/`hant`.
const LANGUAGE_LEN: std::ops::RangeInclusive<usize> = 2..=3;
const REGION_LEN: std::ops::RangeInclusive<usize> = 2..=4;
/// What joins them: `en_us`.
const LOCALE_SEP: char = '_';

/// Minecraft's locale codes: lower case, `en_us`, `pt_br`, `lol_us`, ...
pub fn valid_locale(locale: &str) -> bool {
    let Some((lang, region)) = locale.split_once(LOCALE_SEP) else {
        return false;
    };
    LANGUAGE_LEN.contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase())
        && REGION_LEN.contains(&region.len())
        && region
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The accessor for entry `name`: `screen.title` → `screen_title`,
/// `ModName` → `mod_name`.
pub fn accessor_name(name: &str) -> String {
    let mut out = String::new();
    for (n, part) in name.split(yog_ids::sep::EXT).enumerate() {
        if n > 0 {
            out.push_str(yog_ids::sep::WORD);
        }
        let mut prev_lower = false;
        for c in part.chars() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push_str(yog_ids::sep::WORD);
            }
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

/// The sets whose texts are content's names, and the kind of content.
const NAME_SETS: [(&str, &str); 3] = [
    (code_set::ITEMS, kind::ITEM),
    (code_set::BLOCKS, kind::BLOCK),
    (code_set::ENTITIES, kind::ENTITY),
];

/// The default translation key of text `name` in set `set`: a content
/// name set's the game's key of that content, any other `<mod>.<name>`.
pub fn default_key(set: &str, mod_id: &str, name: &str) -> String {
    match NAME_SETS.iter().find(|(s, _)| *s == set) {
        Some((_, kind)) => yog_ids::lang_key(kind, &yog_ids::join(mod_id, name)),
        None => [mod_id, name].join(yog_ids::sep::EXT),
    }
}
