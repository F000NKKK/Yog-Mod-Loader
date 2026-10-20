//! What a click on the book does: its widgets' actions, read by
//! [`super::BookViewState::handle`] (`cat:<i>`, `entry:<i>` with an index).

pub const HOME: &str = "home";
pub const CATEGORY: &str = "cat:";
pub const ENTRY: &str = "entry:";
pub const PREV_PAGE: &str = "prev_page";
pub const NEXT_PAGE: &str = "next_page";
pub const PREV_LIST: &str = "prev_list";
pub const NEXT_LIST: &str = "next_list";

/// Category `i`'s action.
pub fn category(i: usize) -> String {
    format!("{CATEGORY}{i}")
}

/// Entry `i`'s action.
pub fn entry(i: usize) -> String {
    format!("{ENTRY}{i}")
}
