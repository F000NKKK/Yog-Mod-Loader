//! yog-book — guide books for Yog mods: the format (a `.ybook` file, see
//! [`Book`]) and the renderer the game and Yog-IDLE draw it with
//! ([`BookRenderer`], `draw2d` alone).
//!
//! A book names what it shows — items, blocks, entities, recipes, its own
//! entries — through typed references (`@item:minecraft:stick`, see
//! [`refs`]); tools check them and hand the renderer plain ids.

pub mod markup;
pub mod pattern;
pub mod refs;
pub mod renderer;
pub mod state;
pub mod theme;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use renderer::{BOOK_PNG, BOOK_SHEET, BookRenderer, CRAFTING_PNG, CRAFTING_SHEET, LANG};
pub use state::BookViewState;
pub use theme::BookTheme;

/// The blocks of a multiblock `pattern` (layers top to bottom, rows of
/// characters) with their `mapping`: ((x, y, z), block state).
pub fn multiblock_blocks(pattern: &[Vec<String>], mapping: &BTreeMap<String, String>) -> Vec<((i32, i32, i32), String)> {
    let mut out = Vec::new();
    let layers = pattern.len() as i32;
    for (li, layer) in pattern.iter().enumerate() {
        for (z, row) in layer.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                if let Some(state) = mapping.get(&ch.to_string()) {
                    out.push(((x as i32, layers - 1 - li as i32, z as i32), state.clone()));
                }
            }
        }
    }
    out
}

// ── Pages ────────────────────────────────────────────────────────────────────

/// One page of an entry: `{"type": "spotlight", "item": "minecraft:stick", …}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BookPage {
    /// Text (Yog text: `{b|…}`, `{#rrggbb|…}`, `{link=…|…}`), with an
    /// optional title; in another `font` (a resource-pack font id, e.g. a
    /// `ttf` provider) at `font_size` pixels a line.
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font_size: Option<f32>,
    },
    /// An item in a frame, its name (or `title`) and text.
    Spotlight {
        item: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// A recipe as its station shows it (a crafting grid, a furnace, …).
    Recipe {
        recipe: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// An image (a Yog image source: a texture, `.svg` too).
    Image {
        texture: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// A living entity, turning slowly.
    Entity {
        entity: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// A structure in 3D, turning: `pattern` is its layers top to bottom,
    /// each a list of rows; each character stands for the block `mapping`
    /// gives it (a block state: `minecraft:furnace[facing=north]`), ` ` for air.
    Multiblock {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        pattern: Vec<Vec<String>>,
        mapping: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Links to other entries.
    Relations {
        entries: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// A spell pattern: its name, what it takes and gives, the pattern
    /// itself on a hexagonal grid (see [`pattern`]).
    Pattern {
        op_id: String,
        /// The action's name (the page's title; `op_id` if none).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default)]
        input: String,
        #[serde(default)]
        output: String,
        #[serde(default)]
        text: String,
        /// The patterns drawn on the page (none: just the texts).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        patterns: Vec<pattern::HexPattern>,
    },
    /// Nothing (to start the next on a new spread).
    Empty,
}

// ── Categories and entries ───────────────────────────────────────────────────

/// A category of the book's home page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BookCategory {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// An item (`minecraft:book`) or an image (`minecraft:textures/item/book.png`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Lower first.
    pub sortnum: i32,
}

/// One entry: its pages, in a category.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BookEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub pages: Vec<BookPage>,
    /// An item or an image, as [`BookCategory::icon`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Lower first.
    pub priority: i32,
    /// Hidden from the book's lists.
    pub secret: bool,
    /// Shown as read before it's opened.
    pub read_by_default: bool,
    /// The advancement that unlocks it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advancement: Option<String>,
}

// ── Book ─────────────────────────────────────────────────────────────────────

/// A guide book: what a `.ybook` file holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Book {
    /// The item that opens it.
    pub id: String,
    pub name: String,
    /// The nameplate's second line when `version` is 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// The book's edition: its nameplate says "1st Edition", "2nd Edition"…
    /// (0: the `subtitle` instead).
    pub version: u32,
    pub landing_text: String,
    /// The nameplate's text color (`rrggbb`).
    pub nameplate_color: String,
    /// The color `{accent|terms}` are shown in (`#rrggbb`).
    pub accent: String,
    /// Its sheet (laid out as [`BOOK_PNG`]): `ns:textures/x.png`, or Yog's.
    pub book_texture: String,
    pub categories: Vec<BookCategory>,
    pub entries: Vec<BookEntry>,
    /// Its texts are translation keys, shown in the game's language.
    pub i18n: bool,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            id: String::new(),
            name: String::new(),
            subtitle: None,
            version: 0,
            landing_text: String::new(),
            nameplate_color: "FFDD98".into(),
            accent: "#8d6acc".into(),
            book_texture: "yog:gui/book".into(),
            categories: Vec::new(),
            entries: Vec::new(),
            i18n: false,
        }
    }
}

