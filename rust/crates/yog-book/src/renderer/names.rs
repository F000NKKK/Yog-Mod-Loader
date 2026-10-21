//! The names a book shows that it doesn't spell itself: its own words in
//! the game's language, items' and entities' names (the game's, else made
//! from their ids), editions' ordinals, the sheet it's drawn from.

use std::collections::HashMap;
use yog_sync::{LazyLock, RwLock};

use super::{BOOK_SHEET, LANG};

/// Looks a translation key up in the game's language.
pub(crate) type Translator = fn(&str) -> Option<String>;

/// The game's translations: item names and the book's own words (set with the translator).
static TRANSLATOR: RwLock<Option<Translator>> = RwLock::new(None);

/// The book's own words' keys (`yog.book.<key>`), and where an argument goes.
const WORD_PREFIX: &str = "yog.book.";
const ARGUMENT: &str = "%s";
/// The game's keys for an entity's name and an item's (`<kind>.<ns>.<path>`).
const ENTITY_KEY: &str = "entity";
/// The words an English edition counts with: `th st nd rd`.
const ORDINALS: usize = 4;
/// Numbers that end in these take the first ordinal ("11th", not "11st").
const TEENS: std::ops::RangeInclusive<u32> = 11..=13;
const HUNDRED: u32 = 100;
const TEN: u32 = 10;

pub(crate) fn set_translator(tr: Translator) {
    *TRANSLATOR.write() = Some(tr);
}

fn translator() -> Option<Translator> {
    *TRANSLATOR.read()
}

/// `key` translated, unless the game has no such key (it answers the key back).
fn translated(key: &str) -> Option<String> {
    translator().and_then(|tr| tr(key)).filter(|t| t != key)
}

/// The book's own word `yog.book.<key>` in the game's language (`%s`:
/// `arg`), else English.
pub(crate) fn word(key: &str, arg: &str) -> String {
    let k = format!("{WORD_PREFIX}{key}");
    let text = translated(&k).or_else(|| {
        // English, from the shipped file.
        static EN: LazyLock<HashMap<String, String>> =
            LazyLock::new(|| serde_json::from_str(LANG[0].1).unwrap_or_default());
        EN.get(&k).cloned()
    });
    text.unwrap_or(k).replace(ARGUMENT, arg)
}

/// "staff/oak" → "Oak Staff".
fn name_from_path(path: &str) -> String {
    path.rsplit(yog_ids::sep::PATH)
        .flat_map(|seg| seg.split('_'))
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `n` as the game's language counts editions: "1st" in English (its
/// `yog.book.ordinals`), plain "1" where it has none.
pub(crate) fn ordinal(n: u32) -> String {
    let suffixes = word("ordinals", "");
    let s: Vec<&str> = suffixes.split_whitespace().collect();
    if s.len() < ORDINALS {
        return n.to_string();
    }
    let i = match (n % HUNDRED, n % TEN) {
        (teen, _) if TEENS.contains(&teen) => 0,
        (_, last) if (1..ORDINALS as u32).contains(&last) => last as usize,
        _ => 0,
    };
    format!("{n}{}", s[i])
}

/// `id` as namespace and path (a bare path is the game's).
fn parts(id: &str) -> (&str, &str) {
    id.split_once(yog_ids::sep::NAMESPACE)
        .unwrap_or((yog_ids::ns::MINECRAFT, id))
}

/// An entity's name: the game's (`entity.<ns>.<path>`), else one made from its id.
pub(crate) fn entity_name(id: &str) -> String {
    let (ns, path) = parts(id);
    translated(&format!(
        "{ENTITY_KEY}.{ns}.{}",
        path.replace(yog_ids::sep::PATH, ".")
    ))
    .unwrap_or_else(|| name_from_path(path))
}

/// An item's name: the game's (`item.<ns>.<path>`, else `block.…`), else
/// one made from its id — "hexcasting:staff/oak" → "Oak Staff".
pub(crate) fn pretty_item_name(id: &str) -> String {
    let (ns, path) = parts(id);
    let key = path.replace(yog_ids::sep::PATH, ".");
    [yog_ids::kind::ITEM, yog_ids::kind::BLOCK]
        .iter()
        .find_map(|kind| translated(&format!("{kind}.{ns}.{key}")))
        .unwrap_or_else(|| name_from_path(path))
}

/// The image source of a book's sheet: its own (`book_texture`, a sheet
/// laid out as Yog's; `ns:textures/x.png` and `ns:x` alike) or Yog's.
pub(crate) fn book_sheet(texture: &str) -> String {
    let t = texture.trim();
    if t.is_empty() || t == yog_ids::yog::BOOK_TEXTURE {
        return BOOK_SHEET.to_string();
    }
    yog_gfx::image::short_source(t)
}
