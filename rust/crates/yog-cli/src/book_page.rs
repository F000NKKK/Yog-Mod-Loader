//! A mod's own kinds of book page: `.yui` components with `book_page`,
//! compiled into `yog_formats::book::page_type::PageType`s for the books that use
//! them (see that module).
//!
//! ```xml
//! <Component name="PatternPage" book_page="hexcasting:pattern">
//!     <Property name="name" type="book_text"/>
//!     <Property name="patterns" type="data" default="[]"/>
//!     <BookHeader text="{x:Bind name}"/>
//!     <Drawn drawer="hexcasting:pattern" h="64" data="{x:Bind patterns}" visible="{x:Bind patterns}"/>
//! </Component>
//! ```

use std::path::Path;

use yog_formats::book::page_type::{PageType, compile, is_book_page};

use crate::diagnostics::Diagnostic;
use crate::yui::{self, Element};

/// Whether `root` is a book's own UI — a kind of page (`book_page`) or a
/// recipe layout (`recipe_layout`) — not a screen or a component.
pub fn is_book_ui(root: &Element) -> bool {
    is_book_page(root) || yog_formats::book::layout::is_recipe_layout(root)
}

/// The mod's page types: every `book_page` component under `src/`.
pub fn find(root: &Path, ns: &str, errors: &mut Vec<Diagnostic>) -> Vec<PageType> {
    let mut out = Vec::new();
    for file in yui::find_yui_files(&root.join("src")) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(el) = yui::parse_xml(&text) else {
            continue;
        };
        if !is_book_page(&el) {
            continue;
        }
        let shown = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();
        match compile(&el, ns) {
            Ok(t) => out.push(t),
            Err(e) => errors.push(screen_problem(e).in_file(&shown)),
        }
    }
    out
}

/// The mod's recipe layouts: every `recipe_layout` component under `src/`.
pub fn find_layouts(
    root: &Path,
    ns: &str,
    errors: &mut Vec<Diagnostic>,
) -> Vec<(String, yog_formats::book::layout::RecipeLayout)> {
    let mut out = Vec::new();
    for file in yui::find_yui_files(&root.join("src")) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(el) = yui::parse_xml(&text) else {
            continue;
        };
        if !yog_formats::book::layout::is_recipe_layout(&el) {
            continue;
        }
        let shown = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();
        match yog_formats::book::layout::compile(&el, ns) {
            Ok(l) => out.push((shown, l)),
            Err(e) => errors.push(screen_problem(e).in_file(&shown)),
        }
    }
    out
}

/// A problem with a book's own `.yui` (`YB0004` unless it says what it is).
fn screen_problem(found: Diagnostic) -> Diagnostic {
    found.or_code(yog_ids::build_problem::SCREEN)
}
