//! Book renderer: yog-ui lays the book out and draws its text and
//! buttons; everything else goes through `draw2d` too — the sprite
//! sheets, item icons, SVG pages, entities — so the game and Yog-IDLE's
//! book designer run the same code.

use std::collections::HashMap;
use yog_gfx::GfxContext;
use yog_ui::{Align, FlexDir, Stretch, UiRoot, widget};

use crate::state::BookViewState;
use crate::theme::BookTheme;
use crate::{Book, BookPage};

// Patchouli-compatible book texture layout (512×256 sprite sheet).
// UV coordinates are in pixels on a 512×256 sheet.
// The full open-book background occupies 272×180 at UV (0,0).
pub const BOOK_TEX_W: f32 = 512.0;
pub const BOOK_TEX_H: f32 = 256.0;
// Open-book dimensions in texture-space
pub const BK_W: f32 = 272.0;
pub const BK_H: f32 = 180.0;
// Per-page dimensions and offsets (in book-local coords)
pub const PAGE_W: f32 = 116.0;
pub const PAGE_H: f32 = 156.0;
pub const TOP_PAD: f32 = 18.0; // vertical space above page text area
pub const LEFT_X: f32 = 15.0; // left page X inside book
pub const RIGHT_X: f32 = 141.0; // right page X inside book
// Separator strip UV origin on the sprite sheet
pub const SEP_U: f32 = 140.0;
pub const SEP_V: f32 = 180.0;
pub const SEP_W: f32 = 110.0;
pub const SEP_H: f32 = 3.0;
/// Room (GUI px) the book keeps free around it before it scales down.
const SCREEN_MARGIN: f32 = 16.0;
/// Smallest the book scales on a tiny screen.
const MIN_SCALE: f32 = 0.4;

// ── Embedded textures ─────────────────────────────────────────────────────────

/// The book's sprite sheet (cover, pages, separators, nameplate).
pub static BOOK_PNG: &[u8] = include_bytes!("../assets/book_brown.png");
/// The crafting-grid sprite sheet.
pub static CRAFTING_PNG: &[u8] = include_bytes!("../assets/crafting.png");
/// The book's own words (`yog.book.*`) per language, as the loader's jars
/// ship them in `assets/yog/lang/` (for hosts without them, like Yog-IDLE).
pub static LANG: &[(&str, &str)] = &[
    ("en_us", include_str!("../assets/lang/en_us.json")),
    ("ru_ru", include_str!("../assets/lang/ru_ru.json")),
];
/// The image source the book draws [`BOOK_PNG`] as: the loader ships it
/// as `assets/yog/textures/gui/book/book.png` (Yog-IDLE resolves it to
/// the bytes).
pub const BOOK_SHEET: &str = "yog:gui/book/book";
/// The image source the book draws [`CRAFTING_PNG`] as.
pub const CRAFTING_SHEET: &str = "yog:gui/book/crafting";

// ── Background sprite (texture blit, drawn before yog-ui) ────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SpriteSheet {
    Book,
    Crafting,
}

