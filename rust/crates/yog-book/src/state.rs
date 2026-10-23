//! Per-book navigation state.

use yog_formats::book::Book;

pub mod action;

/// Rows of entries on the first list spread (right page only).
pub const ENTRIES_FIRST: usize = 10;
/// Rows of entries on each subsequent list spread (right page only).
pub const ENTRIES_PER: usize = 12;

/// The rows an entry's name takes in the list: its lines, wrapped to the row.
pub fn list_rows(name: &str) -> usize {
    yog_ui::text::line_count(name, crate::renderer::geometry::LIST_NAME_W, 1.0).max(1)
}

#[derive(Debug, Clone)]
pub struct BookViewState {
    pub cat: usize,
    /// The entry's index among its category's (all of them, not just the spread's).
    pub entry: usize,
    pub page: usize,
    pub at_home: bool,
    /// Which spread of the entry list is shown.
    pub list_spread: usize,
}

impl Default for BookViewState {
    fn default() -> Self {
        Self {
            cat: 0,
            entry: 0,
            page: 0,
            at_home: true,
            list_spread: 0,
        }
    }
}

impl BookViewState {
    /// Handle a navigation event string. Returns true if state changed.
    pub fn handle(&mut self, ev: &str, book: &Book) -> bool {
        if ev == action::HOME {
            if !self.at_home {
                self.at_home = true;
                return true;
            }
        } else if let Some(n) = ev.strip_prefix(action::CATEGORY) {
            if let Ok(i) = n.parse::<usize>()
                && i < book.categories.len()
            {
                let changed = self.at_home || i != self.cat;
                self.cat = i;
                self.entry = 0;
                self.page = 0;
                self.list_spread = 0;
                self.at_home = false;
                return changed;
            }
        } else if let Some(n) = ev.strip_prefix(action::ENTRY) {
            // N is the absolute index within entries_in_cat.
            if let Ok(i) = n.parse::<usize>() {
                let total = self.entries_in_cat(book).len();
                if i < total && (i != self.entry || self.page != 0) {
                    self.entry = i;
                    self.page = 0;
                    return true;
                }
            }
        } else if ev == action::PREV_PAGE {
            if self.page > 0 {
                self.page -= 1;
                return true;
            }
        } else if ev == action::NEXT_PAGE {
            if self.page + 1 < self.page_count(book) {
                self.page += 1;
                return true;
            }
        } else if ev == action::PREV_LIST {
            if self.list_spread > 0 {
                self.list_spread -= 1;
                return true;
            }
        } else if ev == action::NEXT_LIST && self.list_spread + 1 < self.list_spread_count(book) {
            self.list_spread += 1;
            return true;
        }
        false
    }

    /// All entries in the current category, unfiltered.
    pub fn entries_in_cat<'b>(&self, book: &'b Book) -> Vec<&'b yog_formats::book::BookEntry> {
        let cat_id = book
            .categories
            .get(self.cat)
            .map(|c| c.id.as_str())
            .unwrap_or("");
        book.entries
            .iter()
            .filter(|e| e.category == cat_id && crate::shown(e))
            .collect()
    }

    /// The entries each list spread of the current category shows (their
    /// indices), filled row by row: a long name wraps onto more rows.
    fn list_spreads(&self, book: &Book) -> Vec<std::ops::Range<usize>> {
        let all = self.entries_in_cat(book);
        let mut spreads = Vec::new();
        let (mut start, mut used) = (0, 0);
        for (i, e) in all.iter().enumerate() {
            let rows = list_rows(&e.name);
            let room = if spreads.is_empty() {
                ENTRIES_FIRST
            } else {
                ENTRIES_PER
            };
            if used + rows > room && i > start {
                spreads.push(start..i);
                (start, used) = (i, 0);
            }
            used += rows;
        }
        spreads.push(start..all.len());
        spreads
    }

    /// How many list spreads the current category needs.
    pub fn list_spread_count(&self, book: &Book) -> usize {
        self.list_spreads(book).len()
    }

    /// Absolute index of the first entry on the current list spread.
    pub fn list_spread_start(&self, book: &Book) -> usize {
        self.list_spreads(book)
            .get(self.list_spread)
            .map_or(0, |r| r.start)
    }

    /// The slice of entries visible on the current list spread.
    pub fn entries_visible<'b>(&self, book: &'b Book) -> Vec<&'b yog_formats::book::BookEntry> {
        let range = self
            .list_spreads(book)
            .get(self.list_spread)
            .cloned()
            .unwrap_or_default();
        let all = self.entries_in_cat(book);
        all.into_iter()
            .skip(range.start)
            .take(range.len())
            .collect()
    }

    pub fn current_entry<'b>(&self, book: &'b Book) -> Option<&'b yog_formats::book::BookEntry> {
        self.entries_in_cat(book).into_iter().nth(self.entry)
    }

    pub fn page_count(&self, book: &Book) -> usize {
        self.current_entry(book)
            .map(|e| e.pages.len().max(1))
            .unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_long_entry_name_takes_more_rows() {
        assert_eq!(super::list_rows("Типы"), 1);
        assert!(super::list_rows(&"Продвинутое Экранирование ".repeat(3)) > 1);
    }
}
