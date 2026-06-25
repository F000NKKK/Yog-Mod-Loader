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
#[derive(Debug, Clone)]
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