/// A subrect blit from one of the embedded sprite sheets, drawn before yog-ui.
#[derive(Clone)]
pub(crate) struct BgSprite {
    pub sheet: SpriteSheet,
    /// Source rect in pixels on the sheet.
    pub u: f32,
    pub v: f32,
    pub uw: f32,
    pub uh: f32,
    /// Destination on screen.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// ── Pending overlay draw commands ─────────────────────────────────────────────

#[derive(Clone)]
pub(crate) enum OverlayCmd {
    Svg {
        data: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// One of `items` at a time, a new one every second (an ingredient's
    /// alternatives); tags (`#…`) are skipped.
    McItems {
        items: Vec<String>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// Blocks in 3D, turning.
    Blocks {
        blocks: Vec<((i32, i32, i32), String)>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// A living entity standing in the box.
    Entity {
        entity: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// MC default-font text rendered via draw2d (e.g. nameplate title/subtitle).
    McText {
        text: String,
        x: f32,
        y: f32,
        color: u32,
    },
    /// Item icon rendered through our GL pipeline from an MC-managed texture.
    McItem {
        item_id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// Flat texture icon blitted as-is (16×16), for icon strings that name a
    /// texture resource directly rather than an item — matches Patchouli's
    /// `BookIcon.from`: any icon string ending in `.png` is a raw texture,
    /// everything else is parsed as an item stack.
    McTexture {
        path: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

/// Push a book-author-supplied icon (category/entry icon field) as the right
/// overlay kind. Matches Patchouli's `BookIcon.from(String)`: `.png` → raw
/// texture blit, anything else → item stack render.
fn push_icon(overlays: &mut Vec<OverlayCmd>, icon: &str, x: f32, y: f32, w: f32, h: f32) {
    if icon.ends_with(".png") {
        overlays.push(OverlayCmd::McTexture {
            path: icon.to_string(),
            x,
            y,
            w,
            h,
        });
    } else {
        overlays.push(OverlayCmd::McItem {
            item_id: icon.to_string(),
            x,
            y,
            w,
            h,
        });
    }
}

// ── Recipe visuals (parsed from registered recipe JSON) ───────────────────────

#[derive(Clone)]
pub(crate) enum RecipeVis {
    Shaped {
        /// Each slot's alternatives (empty: no ingredient).
        grid: Vec<Vec<String>>,
        width: usize,
        output: String,
        count: u32,
    },
    Shapeless {
        ingredients: Vec<Vec<String>>,
        output: String,
        count: u32,
    },
    /// A one-ingredient recipe: cooking, stonecutting; `station` shows how.
    Smelting {
        input: Vec<String>,
        output: String,
        station: String,
    },
}

/// An ingredient in Yog's recipe format: an id, a `#tag`, or an array of
/// those (any of them).
fn ingredient(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(a) => a.iter().flat_map(ingredient).collect(),
        _ => Vec::new(),
    }
}

/// A recipe in Yog's own format (what a `.yrecipe` says — see
/// `yog_registry::Recipe`), as the book shows it.
fn parse_recipe(json: &str) -> Option<RecipeVis> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let output = v.get("result")?.as_str()?.to_owned();
    let count = v.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32;
    let station = |s: &str| Some(s.to_owned());
    match v.get("type")?.as_str()? {
        "shaped" => {
            let pattern: Vec<&str> = v.get("pattern")?.as_array()?.iter().filter_map(|p| p.as_str()).collect();
            let width = pattern.iter().map(|r| r.chars().count()).max()?.max(1);
            let key = v.get("key")?.as_object()?;
            let mut grid = Vec::new();
            for row in &pattern {
                let mut chars: Vec<char> = row.chars().collect();
                chars.resize(width, ' ');
                for ch in chars {
                    grid.push(key.get(&ch.to_string()).map(ingredient).unwrap_or_default());
                }
            }
            Some(RecipeVis::Shaped { grid, width, output, count })
        }
        "shapeless" => Some(RecipeVis::Shapeless {
            ingredients: v.get("ingredients")?.as_array()?.iter().map(ingredient).collect(),
            output,
            count,
        }),
        kind @ ("smelting" | "blasting" | "smoking" | "campfire_cooking" | "stonecutting") => Some(RecipeVis::Smelting {
            input: ingredient(v.get("ingredient")?),
            output,
            station: match kind {
                "blasting" => station("minecraft:blast_furnace"),
                "smoking" => station("minecraft:smoker"),
                "campfire_cooking" => station("minecraft:campfire"),
                "stonecutting" => station("minecraft:stonecutter"),
                _ => station("minecraft:furnace"),
            }?,
        }),
        _ => None,
    }
}

/// Normalize icon ids like "yog:item/ruby" / "yog:block/x" to registry item
/// ids ("yog:ruby") accepted by the MC item renderer.
fn normalize_item_id(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let name = path
        .trim_start_matches("item/")
        .trim_start_matches("block/");
    format!("{ns}:{name}")
}

/// The image source of a book's sheet: its own (`book_texture`, a sheet
/// laid out as [`BOOK_PNG`]; `ns:textures/x.png` and `ns:x` alike) or Yog's.
fn book_sheet(texture: &str) -> String {
    let t = texture.trim();
    if t.is_empty() || t == "yog:gui/book" {
        return BOOK_SHEET.to_string();
    }
    let (ns, path) = t.split_once(':').unwrap_or(("minecraft", t));
    let path = path.strip_prefix("textures/").unwrap_or(path);
    format!("{ns}:{}", path.strip_suffix(".png").unwrap_or(path))
}

/// The game's translations: item names and the book's own words (set with the translator).
static ITEM_NAMES: std::sync::RwLock<Option<Translator>> = std::sync::RwLock::new(None);

type Translator = fn(&str) -> Option<String>;

/// The book's own word `yog.book.<key>` in the game's language (`%s`:
/// `arg`), else English.
fn word(key: &str, arg: &str) -> String {
    let k = format!("yog.book.{key}");
    let tr = *ITEM_NAMES.read().unwrap_or_else(|e| e.into_inner());
    let text = tr.and_then(|tr| tr(&k)).filter(|t| *t != k).or_else(|| {
        // English, from the shipped file.
        static EN: std::sync::LazyLock<HashMap<String, String>> =
            std::sync::LazyLock::new(|| serde_json::from_str(LANG[0].1).unwrap_or_default());
        EN.get(&k).cloned()
    });
    let text = text.unwrap_or(k);
    text.replace("%s", arg)
}

/// "staff/oak" → "Oak Staff".
fn name_from_path(path: &str) -> String {
    path.rsplit('/')
        .flat_map(|seg| seg.split('_'))
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `n` as the game's language counts editions: "1st" in English (its
/// `yog.book.ordinals`), plain "1" where it has none.
fn ordinal(n: u32) -> String {
    let suffixes = word("ordinals", "");
    let s: Vec<&str> = suffixes.split_whitespace().collect();
    if s.len() < 4 {
        return n.to_string();
    }
    let i = match (n % 100, n % 10) {
        (11..=13, _) => 0,
        (_, 1) => 1,
        (_, 2) => 2,
        (_, 3) => 3,
        _ => 0,
    };
    format!("{n}{}", s[i])
}

/// An entity's name: the game's (`entity.<ns>.<path>`), else one made
/// from its id.
fn entity_name(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let k = format!("entity.{ns}.{}", path.replace('/', "."));
    let tr = *ITEM_NAMES.read().unwrap_or_else(|e| e.into_inner());
    tr.and_then(|tr| tr(&k)).filter(|t| *t != k).unwrap_or_else(|| name_from_path(path))
}

/// An item's name: the game's (`item.<ns>.<path>`, else `block.…`), else
/// one made from its id — "hexcasting:staff/oak" → "Oak Staff".
fn pretty_item_name(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    if let Some(tr) = *ITEM_NAMES.read().unwrap_or_else(|e| e.into_inner()) {
        let key = path.replace('/', ".");
        for kind in ["item", "block"] {
            let k = format!("{kind}.{ns}.{key}");
            if let Some(name) = tr(&k).filter(|n| *n != k) {
                return name;
            }
        }
    }
    name_from_path(path)
}

// ── BookRenderer ──────────────────────────────────────────────────────────────

pub struct BookRenderer {
    pub book: Book,
    pub state: BookViewState,
    pub theme: BookTheme,
    recipes: HashMap<String, RecipeVis>,
    /// Per entry: which page as written each shown page came from.
    origins: HashMap<String, Vec<usize>>,
    /// The book as written (localized), before its text flowed.
    written: Book,
    /// Flowed with the host's own text metrics (else estimated ones).
    measured: bool,
    /// When the renderer was made: what animations count from.
    start: std::time::Instant,
    pub ui: Option<UiRoot>,
    bg_sprites: Vec<BgSprite>,
    overlays: Vec<OverlayCmd>,
    dirty: bool,
    last_sw: f32,
    last_sh: f32,
    /// i18n books: the untranslated book, the translator and the language
    /// probe (the book name's text) the shown book was made for.
    source: Option<Book>,
    translator: Option<fn(&str) -> Option<String>>,
    probe: Option<Option<String>>,
}

impl BookRenderer {
    pub fn new(book: Book) -> Self {
        let theme = BookTheme::default().with_nameplate(&book.nameplate_color);
        let written = styled(&book);
        let (book, origins) = flow_traced(written.clone());
        Self {
            book,
            origins,
            written,
            measured: false,
            start: std::time::Instant::now(),
            state: BookViewState::default(),
            theme,
            recipes: HashMap::new(),
            ui: None,
            bg_sprites: Vec::new(),
            overlays: Vec::new(),
            dirty: true,
            last_sw: 0.0,
            last_sh: 0.0,
            source: None,
            translator: None,
            probe: None,
        }
    }

    /// The game's translations: items' names in recipe titles, and, for
    /// [`Book::i18n`] books, the book's texts — translation keys, looked up
    /// with `tr` (unknown keys stay as they are) and re-looked up whenever
    /// the language changes.
    pub fn set_translator(&mut self, tr: fn(&str) -> Option<String>) {
        *ITEM_NAMES.write().unwrap_or_else(|e| e.into_inner()) = Some(tr);
        if self.book.i18n {
            self.source.get_or_insert_with(|| self.book.clone());
            // (Already flowed: flowing again only splits what still overflows.)
            self.translator = Some(tr);
            self.probe = None;
        }
    }

    /// Re-translates the book when the language changed since last time.
    fn localize(&mut self) {
        let (Some(src), Some(tr)) = (&self.source, self.translator) else {
            return;
        };
        let probe = Some(tr(&src.name));
        if probe == self.probe {
            return;
        }
        let accent = src.accent.clone();
        self.written = src.localized(|k| crate::markup::resolve(&tr(k).unwrap_or_else(|| k.to_string()), &accent));
        (self.book, self.origins) = flow_traced(self.written.clone());
        self.probe = probe;
        self.dirty = true;
    }

    pub fn handle_event(&mut self, ev: &str) {
        if self.state.handle(ev, &self.book) {
            self.dirty = true;
        }
    }

    /// Register a recipe (vanilla recipe JSON) so Crafting/Smelting pages can
    /// render it visually. Unknown recipe types are ignored.
    pub fn add_recipe(&mut self, id: impl Into<String>, json: &str) {
        if let Some(vis) = parse_recipe(json) {
            self.recipes.insert(id.into(), vis);
            self.dirty = true;
        }
    }

    /// Lays the book out for a `sw`×`sh` screen (when something changed);
    /// the book's rect.
    fn prepare(&mut self, sw: f32, sh: f32) -> (f32, f32, f32, f32) {
        // Render at Patchouli-native size (272×180 GUI pixels). Scale DOWN only
        // if the screen is smaller; never scale up (MC font is designed for 1×).
        let scale = ((sw - SCREEN_MARGIN) / BK_W)
            .min((sh - SCREEN_MARGIN) / BK_H)
            .clamp(MIN_SCALE, 1.0);
        let bw = BK_W * scale;
        let bh = BK_H * scale;
        let bx = (sw - bw) / 2.0;
        let by = (sh - bh) / 2.0;

        self.localize();
        // Rebuild widget tree if dirty or screen resized.
        if self.dirty || sw != self.last_sw || sh != self.last_sh {
            let (root, bg_sprites, overlays) = build_ui(
                &self.book,
                &self.state,
                &self.theme,
                &self.recipes,
                bx,
                by,
                bw,
                bh,
            );
            self.ui = Some(root);
            self.bg_sprites = bg_sprites;
            self.overlays = overlays;
            self.dirty = false;
            self.last_sw = sw;
            self.last_sh = sh;
        }
        if let Some(ui) = &mut self.ui
            && ui.needs_layout
        {
            ui.layout(sw, sh);
        }
        (bx, by, bw, bh)
    }

    /// Draws the book into a `sw`×`sh` screen — all of it through
    /// `draw2d` (its sprite sheets as the image sources [`BOOK_SHEET`] and
    /// [`CRAFTING_SHEET`]).
    pub fn render(&mut self, ctx: &GfxContext, sw: f32, sh: f32) {
        // Text flows by the host's own font metrics once it can measure.
        yog_ui::text::use_host_metrics(ctx);
        if !self.measured {
            self.measured = true;
            self.reflow();
        }
        let (bx, by, bw, bh) = self.prepare(sw, sh);
        let d2 = ctx.draw2d();
        let book_sheet = book_sheet(&self.book.book_texture);
        d2.image_region(&book_sheet, bx, by, bw, bh, [0.0, 0.0, BK_W, BK_H], 0xFFFF_FFFF);
        for spr in &self.bg_sprites {
            let sheet = match spr.sheet {
                SpriteSheet::Book => &book_sheet,
                SpriteSheet::Crafting => CRAFTING_SHEET,
            };
            d2.image_region(sheet, spr.x, spr.y, spr.w, spr.h, [spr.u, spr.v, spr.uw, spr.uh], 0xFFFF_FFFF);
        }
        if let Some(ui) = &mut self.ui {
            ui.render(ctx);
        }
        // Animations: entities and structures turn (Patchouli's pace), an
        // ingredient's alternatives take turns every second.
        let t = self.start.elapsed().as_secs_f32();
        for ov in &self.overlays {
            match *ov {
                OverlayCmd::Svg { ref data, x, y, w, h } => d2.svg(data, x, y, w, h),
                OverlayCmd::Entity { ref entity, x, y, w, h } => d2.entity(entity, x, y, w, h, -30.0 + t * 30.0, 0.0),
                OverlayCmd::Blocks { ref blocks, x, y, w, h } => {
                    let list: Vec<((i32, i32, i32), &str)> = blocks.iter().map(|(p, b)| (*p, b.as_str())).collect();
                    d2.blocks(&list, x, y, w, h, -45.0 + t * 20.0, 30.0);
                }
                _ => {}
            }
        }
        // Item stacks and MC-font text last: item rendering changes GL state.
        self.draw_mc_overlays(ctx);
    }

    fn draw_mc_overlays(&self, ctx: &GfxContext) {
        let second = self.start.elapsed().as_secs() as usize;
        for ov in &self.overlays {
            match *ov {
                OverlayCmd::McItems { ref items, x, y, w, h } => {
                    let shown: Vec<&String> = items.iter().filter(|i| !i.starts_with('#')).collect();
                    if !shown.is_empty() {
                        let item = shown[second % shown.len()];
                        ctx.draw2d().item(&normalize_item_id(item), x, y, w.min(h));
                    }
                }
                OverlayCmd::McItem { ref item_id, x, y, w, h } => {
                    ctx.draw2d().item(&normalize_item_id(item_id), x, y, w.min(h));
                }
                OverlayCmd::McText { ref text, x, y, color } => {
                    ctx.draw2d().text(text, x, y, color, false);
                }
                OverlayCmd::McTexture { ref path, x, y, w, h } => {
                    // Standalone icon texture, not an atlas region — drawn
                    // whole, scaled into the icon box.
                    ctx.draw2d().image(path, x, y, w, h);
                }
                _ => {}
            }
        }
    }

    /// The page shown moves by itself (an entity, a structure, an
    /// ingredient's alternatives): the host should keep redrawing it.
    pub fn animated(&self) -> bool {
        self.overlays.iter().any(|o| match o {
            OverlayCmd::Entity { .. } | OverlayCmd::Blocks { .. } => true,
            OverlayCmd::McItems { items, .. } => items.iter().filter(|i| !i.starts_with('#')).count() > 1,
            _ => false,
        })
    }

    /// A click at `(x, y)` (screen GUI pixels, as last rendered): what it
    /// hit does its thing. The action it was (`"next_page"`, `"entry:3"`,
    /// …), if any.
    pub fn click(&mut self, x: f32, y: f32) -> Option<String> {
        let ui = self.ui.as_ref()?;
        let hit = yog_ui::layout::hit_test(&ui.layout_root, x, y)?;
        let action = hit.on_click.clone()?;
        self.handle_event(&action);
        Some(action)
    }

    /// Shows `entry` (an entry id) at `page` — its page as written, not
    /// counting the pages long text flowed onto — as a reader would have
    /// got there; `false` if there's no such entry.
    pub fn show(&mut self, entry: &str, page: usize) -> bool {
        let Some(e) = self.book.entries.iter().find(|e| e.id == entry) else { return false };
        let Some(cat) = self.book.categories.iter().position(|c| c.id == e.category) else { return false };
        self.state.cat = cat;
        self.state.at_home = false;
        let in_cat = self.state.entries_in_cat(&self.book);
        let Some(i) = in_cat.iter().position(|x| x.id == entry) else { return false };
        let shown = self.origins.get(entry).and_then(|o| o.iter().position(|&w| w >= page)).unwrap_or(page);
        self.state.entry = i;
        self.state.page = shown.min(e.pages.len().saturating_sub(1));
        self.dirty = true;
        true
    }

    /// The entry shown (its id) and the page of it as written, or `None`
    /// on the landing and list pages.
    pub fn shown(&self) -> Option<(String, usize)> {
        if self.state.at_home {
            return None;
        }
        let e = self.state.current_entry(&self.book)?;
        let page = self.origins.get(&e.id).and_then(|o| o.get(self.state.page)).copied().unwrap_or(self.state.page);
        Some((e.id.clone(), page))
    }

    /// Flows the text again (the metrics changed), keeping the reader's place.
    fn reflow(&mut self) {
        let at = self.shown();
        (self.book, self.origins) = flow_traced(self.written.clone());
        self.dirty = true;
        if let Some((entry, page)) = at {
            self.show(&entry, page);
        }
    }

    /// Replaces the book (an edit), keeping the reader where they were.
    pub fn set_book(&mut self, book: Book) {
        let at = self.shown();
        self.written = styled(&book);
        (self.book, self.origins) = flow_traced(self.written.clone());
        self.theme = BookTheme::default().with_nameplate(&self.book.nameplate_color);
        self.dirty = true;
        match at {
            Some((entry, page)) if self.show(&entry, page) => {}
            _ => {
                if self.state.cat >= self.book.categories.len() {
                    self.state = BookViewState::default();
                }
            }
        }
    }
}

// ── UI builder ────────────────────────────────────────────────────────────────

thread_local! {
    /// The scale the book is drawn at (≤ 1): its text is drawn at it too,
    /// so it stays on the page however small the book gets.
    static TEXT_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

fn text_scale() -> f32 {
    TEXT_SCALE.with(|s| s.get())
}

/// Book label: MC font, no drop-shadow (Patchouli draws book text shadowless).
fn lbl(text: impl Into<String>) -> widget::Widget {
    widget::label(text).shadow(false).font_scale(text_scale())
}
/// Book button: no drop-shadow.
fn btn(text: impl Into<String>) -> widget::Widget {
    widget::button(text).shadow(false).font_scale(text_scale())
}

/// A title: one line, shrunk (down to half size, then cut short) to fit
/// the page's width.
fn title_lbl(text: impl Into<String>) -> widget::Widget {
    let text = text.into();
    // As wide as the page's inside (less its padding), so `align` can centre it.
    let w = PAGE_W - 10.0;
    let fit = (w / yog_ui::text::str_width(&text, 1.0)).clamp(0.5, 1.0);
    widget::label(text)
        .shadow(false)
        .font_scale(text_scale() * fit)
        .no_wrap()
        .ellipsis()
        .w(w * text_scale())
}

// ── Flowing text ──────────────────────────────────────────────────────────────

/// Width of a page's text, in book pixels.
const TEXT_W: f32 = PAGE_W - 8.0;
/// Height a page's body has for text, in book pixels: the page less its
/// title and separator, the page navigation and the padding.
const BODY_H: f32 = PAGE_H - 22.0 - 12.0 - 14.0;

/// A multiblock page's view of its structure, and its name's height (with
/// the gap under it), in book pixels.
const MB_VIEW: f32 = 70.0;
/// A pattern page's box for its patterns, in book pixels.
const PATTERN_H: f32 = 64.0;
const MB_NAME_H: f32 = 14.0;

/// How much of the body a page's own content takes above its text.
fn content_above_text(page: &BookPage) -> Option<f32> {
    Some(match page {
        BookPage::Text { title: Some(_), .. } => 22.0,
        BookPage::Text { .. } => 0.0,
        BookPage::Spotlight { .. } => 40.0,
        BookPage::Crafting { .. } => 80.0,
        BookPage::Smelting { .. } => 40.0,
        BookPage::Image { .. } | BookPage::Entity { .. } => 104.0,
        BookPage::Pattern { patterns, input, output, .. } => {
            14.0 + if patterns.is_empty() { 0.0 } else { PATTERN_H + 4.0 }
                + if input.is_empty() && output.is_empty() { 0.0 } else { 14.0 }
        }
        BookPage::Multiblock { name, .. } => MB_VIEW + 4.0 + if name.is_some() { MB_NAME_H } else { 0.0 },
        BookPage::Relations { entries, .. } => 12.0 + entries.len() as f32 * 10.0,
        _ => return None,
    })
}

fn page_text_mut(page: &mut BookPage) -> Option<&mut String> {
    match page {
        BookPage::Text { text, .. } | BookPage::Pattern { text, .. } => Some(text),
        BookPage::Spotlight { text, .. }
        | BookPage::Crafting { text, .. }
        | BookPage::Smelting { text, .. }
        | BookPage::Image { text, .. }
        | BookPage::Entity { text, .. }
        | BookPage::Multiblock { text, .. }
        | BookPage::Relations { text, .. } => text.as_mut(),
        _ => None,
    }
}

/// `book` with its texts' own words (see [`crate::markup`]) as Yog text's.
fn styled(book: &Book) -> Book {
    book.localized(|t| crate::markup::resolve(t, &book.accent))
}

/// Text that doesn't fit its page goes on to new text pages after it.
#[cfg(test)]
fn flow(book: Book) -> Book {
    flow_traced(book).0
}

/// [`flow`], and for each entry where each of its pages came from: the
/// index of the page in the book as written.
fn flow_traced(mut book: Book) -> (Book, HashMap<String, Vec<usize>>) {
    let mut origins = HashMap::new();
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    let fits = |h: f32| (((h + yog_ui::text::LINE_GAP) / line_h).floor() as usize).max(1);
    for entry in &mut book.entries {
        let mut pages = Vec::with_capacity(entry.pages.len());
        let mut from = Vec::with_capacity(entry.pages.len());
        for (written, page) in std::mem::take(&mut entry.pages).into_iter().enumerate() {
            let split = split_page(page, &fits);
            from.extend(std::iter::repeat_n(written, split.len()));
            pages.extend(split);
        }
        entry.pages = pages;
        origins.insert(entry.id.clone(), from);
    }
    (book, origins)
}

/// `page`, and the text pages its overflowing text goes on to.
fn split_page(mut page: BookPage, fits: &dyn Fn(f32) -> usize) -> Vec<BookPage> {
    let Some(above) = content_above_text(&page) else { return vec![page] };
    let Some(text) = page_text_mut(&mut page) else { return vec![page] };
    let lines: Vec<String> = text
        .split('\n')
        .flat_map(|para| if para.is_empty() { vec![String::new()] } else { yog_ui::text::wrap_text(para, TEXT_W, 1.0) })
        .collect();
    let first = fits(BODY_H - above);
    if lines.len() <= first {
        return vec![page];
    }
    *text = lines[..first].join("\n");
    let rest: Vec<BookPage> = lines[first..]
        .chunks(fits(BODY_H))
        .map(|chunk| BookPage::Text { text: chunk.join("\n"), title: None })
        .collect();
    std::iter::once(page).chain(rest).collect()
}

/// Build the yog-ui widget tree + bg sprites + overlay commands for the current book state.
/// `bx/by/bw/bh` are the screen-space book rect (same values used to blit the bg texture).
fn build_ui(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
) -> (UiRoot, Vec<BgSprite>, Vec<OverlayCmd>) {
    let mut bg_sprites: Vec<BgSprite> = Vec::new();
    let mut overlays: Vec<OverlayCmd> = Vec::new();

    // Scale from Patchouli's fixed BK_W×BK_H coordinate space → actual bw×bh.
    let sx = bw / BK_W;
    TEXT_SCALE.with(|s| s.set(sx));
    let sy = bh / BK_H;

    // In book-local: left page starts at x=LEFT_X, right at RIGHT_X, both y=TOP_PAD.
    let pw = PAGE_W * sx;
    let ph = PAGE_H * sy;
    let spine_gap = (RIGHT_X - LEFT_X - PAGE_W) * sx; // ≈10px scaled

    // Content x/y in screen space (for SVG overlay positioning).
    let lx = bx + LEFT_X * sx;
    let rx = bx + RIGHT_X * sx;
    let py = by + TOP_PAD * sy;

    // Separator x offset within a page: centered = PAGE_W/2 - SEP_W/2 = 58-55 = 3.
    let sep_cx = (PAGE_W / 2.0 - SEP_W / 2.0) * sx; // = 3*sx

    if state.at_home {
        // Nameplate banner: book-local (-8, 12) from LEFT_PAGE_X, size 140×31.
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Book,
            u: 0.0,
            v: 180.0,
            uw: 140.0,
            uh: 31.0,
            x: bx + (LEFT_X - 8.0) * sx,
            y: by + 12.0 * sy,
            w: 140.0 * sx,
            h: 31.0 * sy,
        });
    }
    // Separator on the right page below its header ("Categories" or the
    // category name; y=12 page-local), centered.
    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Book,
        u: SEP_U,
        v: SEP_V,
        uw: SEP_W,
        uh: SEP_H,
        x: rx + sep_cx,
        y: by + (TOP_PAD + 12.0) * sy,
        w: SEP_W * sx,
        h: (SEP_H * sy).max(1.0),
    });

    let (left_page, right_page) = if state.at_home {
        let l = build_landing_left(book, theme, pw, ph, sx, sy, bx, by, &mut overlays);
        let r = build_categories_right(book, state, theme, pw, ph, rx, py, sx, sy, &mut overlays);
        (l, r)
    } else {
        let l = build_entry_left(
            book,
            state,
            theme,
            recipes,
            pw,
            ph,
            lx,
            py,
            sx,
            sy,
            sep_cx,
            &mut bg_sprites,
            &mut overlays,
        );
        let r = build_entries_right(book, state, theme, pw, ph, rx, py, sx, sy, &mut overlays);
        (l, r)
    };

    // Root row spans the full book width.
    // padding-left = LEFT_X*sx offsets both pages from the left cover edge.
    // padding-top  = TOP_PAD*sy offsets from the header banner.
    // spine_gap spacer sits between the two pages.
    // gap must be 0: analytic overlay positions assume pages sit exactly at
    // LEFT_X / RIGHT_X — any flex gap would shift widgets (and their focus
    // highlights) away from the absolutely-positioned icons.
    let root_widget = widget::panel(FlexDir::Row)
        .w(bw)
        .h(bh)
        .gap(0.0)
        .padding(TOP_PAD * sy, 0.0, 0.0, LEFT_X * sx)
        .child(left_page)
        .child(widget::spacer().w(spine_gap)) // spine gap
        .child(right_page);

    // Outer shell: positions the book on screen via a full-screen transparent Row.
    // We use a Column + Row arrangement to apply top/left offsets without margin support.
    let outer = widget::panel(FlexDir::Column)
        .w(bw)
        .h(bh)
        .child(root_widget);

    // Screen-level container: positions book at (bx, by).
    // bx offset via padding, by offset via padding-top.
    // Note: layout computes from (0,0), so we pass bx/by into the UiRoot manually
    // by wrapping in a full-screen panel with the right padding.
    let screen_root = widget::panel(FlexDir::Row)
        .padding(by, 0.0, 0.0, bx)
        .child(outer);

    let ui = UiRoot::new(&book.id, screen_root);
    (ui, bg_sprites, overlays)
}

// ── Left page: landing (home view) ───────────────────────────────────────────

fn build_landing_left(
    book: &Book,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    sx: f32,
    sy: f32,
    bx: f32,
    by: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    // Patchouli: title at book-local (13, 16), subtitle at (24, 24).
    // These overlap the nameplate sprite which is above/within the page top.
    overlays.push(OverlayCmd::McText {
        text: book.name.clone(),
        x: bx + 13.0 * sx,
        y: by + 16.0 * sy,
        color: theme.nameplate,
    });
    // Its edition ("1st Edition"), else its subtitle — as Patchouli.
    let second = match book.version {
        0 => book.subtitle.clone(),
        n => Some(word("edition", &ordinal(n))),
    };
    if let Some(text) = second {
        overlays.push(OverlayCmd::McText {
            text,
            x: bx + 24.0 * sx,
            y: by + 24.0 * sy,
            color: theme.nameplate,
        });
    }

    // Widget content: landing text + entry count.
    // Landing text starts at page-local y=25 (= book-local y=43).
    // We push it down with top padding = 25*sy.
    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(25.0 * sy, 6.0, 4.0, 4.0)
        .gap(0.0);

    // As small as it takes to fit above the entry count.
    let fit = landing_fit(&book.landing_text);
    for para in book.landing_text.split('\n') {
        if para.is_empty() {
            col = col.child(widget::spacer().h(4.0 * sy));
        } else {
            col = col.child(lbl(para).color(theme.text).font_scale(text_scale() * fit));
        }
    }

    let total = book.entries.len();
    col = col.child(widget::spacer().flex(1.0));
    col = col.child(
        title_lbl(word("entry_count", &total.to_string()))
            .color(theme.nav)
            .h(9.0 * sy)
            .align(Align::Center),
    );
    col
}

/// The scale (≤ 1) the landing text fits its page at.
fn landing_fit(text: &str) -> f32 {
    use yog_ui::text::{LINE_GAP, LINE_H, line_count};
    let room = PAGE_H - 25.0 - 4.0 - 9.0 - 2.0;
    let height = |scale: f32| -> f32 {
        text.split('\n')
            .map(|p| match p.is_empty() {
                true => 4.0,
                false => line_count(p, TEXT_W, scale) as f32 * (LINE_H * scale + LINE_GAP),
            })
            .sum()
    };
    let mut scale = 1.0;
    while scale > 0.6 && height(scale) > room {
        scale -= 0.05;
    }
    scale
}

// ── Right page: category list (home view) ────────────────────────────────────

fn build_categories_right(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    rx: f32,
    py: f32,
    sx: f32,
    sy: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    // Patchouli right-page layout (landing):
    //   "Categories" header at page-local y=0, centered
    //   Separator at page-local y=12 (drawn as bg sprite, not here)
    //   4-column 24×24 icon grid starting at page-local (10, 25)
    let cell_w = 24.0 * sx;
    let cell_h = 24.0 * sy;
    let grid_pad_left = 10.0 * sx;
    let header_h = 9.0 * sy;

    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 4.0, 4.0, 0.0)
        .gap(0.0);

    col = col.child(
        title_lbl(word("categories", ""))
            .color(theme.divider)
            .h(header_h)
            .align(Align::Center)
            .no_wrap(),
    );
    // Gap covering the separator region up to grid start (page-local y=25).
    col = col.child(widget::spacer().h(25.0 * sy - header_h));

    // 4-column icon grid; icons drawn as GL overlays at exact Patchouli positions.
    let cats = &book.categories;
    let icon_s = 16.0 * sx.min(sy);
    let mut row_i = 0usize;
    loop {
        let row_start = row_i * 4;
        if row_start >= cats.len() {
            break;
        }
        let mut row =
            widget::panel(FlexDir::Row)
                .h(cell_h)
                .gap(0.0)
                .padding(0.0, 0.0, 0.0, grid_pad_left);
        for col_i in 0..4 {
            let idx = row_start + col_i;
            if idx >= cats.len() {
                break;
            }
            let cat = &cats[idx];
            let selected = !state.at_home && idx == state.cat;
            let bg = if selected { theme.nav_selected_bg } else { 0 };

            if let Some(icon_id) = &cat.icon {
                push_icon(
                    overlays,
                    icon_id,
                    rx + (10.0 + col_i as f32 * 24.0 + 4.0) * sx,
                    py + (25.0 + row_i as f32 * 24.0 + 4.0) * sy,
                    icon_s,
                    icon_s,
                );
            }
            let cell = widget::panel(FlexDir::Column)
                .w(cell_w)
                .h(cell_h)
                .bg(bg)
                .on_click(format!("cat:{}", idx))
                .id(format!("book_cat_{}", idx));
            row = row.child(cell);
        }
        col = col.child(row);
        row_i += 1;
    }
    col
}

// ── Left page: entry content (entry view) ────────────────────────────────────

fn build_entry_left(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    page_w: f32,
    page_h: f32,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
    sep_cx: f32,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let entry = state.current_entry(book);
    let page = entry.and_then(|e| e.pages.get(state.page));
    let page_count = state.page_count(book);
    let title_text = entry.map(|e| e.name.as_str()).unwrap_or("");

    // Patchouli PageText page 0:
    //   title centered at page-local (PAGE_W/2, 0)
    //   separator at page-local (0, 12)  → centered ≡ sep_cx offset
    //   text body at page-local (0, 22)
    let title_h = 9.0 * sy;
    let sep_h_px = (SEP_H * sy).max(1.0);
    let sep_y = oy + 12.0 * sy;
    let body_oy = oy + 22.0 * sy;

    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Book,
        u: SEP_U,
        v: SEP_V,
        uw: SEP_W,
        uh: SEP_H,
        x: ox + sep_cx,
        y: sep_y,
        w: SEP_W * sx,
        h: sep_h_px,
    });

