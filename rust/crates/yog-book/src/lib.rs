//! yog-book — the renderer guide books (the `.ybook` format,
//! `yog_formats::book`) are drawn with, in the game and in Yog-IDLE
//! ([`BookRenderer`], `draw2d` alone).

pub mod layout_picture;
pub mod markup;
pub mod recipe_area;
pub mod renderer;
pub mod state;
pub mod theme;

use yog_formats::book::BookEntry;

pub use renderer::{
    BOOK_PNG, BOOK_SHEET, BookRenderer, CRAFTING_PNG, CRAFTING_SHEET, LANG, PageDrawer,
};
pub use state::BookViewState;
pub use theme::BookTheme;

/// Whether the reader has advancement `id` — set by the host the book is
/// read in (the runtime asks the game); everything is unlocked without one
/// (an editor's preview).
static UNLOCKED: std::sync::OnceLock<fn(&str) -> bool> = std::sync::OnceLock::new();

/// Tells the books how to know what the reader has unlocked.
pub fn set_unlocked(check: fn(&str) -> bool) {
    let _ = UNLOCKED.set(check);
}

/// The items of an item tag, as the host the book is read in knows them
/// (the game's tags, the IDE's); a tag shows none without one.
static TAG_ITEMS: std::sync::OnceLock<fn(&str) -> Vec<String>> = std::sync::OnceLock::new();

/// Tells the books how to find an item tag's items.
pub fn set_tag_items(items: fn(&str) -> Vec<String>) {
    let _ = TAG_ITEMS.set(items);
}

/// `ingredient` — items, and tags (`#ns:path`) — as the items it shows in
/// turn: each tag replaced by its items.
pub fn ingredient_items(ingredient: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for i in ingredient {
        match i.strip_prefix(yog_ids::sep::TAG_SHOWN) {
            Some(tag) => out.extend(TAG_ITEMS.get().map(|f| f(tag)).unwrap_or_default()),
            None => out.push(i.clone()),
        }
    }
    out
}

/// Whether `entry` shows in the book's lists: not secret, and its
/// advancement (if it waits for one) had.
pub fn shown(entry: &BookEntry) -> bool {
    !entry.secret
        && entry
            .advancement
            .as_deref()
            .is_none_or(|a| UNLOCKED.get().is_none_or(|check| check(a)))
}
