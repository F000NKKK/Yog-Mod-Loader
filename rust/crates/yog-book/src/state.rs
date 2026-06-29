//! Per-book navigation state.

use crate::Book;

#[derive(Debug, Clone)]
pub struct BookViewState {
    pub cat:     usize,
    pub entry:   usize,
    pub page:    usize,
    /// True while showing the landing/home view (categories on right, landing text on left).
    pub at_home: bool,
}

impl Default for BookViewState {
    fn default() -> Self {
        Self { cat: 0, entry: 0, page: 0, at_home: true }
    }
}

impl BookViewState {
    /// Handle a navigation event string (home, cat:N, entry:N, prev_page, next_page).
    /// Returns true if state changed.
    pub fn handle(&mut self, ev: &str, book: &Book) -> bool {
        if ev == "home" {
            if !self.at_home { self.at_home = true; return true; }
        } else if let Some(n) = ev.strip_prefix("cat:") {
            if let Ok(i) = n.parse::<usize>() {
                if i < book.categories.len() {
                    let changed = self.at_home || i != self.cat;
                    self.cat     = i;
                    self.entry   = 0;
                    self.page    = 0;
                    self.at_home = false;
                    return changed;
                }
            }
        } else if let Some(n) = ev.strip_prefix("entry:") {
            if let Ok(i) = n.parse::<usize>() {
                let entries = self.entries_in_cat(book);
                if i < entries.len() && i != self.entry {
                    self.entry = i;
                    self.page  = 0;
                    return true;
                }
            }
        } else if ev == "prev_page" {
            if self.page > 0 {
                self.page -= 1;
                return true;
            }
        } else if ev == "next_page" {
            let max = self.page_count(book);
            if self.page + 1 < max {
                self.page += 1;
                return true;
            }
        }
        false
    }

    pub fn entries_in_cat<'b>(&self, book: &'b Book) -> Vec<&'b crate::BookEntry> {
        let cat_id = book.categories.get(self.cat).map(|c| c.id.as_str()).unwrap_or("");
        book.entries.iter().filter(|e| e.category == cat_id).collect()
    }

    pub fn current_entry<'b>(&self, book: &'b Book) -> Option<&'b crate::BookEntry> {
        self.entries_in_cat(book).into_iter().nth(self.entry)
    }

    pub fn page_count(&self, book: &Book) -> usize {
        self.current_entry(book).map(|e| e.pages.len().max(1)).unwrap_or(1)
    }
}
