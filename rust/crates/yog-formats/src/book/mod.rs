//! The `.ybook` format: a guide book (see [`Book`]) — its categories,
//! entries and pages, the mods' own kinds of page ([`page_type`]) and how
//! recipes are laid out ([`layout`]). A book names what it shows — items,
//! blocks, entities, recipes, its own entries — through typed references
//! (`@item:minecraft:stick`, see [`refs`]); tools check them and hand the
//! renderer (`yog-book`) plain ids.

pub mod layout;
pub mod page_type;
pub mod refs;

use std::collections::BTreeMap;

use yog_data::Schema;
use yog_data::kinds::{Block, Category, Entity, Entry, Item, Recipe};
use yog_data::with::{List, Opt, RefTo, Values};

use refs::{BookText, Icon};

/// The blocks of a multiblock `pattern` (layers top to bottom, rows of
/// characters) with their `mapping`: ((x, y, z), block state).
pub fn multiblock_blocks(
    pattern: &[Vec<String>],
    mapping: &BTreeMap<String, String>,
) -> Vec<((i32, i32, i32), String)> {
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

/// One page of an entry: `{"type": "spotlight", "item": "@item:minecraft:stick", …}`.
#[derive(Debug, Clone, Schema)]
#[data(tag = "type")]
pub enum BookPage {
    /// Text (Yog text: `{b|…}`, `{#rrggbb|…}`, `{link=…|…}`), with an
    /// optional title; in another `font` (a resource-pack font id, e.g. a
    /// `ttf` provider) at `font_size` pixels a line.
    Text {
        #[data(with = "BookText")]
        text: String,
        #[data(with = "Opt<BookText>")]
        title: Option<String>,
        /// A resource-pack font id.
        font: Option<String>,
        /// Pixels a line.
        font_size: Option<f32>,
    },
    /// An item in a frame, its name (or `title`) and text.
    Spotlight {
        #[data(with = "RefTo<(Item, Block)>")]
        item: String,
        #[data(with = "Opt<BookText>")]
        title: Option<String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// A recipe as its station shows it (a crafting grid, a furnace, …).
    Recipe {
        #[data(with = "RefTo<Recipe>")]
        recipe: String,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// An image (a Yog image source: a texture, `.svg` too).
    Image {
        /// An image source (`mymod:textures/gui/x.png`, `.svg` too).
        texture: String,
        #[data(with = "Opt<BookText>")]
        title: Option<String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// A living entity, turning slowly.
    Entity {
        #[data(with = "RefTo<Entity>")]
        entity: String,
        #[data(with = "Opt<BookText>")]
        name: Option<String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// A structure in 3D, turning: `pattern` is its layers top to bottom,
    /// each a list of rows; each character stands for the block `mapping`
    /// gives it (a block state: `minecraft:furnace[facing=north]`), ` ` for air.
    Multiblock {
        #[data(with = "Opt<BookText>")]
        name: Option<String>,
        /// Layers top to bottom, each a list of rows of characters.
        pattern: Vec<Vec<String>>,
        /// What each character is: a block, with a state if it matters.
        #[data(with = "Values<RefTo<Block>>")]
        mapping: BTreeMap<String, String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// Links to other entries.
    Relations {
        #[data(with = "List<RefTo<Entry>>")]
        entries: Vec<String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
    },
    /// A picture the mod's code draws (`Registry::on_book_page`) — Hex
    /// Casting's patterns, a mod's diagram: the `drawer` registered under
    /// that id gets `data` and a box the page's width and `height` high,
    /// between the title and a caption line, over the text.
    Drawn {
        /// The id the mod's drawer is registered under (`hexcasting:pattern`).
        drawer: String,
        #[data(with = "Opt<BookText>")]
        title: Option<String>,
        /// The picture's height in book pixels (0: none).
        #[data(default, range = "0..=150")]
        height: f32,
        /// A line under the picture.
        #[data(with = "Opt<BookText>")]
        caption: Option<String>,
        #[data(with = "Opt<BookText>")]
        text: Option<String>,
        /// Whatever the drawer reads, as is.
        #[data(default)]
        data: yog_data::Value,
    },
    /// Nothing (to start the next on a new spread).
    Empty,
    /// A page of a mod's own type (`type: "hexcasting:pattern"`), laid out
    /// by the book's [`page_type::PageType`] of it.
    #[data(other)]
    Custom(page_type::CustomPage),
}

// ── Categories and entries ───────────────────────────────────────────────────

/// A category of the book's home page.
#[derive(Debug, Clone, Default, Schema)]
pub struct BookCategory {
    pub id: String,
    #[data(with = "BookText")]
    pub name: String,
    #[data(with = "Opt<BookText>")]
    pub description: Option<String>,
    /// An item (`@item:minecraft:book`) or an image (`minecraft:textures/item/book.png`).
    #[data(with = "Opt<Icon>")]
    pub icon: Option<String>,
    /// Lower first.
    #[data(default)]
    pub sortnum: i32,
}

/// One entry: its pages, in a category.
#[derive(Debug, Clone, Default, Schema)]
pub struct BookEntry {
    pub id: String,
    #[data(with = "BookText")]
    pub name: String,
    #[data(with = "RefTo<Category>")]
    pub category: String,
    #[data(default)]
    pub pages: Vec<BookPage>,
    /// An item or an image, as [`BookCategory::icon`].
    #[data(with = "Opt<Icon>")]
    pub icon: Option<String>,
    /// Lower first.
    #[data(default)]
    pub priority: i32,
    /// Hidden from the book's lists.
    #[data(default)]
    pub secret: bool,
    /// Shown as read before it's opened.
    #[data(default)]
    pub read_by_default: bool,
    /// Mods it's about — Yog or Java mod ids (`pehkui`): without every one
    /// of them loaded the book doesn't show it.
    #[data(default)]
    pub requires: Vec<String>,
    /// The advancement that unlocks it.
    pub advancement: Option<String>,
}

// ── Book ─────────────────────────────────────────────────────────────────────

/// A guide book: what a `.ybook` file holds.
#[derive(Debug, Clone, Schema)]
pub struct Book {
    /// The item that opens it (the mod's namespace if it has none).
    pub id: String,
    #[data(with = "BookText")]
    pub name: String,
    /// The nameplate's second line when `version` is 0.
    #[data(with = "Opt<BookText>")]
    pub subtitle: Option<String>,
    /// The book's edition: its nameplate says "1st Edition", "2nd Edition"…
    /// (0: the `subtitle` instead).
    #[data(default)]
    pub version: u32,
    #[data(with = "BookText", default)]
    pub landing_text: String,
    /// The nameplate's text color (`rrggbb`).
    #[data(default = "default_nameplate_color")]
    pub nameplate_color: String,
    /// The color `{accent|terms}` are shown in (`#rrggbb`).
    #[data(default = "default_accent")]
    pub accent: String,
    /// Its sheet: `ns:textures/x.png`, or Yog's (`yog_book::BOOK_PNG` shows its layout).
    #[data(default = "default_book_texture")]
    pub book_texture: String,
    #[data(default)]
    pub categories: Vec<BookCategory>,
    #[data(default)]
    pub entries: Vec<BookEntry>,
    /// Its texts are translation keys, shown in the game's language (the
    /// build turns it on when a text is a `.yres` text or a name).
    #[data(default)]
    pub i18n: bool,
    /// Recipe layouts of its own; the build adds the mod's (`.ydef` recipe kinds and layouts).
    #[data(default)]
    pub layouts: Vec<layout::RecipeLayout>,
    /// The mods' own kinds of page its pages use; the build adds them
    /// (`.yui` components with `book_page`).
    #[data(default)]
    pub page_types: Vec<page_type::PageType>,
}

fn default_nameplate_color() -> String {
    "FFDD98".into()
}

fn default_accent() -> String {
    "#8d6acc".into()
}

fn default_book_texture() -> String {
    yog_ids::yog::BOOK_TEXTURE.into()
}

impl Default for Book {
    fn default() -> Self {
        Book {
            id: String::new(),
            name: String::new(),
            subtitle: None,
            version: 0,
            landing_text: String::new(),
            nameplate_color: default_nameplate_color(),
            accent: default_accent(),
            book_texture: default_book_texture(),
            categories: Vec::new(),
            entries: Vec::new(),
            i18n: false,
            layouts: Vec::new(),
            page_types: Vec::new(),
        }
    }
}

impl Book {
    /// This book with its texts run through `tr` — names, descriptions,
    /// landing text and every page's title/text. With [`Book::i18n`] on,
    /// the texts are translation keys and the renderer shows
    /// `localized(<current language>)` (see `yog_book::BookRenderer::set_translator`).
    pub fn localized(&self, tr: impl Fn(&str) -> String) -> Book {
        let opt = |o: &mut Option<String>| {
            if let Some(t) = o {
                *t = tr(t);
            }
        };
        let mut b = self.clone();
        let types = std::mem::take(&mut b.page_types);
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
                    BookPage::Spotlight { title, text, .. }
                    | BookPage::Image { title, text, .. } => {
                        opt(title);
                        opt(text);
                    }
                    BookPage::Entity { name, text, .. }
                    | BookPage::Multiblock { name, text, .. } => {
                        opt(name);
                        opt(text);
                    }
                    BookPage::Recipe { text, .. } | BookPage::Relations { text, .. } => opt(text),
                    BookPage::Drawn {
                        title,
                        caption,
                        text,
                        ..
                    } => {
                        opt(title);
                        opt(caption);
                        opt(text);
                    }
                    BookPage::Empty => {}
                    // A mod's page: its texts, as its type says.
                    BookPage::Custom(c) => {
                        let Some(t) = types.iter().find(|t| t.id == c.kind) else {
                            continue;
                        };
                        for prop in t.properties.iter().filter(|p| {
                            matches!(
                                p.kind,
                                page_type::PropKind::Text | page_type::PropKind::BookText
                            )
                        }) {
                            if let Some(yog_data::Value::Str(s)) = c.fields.get_mut(&prop.name) {
                                *s = tr(s);
                            }
                        }
                    }
                }
            }
        }
        b.page_types = types;
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> BookPage {
        BookPage::Text {
            text: t.into(),
            title: None,
            font: None,
            font_size: None,
        }
    }

    #[test]
    fn books_round_trip_through_their_json() {
        let book = Book {
            id: "yog:test".into(),
            name: "Test Book".into(),
            version: 1,
            categories: vec![BookCategory {
                id: "yog:c1".into(),
                name: "Cat".into(),
                icon: Some("yog:ruby".into()),
                ..Default::default()
            }],
            entries: vec![BookEntry {
                id: "yog:e1".into(),
                name: "Entry".into(),
                category: "yog:c1".into(),
                pages: vec![
                    text("plain"),
                    BookPage::Spotlight {
                        item: "yog:ruby".into(),
                        title: None,
                        text: None,
                    },
                    BookPage::Recipe {
                        recipe: "yog:r1".into(),
                        text: None,
                    },
                    BookPage::Empty,
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let parsed: Book = yog_data::read_bytes(&yog_data::to_bytes(&book)).expect("it reads back");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.entries[0].pages.len(), 4);
        assert!(
            matches!(&parsed.entries[0].pages[1], BookPage::Spotlight { item, .. } if item == "yog:ruby")
        );
        assert!(
            matches!(&parsed.entries[0].pages[2], BookPage::Recipe { recipe, .. } if recipe == "yog:r1")
        );
    }

    #[test]
    fn localized_translates_every_text() {
        let book = Book {
            id: "m:b".into(),
            name: "m.book.name".into(),
            landing_text: "m.book.landing".into(),
            categories: vec![BookCategory {
                id: "m:basics".into(),
                name: "m.cat".into(),
                ..Default::default()
            }],
            entries: vec![BookEntry {
                id: "m:e".into(),
                name: "m.entry".into(),
                category: "m:basics".into(),
                pages: vec![
                    BookPage::Text {
                        text: "m.text".into(),
                        title: Some("m.title".into()),
                        font: None,
                        font_size: None,
                    },
                    text("raw text"),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let out = book.localized(|k| {
            if k.starts_with("m.") {
                k.to_uppercase()
            } else {
                k.to_string()
            }
        });
        assert_eq!(out.name, "M.BOOK.NAME");
        assert_eq!(out.landing_text, "M.BOOK.LANDING");
        assert_eq!(out.categories[0].name, "M.CAT");
        assert_eq!(out.entries[0].name, "M.ENTRY");
        assert!(
            matches!(&out.entries[0].pages[0], BookPage::Text { text, title, .. } if text == "M.TEXT" && title.as_deref() == Some("M.TITLE"))
        );
        assert!(
            matches!(&out.entries[0].pages[1], BookPage::Text { text, .. } if text == "raw text")
        );
    }
}