    let page_label = format!("{}/{}", state.page + 1, page_count);
    let nav = widget::panel(FlexDir::Row)
        .h(12.0 * sy)
        .gap(4.0)
        .padding(0.0, 2.0, 0.0, 2.0)
        .child(
            btn("◀")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("prev_page")
                .id("prev_page"),
        )
        .child(
            lbl(&page_label)
                .color(theme.nav)
                .flex(1.0)
                .align(Align::Center),
        )
        .child(
            btn("⌂")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("home")
                .id("book_home"),
        )
        .child(
            btn("▶")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("next_page")
                .id("next_page"),
        );

    let page_body = build_page(
        page, state.page, theme, recipes, bg_sprites, overlays, ox, body_oy, sx, sy,
    );

    widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 6.0, 4.0, 4.0)
        .gap(0.0)
        .child(
            title_lbl(title_text)
                .color(theme.title)
                .h(title_h)
                .align(Align::Center)
                .no_wrap(),
        )
        .child(widget::spacer().h((12.0 - 9.0) * sy)) // gap: title end → sep start
        .child(widget::spacer().h(sep_h_px)) // height occupied by sep sprite
        .child(widget::spacer().h((22.0 - 12.0 - SEP_H) * sy)) // gap: sep end → body
        .child(page_body)
        .child(nav)
}

