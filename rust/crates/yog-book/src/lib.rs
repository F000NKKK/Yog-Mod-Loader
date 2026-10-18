//! yog-book — the renderer guide books (the `.ybook` format,
//! `yog_formats::book`) are drawn with, in the game and in Yog-IDLE
//! ([`BookRenderer`], `draw2d` alone).

pub mod markup;
pub mod recipe_area;
pub mod renderer;
pub mod state;
pub mod theme;

use yog_formats::book::BookEntry;

pub use renderer::{BOOK_PNG, BOOK_SHEET, BookRenderer, CRAFTING_PNG, CRAFTING_SHEET, LANG, PageDrawer};
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

/// Whether `entry` shows in the book's lists: not secret, and its
/// advancement (if it waits for one) had.
pub fn shown(entry: &BookEntry) -> bool {
    !entry.secret && entry.advancement.as_deref().is_none_or(|a| UNLOCKED.get().is_none_or(|check| check(a)))
}
