//! yog-book — in-game book/documentation system for Yog mods (Patchouli-like).
//! Full replacement: books, categories, entries, page types, macros, textures.

use yog_registry::ItemDef;

// ── Macros ───────────────────────────────────────────────────────────────────

/// A macro substitution (e.g. `$(thing)` → red color span).
#[derive(Debug, Clone)]
pub struct BookMacro(pub String, pub String);

// ── Page types ───────────────────────────────────────────────────────────────

/// A single page variant inside a book entry.
#[derive(Debug, Clone)]
pub enum BookPage {
    /// Plain formatted text (Patchouli-style).
    Text {
        text: String,
    },
    /// Display an item outlined (tooltip on hover).
    Spotlight {
        item: ItemDef,
        title: Option<String>,
        text: Option<String>,
    },
    /// Crafting recipe display (autorenders 3×3 grid).
    Crafting {
        recipe_id: String,
        text: Option<String>,
    },
    /// Smelting recipe display.
    Smelting {
        recipe_id: String,
        text: Option<String>,
    },
    /// Image overlay page.
    Image {
        texture: String,
        title: Option<String>,
        text: Option<String>,
        border: bool,
    },
    /// Entity display page (renders a living entity in a box).
    Entity {
        entity_type: String,
        name: Option<String>,
        text: Option<String>,
    },
    /// Link to another entry (like Patchouli's relations).
    Relations {
        entries: Vec<String>,
        text: Option<String>,
    },
    /// Empty separator.
    Empty,
    /// Custom pattern page for Hexcasting-style mods (like `hexcasting:pattern`).
    Pattern {
        op_id: String,
        anchor: String,
        input: String,
        output: String,
        text: String,
    },
}

// ── Category ─────────────────────────────────────────────────────────────────

/// Represents a book category tab (e.g. "Basics", "Patterns").
#[derive(Debug, Clone)]
pub struct BookCategory {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Texture for the category icon (path like "minecraft:textures/..." or "hexcasting:textures/item/...")
    pub icon: Option<String>,
    /// Sort priority (lower = first).
    pub sortnum: i32,
}

// ── Entry ────────────────────────────────────────────────────────────────────

/// One entry in a book (like a "page" in the TOC sidebar).
#[derive(Debug, Clone, Default)]
pub struct BookEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub pages: Vec<BookPage>,
    /// Entry icon (item id or texture path).
    pub icon: Option<String>,
    /// If true, hides from the book (used for unlocks).
    pub secret: bool,
    /// Sort priority (lower = first).
    pub priority: i32,
    /// If true, read by default when opening the book.
    pub read_by_default: bool,
    /// Advancement required to unlock.
    pub advancement: Option<String>,
}

// ── Book ─────────────────────────────────────────────────────────────────────

/// The top-level book definition — replaces `patchouli_books/<id>/book.json`.
#[derive(Debug, Clone)]
pub struct Book {
    pub id: String,
    pub name: String,
    pub nameplate_color: String,
    pub landing_text: String,
    pub author: Option<String>,
    pub book_texture: String,
    pub filler_texture: String,
    pub model: String,
    pub categories: Vec<BookCategory>,
    pub entries: Vec<BookEntry>,
    pub macros: Vec<BookMacro>,
    pub use_resource_pack: bool,
    pub show_progress: bool,
    pub i18n: bool,
    pub creative_tab: Option<String>,
}