// ── Right page: entry list for selected category (entry view) ─────────────────

fn build_entries_right(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    rx: f32,
    py: f32,
    sx: f32,
    sy: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let entries = state.entries_visible(book);
    let cat_name = book
        .categories
        .get(state.cat)
        .map(|c| c.name.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| word("entries", ""));
    let spread_count = state.list_spread_count(book);

    // Patchouli GuiBookEntryList layout (right page):
    //   category name at page-local y=0, centered
    //   separator at page-local y=12 (drawn as bg sprite in build_ui)
    //   entries start at page-local y=20, each h=11
    let header_h = 9.0 * sy;
    let row_h = 11.0 * sy;
    let icon_size = 8.0 * sx.min(sy);

    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 4.0, 8.0, 0.0)
        .gap(0.0);

    col = col.child(
        title_lbl(cat_name)
            .color(theme.divider)
            .h(header_h)
            .align(Align::Center),
    );
    // sep region: y=9..15 (sep h=3), then gap to entries start at y=20
    col = col.child(widget::spacer().h((12.0 - 9.0) * sy)); // gap to sep
    col = col.child(widget::spacer().h((SEP_H * sy).max(1.0))); // sep height
    col = col.child(widget::spacer().h((20.0 - 12.0 - SEP_H) * sy)); // gap to entries

    let abs_start = state.list_spread_start();
    for (i, entry) in entries.iter().enumerate() {
        let abs_i = abs_start + i;
        let selected = abs_i == state.entry;
        let bg = if selected { theme.nav_selected_bg } else { 0 };
        let color = if selected {
            theme.nav_selected
        } else {
            theme.nav
        };

        // Icon at page-local (1, 20 + i*11 + 1), 8×8 (Patchouli renders entry
        // icons at 0.5× scale).
        if let Some(icon_id) = &entry.icon {
            push_icon(
                overlays,
                icon_id,
                rx + 1.0 * sx,
                py + (20.0 + i as f32 * 11.0 + 1.0) * sy,
                icon_size,
                icon_size,
            );
        }

        let mut row = widget::panel(FlexDir::Row)
            .h(row_h)
            .gap(2.0)
            .bg(bg)
            .on_click(format!("entry:{}", abs_i))
            .id(format!("book_entry_{}", abs_i))
            .padding(1.0, 2.0, 1.0, 2.0);

        row = row.child(widget::spacer().w(icon_size + 2.0));
        // Long names shrink to the row (as titles do).
        let fit = ((PAGE_W - 26.0) / yog_ui::text::str_width(&entry.name, 1.0)).clamp(0.5, 1.0);
        row = row.child(lbl(&entry.name).color(color).font_scale(text_scale() * fit).flex(1.0).no_wrap());
        col = col.child(row);
    }

    if spread_count > 1 {
        col = col.child(widget::spacer().flex(1.0));
        let spread_label = format!("{}/{}", state.list_spread + 1, spread_count);
        col = col.child(
            widget::panel(FlexDir::Row)
                .h(12.0 * sy)
                .gap(2.0)
                .child(
                    btn("◀")
                        .w(12.0 * sx)
                        .h(12.0 * sy)
                        .color(theme.nav)
                        .on_click("prev_list")
                        .id("prev_list"),
                )
                .child(
                    lbl(&spread_label)
                        .color(theme.nav)
                        .flex(1.0)
                        .align(Align::Center),
                )
                .child(
                    btn("▶")
                        .w(12.0 * sx)
                        .h(12.0 * sy)
                        .color(theme.nav)
                        .on_click("next_list")
                        .id("next_list"),
                ),
        );
    }
    col
}