impl Book {
    /// This book with its texts run through `tr` — names, descriptions,
    /// landing text and every page's title/text. With [`Book::i18n`] on,
    /// the texts are translation keys and the renderer shows
    /// `localized(<current language>)` (see [`BookRenderer::set_translator`]).
    pub fn localized(&self, tr: impl Fn(&str) -> String) -> Book {
        let opt = |o: &mut Option<String>| {
            if let Some(t) = o {
                *t = tr(t);
            }
        };
        let mut b = self.clone();
        b.name = tr(&b.name);
        b.landing_text = tr(&b.landing_text);
        opt(&mut b.subtitle);
        for c in &mut b.categories {
            c.name = tr(&c.name);
            opt(&mut c.description);
        }
        for e in &mut b.entries {
            e.name = tr(&e.name);
            for p in &mut e.pages {
                match p {
                    BookPage::Text { text, title, .. } => {
                        *text = tr(text);
                        opt(title);
                    }
                    BookPage::Spotlight { title, text, .. } | BookPage::Image { title, text, .. } => {
                        opt(title);
                        opt(text);
                    }
                    BookPage::Entity { name, text, .. } | BookPage::Multiblock { name, text, .. } => {
                        opt(name);
                        opt(text);
                    }
                    BookPage::Recipe { text, .. } | BookPage::Relations { text, .. } => opt(text),
                    BookPage::Pattern { text, name, .. } => {
                        *text = tr(text);
                        opt(name);
                    }
                    BookPage::Empty => {}
                }
            }
        }
        b
    }

    /// The book as the JSON hosts and `yog build` pass around.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> BookPage {
        BookPage::Text { text: t.into(), title: None, font: None, font_size: None }
    }

    #[test]
    fn books_round_trip_through_their_json() {
        let book = Book {
            id: "yog:test".into(),
            name: "Test Book".into(),
            version: 1,
            categories: vec![BookCategory { id: "yog:c1".into(), name: "Cat".into(), icon: Some("yog:ruby".into()), ..Default::default() }],
            entries: vec![BookEntry {
                id: "yog:e1".into(),
                name: "Entry".into(),
                category: "yog:c1".into(),
                pages: vec![
                    text("plain"),
                    BookPage::Spotlight { item: "yog:ruby".into(), title: None, text: None },
                    BookPage::Recipe { recipe: "yog:r1".into(), text: None },
                    BookPage::Empty,
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let parsed: Book = serde_json::from_str(&book.to_json()).expect("its JSON parses");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.entries[0].pages.len(), 4);
        assert!(matches!(&parsed.entries[0].pages[1], BookPage::Spotlight { item, .. } if item == "yog:ruby"));
        assert!(matches!(&parsed.entries[0].pages[2], BookPage::Recipe { recipe, .. } if recipe == "yog:r1"));
    }

    #[test]
    fn localized_translates_every_text() {
        let book = Book {
            id: "m:b".into(),
            name: "m.book.name".into(),
            landing_text: "m.book.landing".into(),
            categories: vec![BookCategory { id: "m:basics".into(), name: "m.cat".into(), ..Default::default() }],
            entries: vec![BookEntry {
                id: "m:e".into(),
                name: "m.entry".into(),
                category: "m:basics".into(),
                pages: vec![
                    BookPage::Text { text: "m.text".into(), title: Some("m.title".into()), font: None, font_size: None },
                    text("raw text"),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let out = book.localized(|k| if k.starts_with("m.") { k.to_uppercase() } else { k.to_string() });
        assert_eq!(out.name, "M.BOOK.NAME");
        assert_eq!(out.landing_text, "M.BOOK.LANDING");
        assert_eq!(out.categories[0].name, "M.CAT");
        assert_eq!(out.entries[0].name, "M.ENTRY");
        assert!(matches!(&out.entries[0].pages[0], BookPage::Text { text, title, .. } if text == "M.TEXT" && title.as_deref() == Some("M.TITLE")));
        assert!(matches!(&out.entries[0].pages[1], BookPage::Text { text, .. } if text == "raw text"));
    }
}
