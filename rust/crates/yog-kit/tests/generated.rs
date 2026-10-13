//! The codegen fixture (`fixtures/list.yui` → `fixtures/list.g.rs`, kept in
//! sync by yog-cli's tests) compiled against a view model and driven like
//! `manual_view.rs`.

use yog_api::ui::event::keys;
use yog_api::ui::StateFlags;
use yog_kit::testing::TestHost;
use yog_kit::{Cx, ViewModel};

#[derive(Clone, PartialEq)]
struct Entry {
    name: String,
    even: bool,
}

#[derive(Default)]
struct Vm {
    items: Vec<Entry>,
    search: String,
    only_even: bool,
    selected: Option<String>,
}

impl ViewModel for Vm {
    fn on_open(&mut self, _cx: &mut Cx<'_, Self>) {
        if self.items.is_empty() {
            self.items = (0..100)
                .map(|i| Entry { name: format!("item {i}"), even: i % 2 == 0 })
                .collect();
        }
    }
}

impl Vm {
    fn visible(&self) -> impl Iterator<Item = &Entry> + '_ {
        self.items
            .iter()
            .filter(move |e| (!self.only_even || e.even) && e.name.contains(self.search.as_str()))
    }
    fn count_text(&self) -> String {
        format!("{} shown", self.visible().count())
    }
    fn empty_text(&self) -> &'static str {
        "nothing"
    }
    fn scroll_key(&self) -> (bool, &str) {
        (self.only_even, self.search.as_str())
    }
    fn selected(&self) -> Option<&Entry> {
        match &self.selected {
            Some(s) => self.visible().find(|e| &e.name == s),
            None => self.visible().next(),
        }
    }
    fn selected_id(&self) -> Option<&str> {
        self.selected().map(|e| e.name.as_str())
    }
    fn select(&mut self, id: String) {
        self.selected = Some(id);
    }
    fn remove(&mut self, name: String) {
        self.items.retain(|e| e.name != name);
    }
}

/// The generated screen, as `ui_include!` pulls it into a code-behind;
/// the tests live next to it like a code-behind's `#[cfg(test)]` module.
mod list {
    use super::*;
    include!("fixtures/list.g.rs");

    fn host() -> TestHost<View> {
        let mut h = TestHost::<View>::new(Vm::default());
        h.open();
        h
    }

    #[test]
    fn binds_and_virtualizes() {
        let h = host();
        assert_eq!(h.text("count").as_deref(), Some("100 shown"));
        assert_eq!(h.text("details").as_deref(), Some("item 0"));
        assert!(h.is_visible("details"));
        let list = h.widget("list").unwrap();
        assert!(list.children.len() < 20, "{} children", list.children.len());
        assert!(list.children[1].state.contains(StateFlags::SELECTED));
        assert_eq!(ID, "test:list");
        assert_eq!(ids::ROW_LABEL, "row_label");
    }

    #[test]
    fn commands_toggles_and_text() {
        let mut h = host();
        h.click_id("even");
        assert!(h.vm().only_even);
        assert!(h.state("even").contains(StateFlags::CHECKED));
        assert_eq!(h.text("count").as_deref(), Some("50 shown"));
        h.type_into("search", "8");
        assert_eq!(h.vm().search, "8");
        assert_eq!(h.text("count").as_deref(), Some("14 shown"));
        h.click_id("close");
        assert!(h.closed());
    }

    #[test]
    fn list_selection_item_commands_and_empty() {
        let mut h = host();
        let mut row = h.path("list");
        row.push(2);
        h.click_path(&row);
        assert_eq!(h.vm().selected.as_deref(), Some("item 1"));
        assert_eq!(h.text("details").as_deref(), Some("item 1"));
        assert!(!h.is_visible("details"), "odd item: visible bound to even");
        h.key(keys::DOWN);
        assert_eq!(h.vm().selected.as_deref(), Some("item 2"));

        // the row's remove button: an item command with the item's name
        let mut btn = h.path("list");
        btn.extend([1, 1]);
        h.click_path(&btn);
        assert_eq!(h.vm().items.len(), 99);
        assert!(!h.vm().items.iter().any(|e| e.name == "item 0"));

        h.key(keys::END);
        assert!(h.scroll_offset(0) > 0.0);
        h.type_into("search", "zzz");
        assert_eq!(h.scroll_offset(0), 0.0);
        assert!(h.is_visible("empty"));
        assert_eq!(h.text("empty").as_deref(), Some("nothing"));
        assert!(h.widget("details").is_none());
    }
}

/// The `<Grid>` fixture: a WPF-style grid with bound children.
mod grid {
    use super::*;
    include!("fixtures/grid.g.rs");

    fn host() -> TestHost<View> {
        let mut h = TestHost::<View>::new(Vm::default());
        h.size(200.0, 120.0).open();
        h
    }

    #[test]
    fn cells_rows_and_bindings() {
        let mut h = host();
        assert_eq!(h.text("title").as_deref(), Some("100 shown"));
        let title = h.rect("title").unwrap();
        let list = h.rect("list").unwrap();
        let details = h.rect("presenter").unwrap();
        let close = h.rect("close").unwrap();
        // Auto header (one line, centred over both columns), `*` middle row,
        // 20 px footer; 80 px + `*` columns.
        assert_eq!((list.x, list.w), (0.0, 80.0));
        assert_eq!(list.y, title.y + title.h + 2.0);
        assert_eq!(list.y + list.h + 2.0 + 20.0, 120.0);
        assert_eq!((details.x, details.y, details.w), (84.0, list.y + 4.0, 112.0));
        assert_eq!((close.x, close.y, close.w, close.h), (160.0, 100.0, 40.0, 20.0));
        assert!(title.x > 0.0 && title.x + title.w < 200.0);
        // The long list stays inside its row and virtualizes.
        assert!(h.widget("list").unwrap().children.len() < 20);
        assert_eq!(h.text("details").as_deref(), Some("item 0"));

        let mut row = h.path("list");
        row.push(2);
        h.click_path(&row);
        assert_eq!(h.text("details").as_deref(), Some("item 1"));
        let mut btn = h.path("list");
        btn.extend([1, 1]);
        h.click_path(&btn);
        assert_eq!(h.vm().items.len(), 99);
        assert_eq!(h.text("title").as_deref(), Some("99 shown"));
        h.click_id("close");
        assert!(h.closed());
    }
}