/// Draw a Patchouli-style crafting recipe: title, 100×62 grid background
/// (UV 0,0 on the crafting sheet), 3×N ingredient icons, output item.
/// `rx0`/`ry` are body-local recipe origin; `ox`/`oy` the body screen origin.
#[allow(clippy::too_many_arguments)]
fn draw_crafting_grid(
    theme: &BookTheme,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
    col: &mut widget::Widget,
    grid: &[Vec<String>],
    width: usize,
    output: &str,
    count: u32,
    shapeless: bool,
    ox: f32,
    oy: f32,
    rx0: f32,
    ry: f32,
    sx: f32,
    sy: f32,
) {
    let mut c = std::mem::replace(col, widget::panel(FlexDir::Column));
    c = c.child(
        title_lbl(pretty_item_name(output))
            .color(theme.title)
            .h(10.0 * sy)
            .align(Align::Center),
    );

    // Grid background: blit at (rx0-2, ry-2) size 100×62, UV(0,0).
    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Crafting,
        u: 0.0,
        v: 0.0,
        uw: 100.0,
        uh: 62.0,
        x: ox + (rx0 - 2.0) * sx,
        y: oy + (ry - 2.0) * sy,
        w: 100.0 * sx,
        h: 62.0 * sy,
    });
    // Shapeless marker: UV(0,64) 11×11 at (rx0+62, ry+2).
    if shapeless {
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Crafting,
            u: 0.0,
            v: 64.0,
            uw: 11.0,
            uh: 11.0,
            x: ox + (rx0 + 62.0) * sx,
            y: oy + (ry + 2.0) * sy,
            w: 11.0 * sx,
            h: 11.0 * sy,
        });
    }

    let icon_s = 16.0 * sx.min(sy);
    let width = width.max(1);
    for (i, slot) in grid.iter().enumerate() {
        if !slot.is_empty() {
            overlays.push(OverlayCmd::McItems {
                items: slot.clone(),
                x: ox + (rx0 + 3.0 + (i % width) as f32 * 19.0) * sx,
                y: oy + (ry + 3.0 + (i / width) as f32 * 19.0) * sy,
                w: icon_s,
                h: icon_s,
            });
        }
    }
    // Output item at (rx0+79, ry+22); stack count below it when > 1.
    overlays.push(OverlayCmd::McItem {
        item_id: output.to_owned(),
        x: ox + (rx0 + 79.0) * sx,
        y: oy + (ry + 22.0) * sy,
        w: icon_s,
        h: icon_s,
    });
    if count > 1 {
        overlays.push(OverlayCmd::McText {
            text: format!("x{}", count),
            x: ox + (rx0 + 81.0) * sx,
            y: oy + (ry + 40.0) * sy,
            color: theme.title,
        });
    }
    // Patchouli's "toast symbol" under the output: the crafting table.
    overlays.push(OverlayCmd::McItem {
        item_id: "minecraft:crafting_table".into(),
        x: ox + (rx0 + 79.0) * sx,
        y: oy + (ry + 41.0) * sy,
        w: icon_s,
        h: icon_s,
    });

    // Spacer from title end (10) to below the grid (ry+62+4).
    c = c.child(widget::spacer().h((ry + 62.0 + 4.0 - 10.0) * sy));
    *col = c;
}