impl Book {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            nameplate_color: "000000".into(),
            landing_text: String::new(),
            author: None,
            book_texture: "yog:textures/gui/book.png".into(),
            filler_texture: "yog:textures/gui/book_filler.png".into(),
            model: "minecraft:book".into(),
            categories: Vec::new(),
            entries: Vec::new(),
            macros: Vec::new(),
            use_resource_pack: false,
            show_progress: true,
            i18n: false,
            creative_tab: None,
        }
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn book_texture(mut self, tex: impl Into<String>) -> Self {
        self.book_texture = tex.into();
        self
    }

    pub fn filler_texture(mut self, tex: impl Into<String>) -> Self {
        self.filler_texture = tex.into();
        self
    }

    pub fn nameplate(mut self, color: impl Into<String>) -> Self {
        self.nameplate_color = color.into();
        self
    }

    pub fn landing_text(mut self, text: impl Into<String>) -> Self {
        self.landing_text = text.into();
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn creative_tab(mut self, tab: impl Into<String>) -> Self {
        self.creative_tab = Some(tab.into());
        self
    }

    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    pub fn i18n(mut self, val: bool) -> Self {
        self.i18n = val;
        self
    }

    pub fn use_resource_pack(mut self, val: bool) -> Self {
        self.use_resource_pack = val;
        self
    }

    pub fn add_macro(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.macros.push(BookMacro(key.into(), value.into()));
        self
    }

    pub fn add_category(mut self, category: BookCategory) -> Self {
        self.categories.push(category);
        self
    }

    pub fn add_entry(mut self, entry: BookEntry) -> Self {
        self.entries.push(entry);
        self
    }
}

impl Default for Book {
    fn default() -> Self {
        Self::new("yog:default", "Unknown Book")
    }
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Global registry for all in-game books.
#[derive(Debug, Default)]
pub struct BookRegistry {
    books: std::collections::HashMap<String, Book>,
}

impl BookRegistry {
    pub fn register(&mut self, book: Book) {
        self.books.insert(book.id.clone(), book);
    }

    pub fn get(&self, id: &str) -> Option<&Book> {
        self.books.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Book> {
        self.books.values()
    }
}

// ── Builder helpers ──────────────────────────────────────────────────────────

pub fn text_page(text: impl Into<String>) -> BookPage {
    BookPage::Text { text: text.into() }
}

pub fn spotlight_page(item: ItemDef) -> BookPage {
    BookPage::Spotlight { item, title: None, text: None }
}

pub fn crafting_page(recipe_id: impl Into<String>) -> BookPage {
    BookPage::Crafting { recipe_id: recipe_id.into(), text: None }
}

pub fn crafting_page_with_text(recipe_id: impl Into<String>, text: impl Into<String>) -> BookPage {
    BookPage::Crafting { recipe_id: recipe_id.into(), text: Some(text.into()) }
}

pub fn smelting_page(recipe_id: impl Into<String>) -> BookPage {
    BookPage::Smelting { recipe_id: recipe_id.into(), text: None }
}

pub fn image_page(texture: impl Into<String>) -> BookPage {
    BookPage::Image { texture: texture.into(), title: None, text: None, border: true }
}

pub fn entity_page(entity_type: impl Into<String>) -> BookPage {
    BookPage::Entity { entity_type: entity_type.into(), name: None, text: None }
}

pub fn relations_page(entries: Vec<String>) -> BookPage {
    BookPage::Relations { entries, text: None }
}

pub fn pattern_page(op_id: impl Into<String>, anchor: impl Into<String>, input: impl Into<String>, output: impl Into<String>, text: impl Into<String>) -> BookPage {
    BookPage::Pattern {
        op_id: op_id.into(),
        anchor: anchor.into(),
        input: input.into(),
        output: output.into(),
        text: text.into(),
    }
}

// ── Book → yog-ui bridge ─────────────────────────────────────────────────────
#[cfg(feature = "yog-ui")]
pub mod book_ui {
    use crate::{Book, BookEntry, BookPage};
    use yog_ui::widget::{self, Widget};
    use yog_ui::{Align, FlexDir, UiRoot};

