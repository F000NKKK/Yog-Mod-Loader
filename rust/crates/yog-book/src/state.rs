//! Per-book navigation state.

use crate::Book;

/// Max entries shown on the first list spread (right page only).
pub const ENTRIES_FIRST: usize = 11;
/// Max entries shown on each subsequent list spread (right page only).
pub const ENTRIES_PER: usize = 13;

#[derive(Debug, Clone)]
pub struct BookViewState {
    pub cat:         usize,
    pub entry:       usize,   // absolute index within entries_in_cat
    pub page:        usize,
    pub at_home:     bool,
    pub list_spread: usize,   // which page of the entry list we're on
}

impl Default for BookViewState {
    fn default() -> Self {
        Self { cat: 0, entry: 0, page: 0, at_home: true, list_spread: 0 }
    }
}

impl BookViewState {
    /// Handle a navigation event string. Returns true if state changed.
    pub fn handle(&mut self, ev: &str, book: &Book) -> bool {
        if ev == "home" {
            if !self.at_home { self.at_home = true; return true; }
        } else if let Some(n) = ev.strip_prefix("cat:") {
            if let Ok(i) = n.parse::<usize>() {
                if i < book.categories.len() {
                    let changed = self.at_home || i != self.cat;
                    self.cat         = i;
                    self.entry       = 0;
                    self.page        = 0;
                    self.list_spread = 0;
                    self.at_home     = false;
                    return changed;
                }
            }
        } else if let Some(n) = ev.strip_prefix("entry:") {
            // N is the absolute index within entries_in_cat.
            if let Ok(i) = n.parse::<usize>() {
                let total = self.entries_in_cat(book).len();
                if i < total && (i != self.entry || self.page != 0) {
                    self.entry = i;
                    self.page  = 0;
                    return true;
                }
            }
        } else if ev == "prev_page" {
            if self.page > 0 { self.page -= 1; return true; }
        } else if ev == "next_page" {
            if self.page + 1 < self.page_count(book) {
                self.page += 1; return true;
            }
        } else if ev == "prev_list" {
            if self.list_spread > 0 { self.list_spread -= 1; return true; }
        } else if ev == "next_list" {
            if self.list_spread + 1 < self.list_spread_count(book) {
                self.list_spread += 1; return true;
            }
        }
        false
    }

    /// All entries in the current category, unfiltered.
    pub fn entries_in_cat<'b>(&self, book: &'b Book) -> Vec<&'b crate::BookEntry> {
        let cat_id = book.categories.get(self.cat).map(|c| c.id.as_str()).unwrap_or("");
        book.entries.iter().filter(|e| e.category == cat_id).collect()
    }

    /// How many list spreads the current category needs.
    pub fn list_spread_count(&self, book: &Book) -> usize {
        let total = self.entries_in_cat(book).len();
        if total <= ENTRIES_FIRST { return 1; }
        1 + (total - ENTRIES_FIRST + ENTRIES_PER - 1) / ENTRIES_PER
    }

    /// Absolute index of the first entry on the current list spread.
    pub fn list_spread_start(&self) -> usize {
        if self.list_spread == 0 { 0 }
        else { ENTRIES_FIRST + (self.list_spread - 1) * ENTRIES_PER }
    }

    /// The slice of entries visible on the current list spread.
    pub fn entries_visible<'b>(&self, book: &'b Book) -> Vec<&'b crate::BookEntry> {
        let all = self.entries_in_cat(book);
        let start = self.list_spread_start();
        let count = if self.list_spread == 0 { ENTRIES_FIRST } else { ENTRIES_PER };
        all.into_iter().skip(start).take(count).collect()
    }

    pub fn current_entry<'b>(&self, book: &'b Book) -> Option<&'b crate::BookEntry> {
        self.entries_in_cat(book).into_iter().nth(self.entry)
    }

    pub fn page_count(&self, book: &Book) -> usize {
        self.current_entry(book).map(|e| e.pages.len().max(1)).unwrap_or(1)
    }
}