// ── Page content builder ──────────────────────────────────────────────────────

/// Build the content widget for a single page.
/// `ox/oy` = screen top-left of the page body area.
/// `sx/sy` = book-local-to-screen scale factors.
fn build_page(
    page: Option<&BookPage>,
    page_num: usize,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
) -> widget::Widget {
    let mut col = widget::panel(FlexDir::Column).flex(1.0).gap(4.0);

    let Some(page) = page else {
        return col.child(lbl(word("no_entries", "")).color(theme.nav));
    };

    match page {
        BookPage::Text { text, title } => {
            // On non-first pages, show an optional section title + separator.
            if page_num > 0
                && let Some(t) = title
            {
                let sep_h_px = (SEP_H * sy).max(1.0);
                let title_h = 9.0 * sy;
                col = col.child(
                    title_lbl(t.as_str())
                        .color(theme.title)
                        .h(title_h)
                        .align(Align::Center)
                        .no_wrap(),
                );
                // sep at y=12 page-local, centered
                bg_sprites.push(BgSprite {
                    sheet: SpriteSheet::Book,
                    u: SEP_U,
                    v: SEP_V,
                    uw: SEP_W,
                    uh: SEP_H,
                    x: ox + (PAGE_W / 2.0 - SEP_W / 2.0) * sx,
                    y: oy + 12.0 * sy,
                    w: SEP_W * sx,
                    h: sep_h_px,
                });
                col = col.child(widget::spacer().h((12.0 - 9.0) * sy));
                col = col.child(widget::spacer().h(sep_h_px));
                col = col.child(widget::spacer().h((22.0 - 12.0 - SEP_H) * sy));
            }
            // One label: its lines are as far apart as the flowing counted.
            col = col.child(lbl(text.as_str()).color(theme.text));
        }

        BookPage::Spotlight { item, title, text } => {
            // Patchouli PageSpotlight (page-body-local coordinates):
            //   title at y=0 centered, item frame at (PAGE_W/2-33, 10) 66×26
            //   (UV 0,102 on the 128×256 crafting sheet), item at (PAGE_W/2-8, 15),
            //   text at y=40.
            bg_sprites.push(BgSprite {
                sheet: SpriteSheet::Crafting,
                u: 0.0,
                v: 102.0,
                uw: 66.0,
                uh: 26.0,
                x: ox + (PAGE_W / 2.0 - 33.0) * sx,
                y: oy + 10.0 * sy,
                w: 66.0 * sx,
                h: 26.0 * sy,
            });

            let item_name = title
                .clone()
                .or_else(|| item.name.clone())
                .unwrap_or_else(|| pretty_item_name(&item.id));
            col = col.child(
                title_lbl(item_name)
                    .color(theme.title)
                    .h(10.0 * sy)
                    .align(Align::Center),
            );

            let icon_s = 16.0 * sx.min(sy);
            overlays.push(OverlayCmd::McItem {
                item_id: item.id.clone(),
                x: ox + (PAGE_W / 2.0 - 8.0) * sx,
                y: oy + 15.0 * sy,
                w: icon_s,
                h: icon_s,
            });

            // Spacer from title end (y=10) to text start (y=40).
            col = col.child(widget::spacer().h(30.0 * sy));

            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Crafting { recipe_id, text } => {
            // Patchouli PageCrafting: recipe origin at page-local
            // (PAGE_W/2-49, 4); we place it in body-local coords below a title.
            let rx0 = PAGE_W / 2.0 - 49.0; // = 9
            let ry = 12.0; // body-local recipe y (title occupies 0..10)
            match recipes.get(recipe_id.as_str()) {
                Some(RecipeVis::Shaped {
                    grid,
                    width,
                    output,
                    count,
                }) => {
                    draw_crafting_grid(
                        theme, bg_sprites, overlays, &mut col, grid, *width, output, *count, false,
                        ox, oy, rx0, ry, sx, sy,
                    );
                }
                Some(RecipeVis::Shapeless {
                    ingredients,
                    output,
                    count,
                }) => {
                    draw_crafting_grid(
                        theme, bg_sprites, overlays, &mut col, ingredients, 3, output, *count, true, ox,
                        oy, rx0, ry, sx, sy,
                    );
                }
                _ => {
                    col = col.child(lbl(format!("[Recipe: {}]", recipe_id)).color(theme.nav));
                }
            }
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Smelting { recipe_id, text } => {
            // Patchouli PageSmelting: bg UV(11,71) 96×24, input at +4,+4,
            // output at +76,+4.
            let rx0 = PAGE_W / 2.0 - 49.0;
            let ry = 12.0;
            match recipes.get(recipe_id.as_str()) {
                Some(RecipeVis::Smelting { input, output, station }) => {
                    col = col.child(
                        title_lbl(pretty_item_name(output))
                            .color(theme.title)
                            .h(10.0 * sy)
                            .align(Align::Center),
                    );
                    bg_sprites.push(BgSprite {
                        sheet: SpriteSheet::Crafting,
                        u: 11.0,
                        v: 71.0,
                        uw: 96.0,
                        uh: 24.0,
                        x: ox + rx0 * sx,
                        y: oy + ry * sy,
                        w: 96.0 * sx,
                        h: 24.0 * sy,
                    });
                    let icon_s = 16.0 * sx.min(sy);
                    overlays.push(OverlayCmd::McItems {
                        items: input.clone(),
                        x: ox + (rx0 + 4.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    // Center: the furnace (blast furnace, …) itself (Patchouli's "toast symbol").
                    overlays.push(OverlayCmd::McItem {
                        item_id: station.clone(),
                        x: ox + (rx0 + 40.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    overlays.push(OverlayCmd::McItem {
                        item_id: output.clone(),
                        x: ox + (rx0 + 76.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    // Spacer from title end (10) to below the furnace strip (ry+24+4).
                    col = col.child(widget::spacer().h((ry + 24.0 + 4.0 - 10.0) * sy));
                }
                _ => {
                    col = col.child(lbl(format!("[Recipe: {}]", recipe_id)).color(theme.nav));
                }
            }
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Image {
            texture,
            title,
            text,
            ..
        } => {
            if let Some(t) = title {
                col = col.child(title_lbl(t.as_str()).color(theme.title));
            }
            col = col.child(
                widget::image(texture)
                    .region([0.0, 0.0, 80.0, 80.0])
                    .stretch(Stretch::Fill)
                    .w(80.0)
                    .h(80.0),
            );
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Svg { data, title, text } => {
            let top = if let Some(t) = title {
                col = col.child(title_lbl(t.as_str()).color(theme.title));
                12.0
            } else {
                0.0
            };
            overlays.push(OverlayCmd::Svg {
                data: data.clone(),
                x: ox + (PAGE_W / 2.0 - 32.0) * sx,
                y: oy + top * sy,
                w: 64.0 * sx,
                h: 64.0 * sy,
            });
            col = col.child(widget::spacer().h(68.0 * sy));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::CustomText { text, font, color } => {
            // A resource-pack font (assets/<ns>/font/<name>.json — a `ttf`
            // provider, say), at `size_px` for its 9-pixel line.
            col = col.child(
                lbl(text.as_str())
                    .font(font.font_id.as_str())
                    .font_scale(text_scale() * font.size_px / yog_ui::text::LINE_H)
                    .color(*color),
            );
        }

        BookPage::Relations { entries, text } => {
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
            col = col.child(title_lbl(word("see_also", "")).color(theme.title));
            for e in entries {
                col = col.child(lbl(format!("• {}", e)).color(theme.nav));
            }
        }

        BookPage::Multiblock { name, pattern, mapping, text } => {
            // A 70px view (under its name), its text below.
            let top = if let Some(n) = name {
                col = col.child(title_lbl(n.as_str()).color(theme.title).h(10.0 * sy));
                MB_NAME_H
            } else {
                0.0
            };
            overlays.push(OverlayCmd::Blocks {
                blocks: crate::multiblock_blocks(pattern, mapping),
                x: ox + (PAGE_W / 2.0 - MB_VIEW / 2.0) * sx,
                y: oy + top * sy,
                w: MB_VIEW * sx,
                h: MB_VIEW * sy,
            });
            col = col.child(widget::spacer().h(MB_VIEW * sy));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Entity {
            entity_type,
            name,
            text,
        } => {
            let display = name.clone().unwrap_or_else(|| entity_name(entity_type));
            col = col.child(title_lbl(display).color(theme.title));
            overlays.push(OverlayCmd::Entity {
                entity: entity_type.clone(),
                x: ox + (PAGE_W / 2.0 - 40.0) * sx,
                y: oy + 12.0 * sy,
                w: 80.0 * sx,
                h: 80.0 * sy,
            });
            col = col.child(widget::spacer().h(84.0 * sy));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Pattern {
            op_id,
            name,
            input,
            output,
            text,
            patterns,
            ..
        } => {
            col = col.child(title_lbl(name.as_deref().unwrap_or(op_id.as_str())).color(theme.title).h(10.0 * sy));
            // Hex Casting's page: the patterns in a box the page's width
            // under the name, then what it takes and gives.
            if !patterns.is_empty() {
                overlays.push(OverlayCmd::Svg {
                    data: crate::pattern::svg(patterns, PAGE_W, PATTERN_H),
                    x: ox,
                    y: oy + 14.0 * sy,
                    w: PAGE_W * sx,
                    h: PATTERN_H * sy,
                });
                col = col.child(widget::spacer().h(PATTERN_H * sy));
            }
            if !input.is_empty() || !output.is_empty() {
                col = col.child(lbl(format!("{{u|{input}}} → {{u|{output}}}")).color(theme.text));
            }
            if !text.is_empty() {
                col = col.child(lbl(text.as_str()).color(theme.text));
            }
        }

        BookPage::Empty => {}
    }

    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BookEntry;

    fn book(pages: Vec<BookPage>) -> Book {
        let mut book = Book::new("t:b", "B");
        book.entries.push(BookEntry { id: "e".into(), name: "E".into(), pages, ..Default::default() });
        book
    }

    #[test]
    fn long_text_flows_onto_new_pages() {
        let long = "word ".repeat(400);
        let flowed = flow(book(vec![BookPage::Text { text: long.clone(), title: None }, BookPage::Empty]));
        let pages = &flowed.entries[0].pages;
        assert!(pages.len() > 3, "{}", pages.len());
        assert!(matches!(pages.last(), Some(BookPage::Empty)), "what came after stays after");
        let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
        for p in &pages[..pages.len() - 1] {
            let BookPage::Text { text, .. } = p else { panic!("{p:?}") };
            assert!(text.lines().count() as f32 * line_h <= BODY_H + 1.0);
            assert!(text.lines().all(|l| yog_ui::text::str_width(l, 1.0) <= TEXT_W));
        }
        let words: usize = pages.iter().map(|p| match p {
            BookPage::Text { text, .. } => text.split_whitespace().count(),
            _ => 0,
        }).sum();
        assert_eq!(words, 400, "nothing lost");
    }

    #[test]
    fn edits_keep_the_reader_where_they_were() {
        let mut b = book(vec![BookPage::Text { text: "one".into(), title: None }, BookPage::Text { text: "two".into(), title: None }]);
        b.categories.push(crate::BookCategory { id: "c".into(), name: "C".into(), ..Default::default() });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b.clone());
        assert_eq!(r.shown(), None, "opens on the landing page");
        assert!(r.show("e", 1));
        assert_eq!(r.shown(), Some(("e".into(), 1)));
        r.prepare(800.0, 600.0);
        assert!(r.ui.is_some());

        b.entries[0].pages.push(BookPage::Empty);
        r.set_book(b);
        assert_eq!(r.shown(), Some(("e".into(), 1)));
        assert!(!r.show("missing", 0));
    }

    #[test]
    fn pages_are_counted_as_written() {
        let long = "word ".repeat(400);
        let mut b = book(vec![BookPage::Text { text: long, title: None }, BookPage::Empty]);
        b.categories.push(crate::BookCategory { id: "c".into(), name: "C".into(), ..Default::default() });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        assert!(r.show("e", 1));
        assert_eq!(r.shown(), Some(("e".into(), 1)), "the second page as written");
        assert!(r.state.page > 1, "past the first page's flowed text");
        r.handle_event("prev_page");
        assert_eq!(r.shown(), Some(("e".into(), 0)), "flowed text belongs to the first");
    }

    #[test]
    fn reads_yog_recipes() {
        let shaped = parse_recipe(r##"{"type":"shaped","pattern":["AB"],"key":{"A":"m:a","B":["m:x","#c:t"]},"result":"m:out","count":2}"##);
        let Some(RecipeVis::Shaped { grid, width, output, count }) = shaped else { panic!() };
        assert_eq!((width, output.as_str(), count), (2, "m:out", 2));
        assert_eq!(grid[1], ["m:x", "#c:t"]);
        let blast = parse_recipe(r##"{"type":"blasting","ingredient":"m:ore","result":"m:ingot"}"##);
        assert!(matches!(blast, Some(RecipeVis::Smelting { station, .. }) if station == "minecraft:blast_furnace"));
        assert!(parse_recipe(r#"{"type":"minecraft:crafting_shaped"}"#).is_none(), "vanilla JSON isn't the book's");
    }

    #[test]
    fn multiblocks_turn_and_animate() {
        let pattern = vec![vec!["G".to_string()], vec!["S ".to_string()]];
        let mapping = [("G".to_string(), "minecraft:glass".to_string()), ("S".to_string(), "minecraft:stone".to_string())].into();
        let blocks = crate::multiblock_blocks(&pattern, &mapping);
        assert_eq!(blocks, [((0, 1, 0), "minecraft:glass".to_string()), ((0, 0, 0), "minecraft:stone".to_string())]);

        let mut b = book(vec![BookPage::Multiblock { name: None, pattern, mapping, text: None }]);
        b.categories.push(crate::BookCategory { id: "c".into(), name: "C".into(), ..Default::default() });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        r.prepare(800.0, 600.0);
        assert!(!r.animated(), "the landing page is still");
        assert_eq!(landing_fit("short"), 1.0);
        assert!(landing_fit(&"many words here ".repeat(60)) < 1.0, "long landing texts shrink");
        r.show("e", 0);
        r.prepare(800.0, 600.0);
        assert!(r.animated(), "a structure turns");
    }

    #[test]
    fn short_text_stays_put() {
        let pages = vec![BookPage::Text { text: "Hello".into(), title: Some("T".into()) }];
        let flowed = flow(book(pages.clone()));
        assert_eq!(flowed.entries[0].pages.len(), 1);
    }
}