    /// Build a `UiRoot` from a `Book`.
    /// The UI has: left panel (categories + entries), right panel (pages),
    /// prev/next buttons at bottom.
    pub fn build_book_ui(book: &Book, selected_cat: usize, selected_entry: usize, current_page: usize) -> UiRoot {
        let mut cats: Vec<Widget> = Vec::new();
        for (i, cat) in book.categories.iter().enumerate() {
            let color = if i == selected_cat { 0xFF_FFFF55 } else { 0xFF_CCCCCC };
            cats.push(widget::button(&cat.name)
                .color(color)
                .on_click(format!("cat:{}", i)));
        }

        let cat = book.categories.get(selected_cat);
        let mut entries: Vec<Widget> = Vec::new();
        if let Some(cat) = cat {
            let cat_entries: Vec<&BookEntry> = book.entries.iter()
                .filter(|e| e.category == cat.id).collect();
            for (i, entry) in cat_entries.iter().enumerate() {
                let color = if i == selected_entry { 0xFF_FFFF55 } else { 0xFF_CCCCCC };
                let label = if entry.name.len() > 14 { &entry.name[..14] } else { &entry.name };
                entries.push(widget::button(label)
                    .color(color)
                    .on_click(format!("entry:{}", i)));
            }
        }

        let mut pages: Vec<Widget> = Vec::new();
        if let Some(cat) = cat {
            let cat_entries: Vec<&BookEntry> = book.entries.iter()
                .filter(|e| e.category == cat.id).collect();
            if let Some(entry) = cat_entries.get(selected_entry) {
                if let Some(page) = entry.pages.get(current_page) {
                    pages.push(render_page(page));
                }
            }
        }

        let nav = widget::panel(FlexDir::Row).gap(4.0)
            .child(widget::button("<").w(28.0).on_click("prev_page"))
            .child(widget::label(&format!("{}/{}", current_page + 1,
                cat.map_or(0, |c| {
                    book.entries.iter().filter(|e| e.category == c.id).nth(selected_entry)
                        .map_or(0, |e| e.pages.len())
                }))).color(0xFF_888888).flex(1.0).align(Align::Center))
            .child(widget::button(">").w(28.0).on_click("next_page"));

        UiRoot::new(&book.id,
            widget::panel(FlexDir::Row).gap(2.0)
                .padding(2.0, 2.0, 2.0, 2.0).bg(0xFF_2A1A0E)
                .child(
                    widget::panel(FlexDir::Column).w(104.0)
                        .child(widget::label("Categories").color(0xFF_888888))
                        .child(widget::panel(FlexDir::Column).gap(1.0)
                            .child_many(cats))
                        .child(widget::label("Entries").color(0xFF_888888))
                        .child(widget::panel(FlexDir::Column).gap(1.0)
                            .child_many(entries))
                )
                .child(
                    widget::panel(FlexDir::Column).flex(1.0).gap(2.0)
                        .child(widget::panel(FlexDir::Column).flex(1.0)
                            .child_many(pages))
                        .child(nav)
                )
        )
    }

    fn render_page(page: &BookPage) -> Widget {
        match page {
            BookPage::Text { text } =>
                widget::label(text).color(0xFF_CCCCAA),
            BookPage::Spotlight { item, title, text } => {
                let mut p = widget::panel(FlexDir::Column).gap(2.0);
                if let Some(t) = title { p = p.child(widget::label(t).color(0xFF_FFFF55)); }
                p = p.child(widget::item_slot(&item.id));
                if let Some(t) = text { p = p.child(widget::label(t).color(0xFF_CCCCAA)); }
                p
            }
            BookPage::Crafting { recipe_id, text } => {
                let mut p = widget::panel(FlexDir::Column).gap(2.0);
                p = p.child(widget::label(format!("Crafting: {}", recipe_id)).color(0xFF_888888));
                if let Some(t) = text { p = p.child(widget::label(t).color(0xFF_CCCCAA)); }
                p
            }
            BookPage::Smelting { recipe_id, text } => {
                let mut p = widget::panel(FlexDir::Column).gap(2.0);
                p = p.child(widget::label(format!("Smelting: {}", recipe_id)).color(0xFF_888888));
                if let Some(t) = text { p = p.child(widget::label(t).color(0xFF_CCCCAA)); }
                p
            }
            BookPage::Empty => widget::spacer(),
            _ => widget::label("(unsupported page)").color(0xFF_888888),
        }
    }

    // Helper: add multiple children to a widget
    trait WidgetExt {
        fn child_many(self, children: Vec<Widget>) -> Self;
    }
    impl WidgetExt for Widget {
        fn child_many(mut self, children: Vec<Widget>) -> Self {
            for c in children { self = self.child(c); }
            self
        }
    }
}

// ── JSON serialization ────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

impl BookPage {
    pub fn to_json(&self) -> String {
        match self {
            Self::Text { text } =>
                format!(r#"{{"type":"text","text":"{}"}}"#, esc(text)),
            Self::Spotlight { item, title, text } => {
                let t = title.as_deref().map(|s| format!(r#","title":"{}""#, esc(s))).unwrap_or_default();
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"spotlight","item":"{id}"{t}{tx}}}"#, id = esc(&item.id))
            }
            Self::Crafting { recipe_id, text } => {
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"crafting","recipe":"{}"{}}}"#, esc(recipe_id), tx)
            }
            Self::Smelting { recipe_id, text } => {
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"smelting","recipe":"{}"{}}}"#, esc(recipe_id), tx)
            }
            Self::Image { texture, title, text, border } => {
                let t = title.as_deref().map(|s| format!(r#","title":"{}""#, esc(s))).unwrap_or_default();
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"image","texture":"{}","border":{}{}{}}}"#,
                    esc(texture), border, t, tx)
            }
            Self::Entity { entity_type, name, text } => {
                let n = name.as_deref().map(|s| format!(r#","name":"{}""#, esc(s))).unwrap_or_default();
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"entity","entity":"{}"{}{}}}"#, esc(entity_type), n, tx)
            }
            Self::Relations { entries, text } => {
                let e: String = entries.iter().map(|s| format!(r#""{}""#, esc(s))).collect::<Vec<_>>().join(",");
                let tx = text.as_deref().map(|s| format!(r#","text":"{}""#, esc(s))).unwrap_or_default();
                format!(r#"{{"type":"relations","entries":[{}]{}}}"#, e, tx)
            }
            Self::Empty => r#"{"type":"empty"}"#.to_string(),
            Self::Pattern { op_id, anchor, input, output, text } =>
                format!(r#"{{"type":"pattern","op_id":"{}","anchor":"{}","input":"{}","output":"{}","text":"{}"}}"#,
                    esc(op_id), esc(anchor), esc(input), esc(output), esc(text)),
        }
    }
}

impl BookEntry {
    pub fn to_json(&self) -> String {
        let pages: String = self.pages.iter().map(|p| p.to_json()).collect::<Vec<_>>().join(",");
        let icon = self.icon.as_deref().map(|s| format!(r#","icon":"{}""#, esc(s))).unwrap_or_default();
        let adv = self.advancement.as_deref().map(|s| format!(r#","advancement":"{}""#, esc(s))).unwrap_or_default();
        format!(
            r#"{{"id":"{}","name":"{}","category":"{}","pages":[{}],"secret":{},"priority":{},"read_by_default":{}{}{}}}"#,
            esc(&self.id), esc(&self.name), esc(&self.category), pages,
            self.secret, self.priority, self.read_by_default, icon, adv
        )
    }
}

impl BookCategory {
    pub fn to_json(&self) -> String {
        let desc = self.description.as_deref().map(|s| format!(r#","description":"{}""#, esc(s))).unwrap_or_default();
        let icon = self.icon.as_deref().map(|s| format!(r#","icon":"{}""#, esc(s))).unwrap_or_default();
        format!(
            r#"{{"id":"{}","name":"{}","sortnum":{}{}{}}}"#,
            esc(&self.id), esc(&self.name), self.sortnum, desc, icon
        )
    }
}

impl Book {
    pub fn to_json(&self) -> String {
        let cats: String = self.categories.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(",");
        let entries: String = self.entries.iter().map(|e| e.to_json()).collect::<Vec<_>>().join(",");
        let author = self.author.as_deref().map(|s| format!(r#","author":"{}""#, esc(s))).unwrap_or_default();
        let tab = self.creative_tab.as_deref().map(|s| format!(r#","creative_tab":"{}""#, esc(s))).unwrap_or_default();
        format!(
            r#"{{"id":"{}","name":"{}","nameplate_color":"{}","landing_text":"{}","book_texture":"{}","filler_texture":"{}","model":"{}","show_progress":{},"i18n":{},"use_resource_pack":{},"categories":[{}],"entries":[{}]{}{}}}"#,
            esc(&self.id), esc(&self.name), esc(&self.nameplate_color), esc(&self.landing_text),
            esc(&self.book_texture), esc(&self.filler_texture), esc(&self.model),
            self.show_progress, self.i18n, self.use_resource_pack,
            cats, entries, author, tab
        )
    }
}