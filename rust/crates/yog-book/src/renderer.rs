//! Book renderer: yog-ui lays the book out and draws its text and
//! buttons; everything else goes through `draw2d` too — the sprite
//! sheets, item icons, SVG pages, entities — so the game and Yog-IDLE's
//! book designer run the same code.

use std::collections::{BTreeMap, HashMap};
use yog_gfx::{Color, GfxContext, Rect, Vec2};
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

/// Blocks shown at a slot's angle (a station), as the game shows a block
/// item; a multiblock turns slowly about the vertical, swaying this far.
const SLOT_YAW: f32 = 225.0;
const MULTIBLOCK_YAW: f32 = -45.0;
const MULTIBLOCK_SWAY: f32 = 20.0;
/// How far blocks are tilted toward the viewer.
const BLOCK_PITCH: f32 = 30.0;

// ── Background sprite (texture blit, drawn before yog-ui) ────────────────────

#[derive(Clone, PartialEq)]
pub(crate) enum SpriteSheet {
    Book,
    Crafting,
    /// Any image source (a recipe layout's).
    Image(String),
}

/// A subrect blit from one of the embedded sprite sheets, drawn before yog-ui.
#[derive(Clone)]
pub(crate) struct BgSprite {
    pub sheet: SpriteSheet,
    /// Source rect in pixels on the sheet.
    pub src: Rect,
    /// Destination on screen.
    pub dst: Rect,
}

// ── Pending overlay draw commands ─────────────────────────────────────────────

/// Draws a mod's picture (`BookPage::Drawn`): `(ctx, drawer, data, [x, y, w, h])`.
pub type PageDrawer<'a> = &'a mut dyn FnMut(&GfxContext, &str, &yog_data::Value, [f32; 4]);

#[derive(Clone)]
pub(crate) enum OverlayCmd {
    /// A mod's picture: its drawer (`BookPage::Drawn`) draws `data` in the box.
    Drawn {
        drawer: String,
        data: yog_data::Value,
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
    /// Blocks in 3D.
    Blocks {
        blocks: Vec<((i32, i32, i32), String)>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        /// Slowly turning (a multiblock on show), else at a slot's angle (a station).
        turning: bool,
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
        color: Color,
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

/// A recipe as a book shows it: its kind (which [`crate::layout`] draws
/// it), each slot's ingredients (each one's alternatives; empty: none),
/// result and values.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecipeVis {
    pub kind: String,
    pub slots: BTreeMap<String, Vec<Vec<String>>>,
    pub output: String,
    pub count: u32,
    /// `time`, `experience`, a mod kind's `values` — for a layout's texts.
    pub values: BTreeMap<String, String>,
}

/// An ingredient in Yog's recipe format: an id, a `#tag`, or an array of
/// those (any of them).
fn ingredient(v: &yog_data::Value) -> Vec<String> {
    use yog_data::Value;
    match v {
        Value::Str(s) => vec![s.clone()],
        Value::Ref(r) => vec![r.id.clone()],
        Value::Tag(r) => vec![format!("#{}", r.id)],
        Value::List(a) => a.iter().flat_map(ingredient).collect(),
        _ => Vec::new(),
    }
}

/// A recipe in Yog's own format (what a `.yrecipe` says, ids resolved —
/// see `yog_registry::Recipe`), as the book shows it.
fn parse_recipe(v: &yog_data::Value) -> Option<RecipeVis> {
    use yog_data::Value;
    use yog_registry::recipe_kinds::{self, GRID_SIDE, fields};
    let kind = v.get(fields::TYPE)?.as_str()?.to_owned();
    let output = match v.get(fields::RESULT)? {
        Value::Ref(r) => r.id.clone(),
        r => r.as_str()?.to_owned(),
    };
    let count = v.get(fields::COUNT).and_then(|c| c.as_i64()).unwrap_or(1).max(0) as u32;
    let mut slots = BTreeMap::new();
    let mut values = BTreeMap::new();
    let text = |x: &Value| match x {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        x => x.as_str().map(str::to_owned).unwrap_or_default(),
    };
    match recipe_kinds::game_kind(&kind) {
        // The game's kind: its slots where its definition says they're written.
        Some(def) => {
            for (name, spec) in &def.slots.0 {
                let places = if spec.grid {
                    // The pattern's cells by position on the grid.
                    let key = v.get(fields::KEY)?.as_map()?;
                    let mut grid = vec![Vec::new(); GRID_SIDE * GRID_SIDE];
                    for (r, row) in v.get(fields::PATTERN)?.as_list()?.iter().filter_map(|p| p.as_str()).take(GRID_SIDE).enumerate() {
                        let mut buf = [0u8; 4];
                        for (c, ch) in row.chars().take(GRID_SIDE).enumerate() {
                            grid[r * GRID_SIDE + c] = key.get(ch.encode_utf8(&mut buf)).map(ingredient).unwrap_or_default();
                        }
                    }
                    grid
                } else {
                    let field = v.get(spec.field.as_deref()?);
                    match (field, spec.max) {
                        // Several places: a list of them.
                        (Some(list), max) if max > 1 => list.as_list()?.iter().map(ingredient).collect(),
                        // One place: an ingredient (or a list of alternatives).
                        (Some(x), _) => vec![ingredient(x)],
                        (None, _) if spec.required => return None,
                        (None, _) => vec![Vec::new()],
                    }
                };
                slots.insert(name.clone(), places);
            }
            for (name, _) in &def.values.0 {
                let given = v.get(name).map(text);
                if let Some(x) = given.or_else(|| def.default_of(name).map(|d| d.to_string())) {
                    values.insert(name.clone(), x);
                }
            }
        }
        // A mod's own kind: named slots, values.
        None => {
            for (name, list) in v.get(fields::SLOTS).and_then(|s| s.as_map()).into_iter().flat_map(|m| m.iter()) {
                slots.insert(name.to_string(), list.as_list().into_iter().flatten().map(ingredient).collect());
            }
            for (name, x) in v.get(fields::VALUES).and_then(|s| s.as_map()).into_iter().flat_map(|m| m.iter()) {
                values.insert(name.to_string(), text(x));
            }
        }
    }
    Some(RecipeVis { kind, slots, output, count, values })
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
    tr.and_then(|tr| tr(&k))
        .filter(|t| *t != k)
        .unwrap_or_else(|| name_from_path(path))
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
        let (flowed, origins) = flow_traced(written.clone());
        // Translated, it's flowed again from the book as written: flowing
        // its keys would split them.
        let source = book.i18n.then_some(book);
        let book = flowed;
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
            translator: None,
            probe: None,
            source,
        }
    }

    /// The book it draws (its long texts flowed onto more pages).
    pub fn book(&self) -> &Book {
        &self.book
    }

    /// The game's translations: items' names in recipe titles, and, for
    /// [`Book::i18n`] books, the book's texts — translation keys, looked up
    /// with `tr` (unknown keys stay as they are) and re-looked up whenever
    /// the language changes.
    pub fn set_translator(&mut self, tr: fn(&str) -> Option<String>) {
        *ITEM_NAMES.write().unwrap_or_else(|e| e.into_inner()) = Some(tr);
        if self.book.i18n {
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
        self.written = src.localized(|k| {
            crate::markup::resolve(&tr(k).unwrap_or_else(|| k.to_string()), &accent)
        });
        (self.book, self.origins) = flow_traced(self.written.clone());
        self.probe = probe;
        self.dirty = true;
    }

    pub fn handle_event(&mut self, ev: &str) {
        if self.state.handle(ev, &self.book) {
            self.dirty = true;
        }
    }

    /// Register a recipe (Yog's recipe, as Yog Data) so recipe pages can
    /// render it visually. Unknown recipe types are ignored.
    pub fn add_recipe(&mut self, id: impl Into<String>, recipe: &yog_data::Value) {
        if let Some(vis) = parse_recipe(recipe) {
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
    /// Draws the book; mods' pictures (`BookPage::Drawn`) are left empty —
    /// see [`BookRenderer::render_with`].
    pub fn render(&mut self, ctx: &GfxContext, sw: f32, sh: f32) {
        self.render_with(ctx, sw, sh, &mut |_, _, _, _| {});
    }

    /// Draws the book, mods' pictures with `draw(ctx, drawer, data, [x, y,
    /// w, h])` (screen pixels) — the game calls the mod that registered
    /// `drawer`.
    pub fn render_with(&mut self, ctx: &GfxContext, sw: f32, sh: f32, draw: PageDrawer<'_>) {
        // Text flows by the host's own font metrics once it can measure.
        yog_ui::text::use_host_metrics(ctx);
        if !self.measured {
            self.measured = true;
            self.reflow();
        }
        let (bx, by, bw, bh) = self.prepare(sw, sh);
        let d2 = ctx.draw2d();
        let book_sheet = book_sheet(&self.book.book_texture);
        d2.image_region(&book_sheet, Rect::new(bx, by, bw, bh), Some(Rect::new(0.0, 0.0, BK_W, BK_H)), Color::WHITE);
        for spr in &self.bg_sprites {
            let sheet = match &spr.sheet {
                SpriteSheet::Book => &book_sheet,
                SpriteSheet::Crafting => CRAFTING_SHEET,
                SpriteSheet::Image(source) => source.as_str(),
            };
            d2.image_region(sheet, spr.dst, Some(spr.src), Color::WHITE);
        }
        if let Some(ui) = &mut self.ui {
            ui.render(ctx);
        }
        // Animations: entities and structures turn (Patchouli's pace), an
        // ingredient's alternatives take turns every second.
        let t = self.start.elapsed().as_secs_f32();
        for ov in &self.overlays {
            match *ov {
                OverlayCmd::Drawn {
                    ref drawer,
                    ref data,
                    x,
                    y,
                    w,
                    h,
                } => draw(ctx, drawer, data, [x, y, w, h]),
                OverlayCmd::Entity {
                    ref entity,
                    x,
                    y,
                    w,
                    h,
                } => d2.entity(entity, Rect::new(x, y, w, h), -30.0 + t * 30.0, 0.0),
                OverlayCmd::Blocks {
                    ref blocks,
                    x,
                    y,
                    w,
                    h,
                    turning,
                } => {
                    let list: Vec<((i32, i32, i32), &str)> =
                        blocks.iter().map(|(p, b)| (*p, b.as_str())).collect();
                    let yaw = if turning { MULTIBLOCK_YAW + t * MULTIBLOCK_SWAY } else { SLOT_YAW };
                    d2.blocks(&list, Rect::new(x, y, w, h), yaw, BLOCK_PITCH);
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
                OverlayCmd::McItems {
                    ref items,
                    x,
                    y,
                    w,
                    h,
                } => {
                    let shown: Vec<&String> =
                        items.iter().filter(|i| !i.starts_with('#')).collect();
                    if !shown.is_empty() {
                        let item = shown[second % shown.len()];
                        ctx.draw2d().item(&normalize_item_id(item), Vec2::new(x, y), w.min(h));
                    }
                }
                OverlayCmd::McItem {
                    ref item_id,
                    x,
                    y,
                    w,
                    h,
                } => {
                    ctx.draw2d()
                        .item(&normalize_item_id(item_id), Vec2::new(x, y), w.min(h));
                }
                OverlayCmd::McText {
                    ref text,
                    x,
                    y,
                    color,
                } => {
                    ctx.draw2d().text(text, Vec2::new(x, y), color, false);
                }
                OverlayCmd::McTexture {
                    ref path,
                    x,
                    y,
                    w,
                    h,
                } => {
                    // Standalone icon texture, not an atlas region — drawn
                    // whole, scaled into the icon box.
                    ctx.draw2d().image(path, Rect::new(x, y, w, h));
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
            OverlayCmd::McItems { items, .. } => {
                items.iter().filter(|i| !i.starts_with('#')).count() > 1
            }
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
        let Some(e) = self.book.entries.iter().find(|e| e.id == entry) else {
            return false;
        };
        let Some(cat) = self.book.categories.iter().position(|c| c.id == e.category) else {
            return false;
        };
        self.state.cat = cat;
        self.state.at_home = false;
        let in_cat = self.state.entries_in_cat(&self.book);
        let Some(i) = in_cat.iter().position(|x| x.id == entry) else {
            return false;
        };
        let shown = self
            .origins
            .get(entry)
            .and_then(|o| o.iter().position(|&w| w >= page))
            .unwrap_or(page);
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
        let page = self
            .origins
            .get(&e.id)
            .and_then(|o| o.get(self.state.page))
            .copied()
            .unwrap_or(self.state.page);
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
        self.probe = None;
        self.source = book.i18n.then_some(book);
        (self.book, self.origins) = flow_traced(self.written.clone());
        self.localize();
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
const MB_NAME_H: f32 = 14.0;

/// How much of the body a page's own content takes above its text.
fn content_above_text(page: &BookPage) -> Option<f32> {
    Some(match page {
        BookPage::Text { title: Some(_), .. } => 22.0,
        BookPage::Text { .. } => 0.0,
        BookPage::Spotlight { .. } => 40.0,
        // (A crafting grid's height: a furnace's is less.)
        BookPage::Recipe { .. } => 80.0,
        BookPage::Image { .. } | BookPage::Entity { .. } => 104.0,
        BookPage::Drawn { title, height, caption, .. } => {
            let title = if title.is_some() { 14.0 } else { 0.0 };
            let picture = if *height > 0.0 { height + 4.0 } else { 0.0 };
            title + picture + if caption.is_some() { 14.0 } else { 0.0 }
        }
        BookPage::Multiblock { name, .. } => {
            MB_VIEW + 4.0 + if name.is_some() { MB_NAME_H } else { 0.0 }
        }
        BookPage::Relations { entries, .. } => 12.0 + entries.len() as f32 * 10.0,
        _ => return None,
    })
}

fn page_text_mut(page: &mut BookPage) -> Option<&mut String> {
    match page {
        BookPage::Text { text, .. } => Some(text),
        BookPage::Spotlight { text, .. }
        | BookPage::Recipe { text, .. }
        | BookPage::Image { text, .. }
        | BookPage::Entity { text, .. }
        | BookPage::Multiblock { text, .. }
        | BookPage::Relations { text, .. }
        | BookPage::Drawn { text, .. } => text.as_mut(),
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
            let split = split_page(page, &book.page_types, &fits);
            from.extend(std::iter::repeat_n(written, split.len()));
            pages.extend(split);
        }
        entry.pages = pages;
        origins.insert(entry.id.clone(), from);
    }
    (book, origins)
}

/// A mod's page's room for its text: the property its last text shows
/// (`<BookText text="{x:Bind text}"/>`, nothing else in it), and the
/// height the rest of the page takes, in book pixels.
fn custom_text(page: &crate::page_type::CustomPage, types: &[crate::page_type::PageType]) -> Option<(String, f32)> {
    use crate::page_type::PageElement as E;
    let t = types.iter().find(|t| t.id == page.kind)?;
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    let shown: Vec<&E> = t.body.iter().filter(|el| match el {
        E::Header { visible, .. } | E::Text { visible, .. } | E::Item { visible, .. } | E::Image { visible, .. } | E::Drawn { visible, .. } => t.visible(page, visible),
        E::Spacer { .. } => true,
    }).collect();
    let at = shown.iter().rposition(|el| matches!(el, E::Text { .. }))?;
    let E::Text { text, .. } = shown[at] else { return None };
    let name = text.trim().strip_prefix("{x:Bind ")?.strip_suffix('}')?.trim();
    if !matches!(page.fields.get(name), Some(yog_data::Value::Str(_))) {
        return None;
    }
    let others: f32 = shown.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, el)| match el {
        E::Header { .. } => 14.0,
        E::Text { text, .. } => t.text(page, text).split('\n').map(|p| yog_ui::text::wrap_text(p, TEXT_W, 1.0).len().max(1)).sum::<usize>() as f32 * line_h + 4.0,
        E::Item { .. } => 20.0,
        E::Image { height, .. } | E::Drawn { height, .. } | E::Spacer { height } => height + 4.0,
    }).sum();
    Some((name.to_string(), others))
}

/// `page`, and the text pages its overflowing text goes on to.
fn split_page(mut page: BookPage, types: &[crate::page_type::PageType], fits: &dyn Fn(f32) -> usize) -> Vec<BookPage> {
    let (above, text) = match &mut page {
        BookPage::Custom(c) => {
            let Some((name, above)) = custom_text(c, types) else { return vec![page] };
            let Some(yog_data::Value::Str(text)) = c.fields.get_mut(&name) else { return vec![page] };
            (above, text)
        }
        _ => {
            let Some(above) = content_above_text(&page) else {
                return vec![page];
            };
            let Some(text) = page_text_mut(&mut page) else {
                return vec![page];
            };
            (above, text)
        }
    };
    let lines: Vec<String> = text
        .split('\n')
        .flat_map(|para| {
            if para.is_empty() {
                vec![String::new()]
            } else {
                yog_ui::text::wrap_text(para, TEXT_W, 1.0)
            }
        })
        .collect();
    let first = fits(BODY_H - above);
    if lines.len() <= first {
        return vec![page];
    }
    *text = lines[..first].join("\n");
    let rest: Vec<BookPage> = lines[first..]
        .chunks(fits(BODY_H))
        .map(|chunk| BookPage::Text {
            text: chunk.join("\n"),
            title: None,
            font: None,
            font_size: None,
        })
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
            src: Rect::new(0.0, 180.0, 140.0, 31.0),
            dst: Rect::new(bx + (LEFT_X - 8.0) * sx, by + 12.0 * sy, 140.0 * sx, 31.0 * sy),
        });
    }
    // Separator on the right page below its header ("Categories" or the
    // category name; y=12 page-local), centered.
    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Book,
        src: Rect::new(SEP_U, SEP_V, SEP_W, SEP_H),
        dst: Rect::new(rx + sep_cx, by + (TOP_PAD + 12.0) * sy, SEP_W * sx, (SEP_H * sy).max(1.0)),
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
            let bg = if selected { theme.nav_selected_bg } else { Color::TRANSPARENT };

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
        src: Rect::new(SEP_U, SEP_V, SEP_W, SEP_H),
        dst: Rect::new(ox + sep_cx, sep_y, SEP_W * sx, sep_h_px),
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
        page,
        state.page,
        theme,
        recipes,
        &book.layouts,
        &book.page_types,
        bg_sprites,
        overlays,
        ox,
        body_oy,
        sx,
        sy,
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
        let bg = if selected { theme.nav_selected_bg } else { Color::TRANSPARENT };
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
        row = row.child(
            lbl(&entry.name)
                .color(color)
                .font_scale(text_scale() * fit)
                .flex(1.0)
                .no_wrap(),
        );
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

/// Draws `vis` the way `layout` lays it out, under a title with its result's
/// name, centred on the page (body-local coordinates, like Patchouli's pages).
#[allow(clippy::too_many_arguments)]
fn draw_layout(
    theme: &BookTheme,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
    col: &mut widget::Widget,
    vis: &RecipeVis,
    layout: &crate::layout::RecipeLayout,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
) {
    use crate::layout::{FRAME_UV, LayoutPart};
    let mut c = std::mem::replace(col, widget::panel(FlexDir::Column));
    c = c.child(
        title_lbl(pretty_item_name(&vis.output))
            .color(theme.title)
            .h(10.0 * sy)
            .align(Align::Center),
    );
    let (lx, ly) = (PAGE_W / 2.0 - 1.0 - layout.size[0] / 2.0, 10.0);
    let at = |p: [f32; 2]| (ox + (lx + p[0]) * sx, oy + (ly + p[1]) * sy);
    let icon_s = 16.0 * sx.min(sy);
    let frame = |bg: &mut Vec<BgSprite>, p: [f32; 2]| {
        let (x, y) = at([p[0] - 4.0, p[1] - 4.0]);
        let [u, v, uw, uh] = FRAME_UV;
        bg.push(BgSprite {
            sheet: SpriteSheet::Crafting,
            src: Rect::new(u, v, uw, uh),
            dst: Rect::new(x, y, uw * sx, uh * sy),
        });
    };
    for part in &layout.parts {
        match part {
            LayoutPart::Image { source, uv, at: p } => {
                let (x, y) = at(*p);
                let sheet = if source == CRAFTING_SHEET {
                    SpriteSheet::Crafting
                } else {
                    SpriteSheet::Image(source.clone())
                };
                let [u, v, uw, uh] = *uv;
                bg_sprites.push(BgSprite {
                    sheet: sheet,
                    src: Rect::new(u, v, uw, uh),
                    dst: Rect::new(x, y, uw * sx, uh * sy),
                });
            }
            LayoutPart::Slots {
                slot,
                at: places,
                frame: framed,
            } => {
                let ingredients = vis.slots.get(slot);
                for (i, p) in places.iter().enumerate() {
                    if *framed {
                        frame(bg_sprites, *p);
                    }
                    if let Some(items) =
                        ingredients.and_then(|l| l.get(i)).filter(|x| !x.is_empty())
                    {
                        let (x, y) = at(*p);
                        overlays.push(OverlayCmd::McItems {
                            items: items.clone(),
                            x,
                            y,
                            w: icon_s,
                            h: icon_s,
                        });
                    }
                }
            }
            LayoutPart::Result {
                at: p,
                frame: framed,
            } => {
                if *framed {
                    frame(bg_sprites, *p);
                }
                let (x, y) = at(*p);
                overlays.push(OverlayCmd::McItem {
                    item_id: vis.output.clone(),
                    x,
                    y,
                    w: icon_s,
                    h: icon_s,
                });
            }
            LayoutPart::Count { at: p } => {
                if vis.count > 1 {
                    let (x, y) = at(*p);
                    overlays.push(OverlayCmd::McText {
                        text: format!("x{}", vis.count),
                        x,
                        y,
                        color: theme.title,
                    });
                }
            }
            LayoutPart::Block { block, at: p } => {
                let (x, y) = at(*p);
                overlays.push(OverlayCmd::Blocks { blocks: vec![((0, 0, 0), block.clone())], x, y, w: icon_s, h: icon_s, turning: false });
            }
            LayoutPart::Item { item, at: p } => {
                let (x, y) = at(*p);
                overlays.push(OverlayCmd::McItem {
                    item_id: item.clone(),
                    x,
                    y,
                    w: icon_s,
                    h: icon_s,
                });
            }
            LayoutPart::Text { text, at: p } => {
                let mut shown = text.clone();
                for (name, value) in &vis.values {
                    shown = shown.replace(&format!("{{{name}}}"), value);
                }
                let (x, y) = at(*p);
                overlays.push(OverlayCmd::McText {
                    text: shown,
                    x,
                    y,
                    color: theme.text,
                });
            }
        }
    }
    // The title is 10 high; the layout, and a little room under it.
    c = c.child(widget::spacer().h((layout.size[1] + 6.0) * sy));
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
    layouts: &[crate::layout::RecipeLayout],
    page_types: &[crate::page_type::PageType],
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
        BookPage::Text {
            text,
            title,
            font,
            font_size,
        } => {
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
                    src: Rect::new(SEP_U, SEP_V, SEP_W, SEP_H),
                    dst: Rect::new(ox + (PAGE_W / 2.0 - SEP_W / 2.0) * sx, oy + 12.0 * sy, SEP_W * sx, sep_h_px),
                });
                col = col.child(widget::spacer().h((12.0 - 9.0) * sy));
                col = col.child(widget::spacer().h(sep_h_px));
                col = col.child(widget::spacer().h((22.0 - 12.0 - SEP_H) * sy));
            }
            // One label: its lines are as far apart as the flowing counted.
            let mut l = lbl(text.as_str()).color(theme.text);
            // Another font (a resource-pack one), `font_size` pixels a line.
            if let Some(f) = font {
                l = l.font(f.as_str()).font_scale(
                    text_scale() * font_size.unwrap_or(yog_ui::text::LINE_H) / yog_ui::text::LINE_H,
                );
            }
            col = col.child(l);
        }

        BookPage::Spotlight { item, title, text } => {
            // Patchouli PageSpotlight (page-body-local coordinates):
            //   title at y=0 centered, item frame at (PAGE_W/2-33, 10) 66×26
            //   (UV 0,102 on the 128×256 crafting sheet), item at (PAGE_W/2-8, 15),
            //   text at y=40.
            bg_sprites.push(BgSprite {
                sheet: SpriteSheet::Crafting,
                src: Rect::new(0.0, 102.0, 66.0, 26.0),
                dst: Rect::new(ox + (PAGE_W / 2.0 - 33.0) * sx, oy + 10.0 * sy, 66.0 * sx, 26.0 * sy),
            });

            let item_name = title.clone().unwrap_or_else(|| pretty_item_name(item));
            col = col.child(
                title_lbl(item_name)
                    .color(theme.title)
                    .h(10.0 * sy)
                    .align(Align::Center),
            );

            let icon_s = 16.0 * sx.min(sy);
            overlays.push(OverlayCmd::McItem {
                item_id: item.clone(),
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

        BookPage::Recipe { recipe, text } => {
            match recipes
                .get(recipe.as_str())
                .and_then(|vis| crate::layout::find(layouts, &vis.kind).map(|l| (vis, l)))
            {
                Some((vis, layout)) => draw_layout(
                    theme, bg_sprites, overlays, &mut col, vis, layout, ox, oy, sx, sy,
                ),
                None => col = col.child(lbl(format!("[Recipe: {}]", recipe)).color(theme.nav)),
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
                    .region(Rect::new(0.0, 0.0, 80.0, 80.0))
                    .stretch(Stretch::Fill)
                    .w(80.0)
                    .h(80.0),
            );
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
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

        BookPage::Multiblock {
            name,
            pattern,
            mapping,
            text,
        } => {
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
                turning: true,
            });
            col = col.child(widget::spacer().h(MB_VIEW * sy));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Entity { entity, name, text } => {
            let display = name.clone().unwrap_or_else(|| entity_name(entity));
            col = col.child(title_lbl(display).color(theme.title));
            overlays.push(OverlayCmd::Entity {
                entity: entity.clone(),
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

        BookPage::Drawn {
            drawer,
            title,
            height,
            caption,
            text,
            data,
        } => {
            let mut top = 0.0;
            if let Some(t) = title {
                col = col.child(title_lbl(t.as_str()).color(theme.title).h(10.0 * sy));
                top = 14.0;
            }
            // The picture: a box the page's width, drawn by the mod's code.
            if *height > 0.0 {
                overlays.push(OverlayCmd::Drawn {
                    drawer: drawer.clone(),
                    data: data.clone(),
                    x: ox,
                    y: oy + top * sy,
                    w: PAGE_W * sx,
                    h: height * sy,
                });
                col = col.child(widget::spacer().h(height * sy));
            }
            if let Some(c) = caption {
                col = col.child(lbl(c.as_str()).color(theme.text));
            }
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Empty => {}

        // A mod's own kind of page: its type's elements, top to bottom.
        BookPage::Custom(c) => {
            use crate::page_type::PageElement as E;
            let Some(t) = page_types.iter().find(|t| t.id == c.kind) else {
                return col.child(lbl(format!("[{}]", c.kind)).color(theme.nav));
            };
            let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
            // How far down the next element starts, in book pixels (pictures are overlays).
            let mut top = 0.0;
            for el in &t.body {
                match el {
                    E::Header { text, visible } if t.visible(c, visible) => {
                        col = col.child(title_lbl(t.text(c, text)).color(theme.title).h(10.0 * sy));
                        top += 14.0;
                    }
                    E::Text { text, visible } if t.visible(c, visible) => {
                        let words = t.text(c, text);
                        let lines: usize = words.split('\n').map(|p| yog_ui::text::wrap_text(p, TEXT_W, 1.0).len().max(1)).sum();
                        col = col.child(lbl(words).color(theme.text));
                        top += lines as f32 * line_h + 4.0;
                    }
                    E::Item { item, visible } if t.visible(c, visible) => {
                        let id = crate::page_type::words(&t.value(c, item));
                        push_icon(overlays, &id, ox + (PAGE_W / 2.0 - 8.0) * sx, oy + top * sy, 16.0 * sx, 16.0 * sy);
                        col = col.child(widget::spacer().h(16.0 * sy));
                        top += 20.0;
                    }
                    E::Image { image, height, visible } if t.visible(c, visible) => {
                        let src = crate::page_type::words(&t.value(c, image));
                        col = col.child(widget::image(src).stretch(Stretch::Fill).w(PAGE_W * sx).h(height * sy));
                        top += height + 4.0;
                    }
                    E::Drawn { drawer, height, data, visible } if t.visible(c, visible) => {
                        overlays.push(OverlayCmd::Drawn {
                            drawer: drawer.clone(),
                            data: t.value(c, data),
                            x: ox,
                            y: oy + top * sy,
                            w: PAGE_W * sx,
                            h: height * sy,
                        });
                        col = col.child(widget::spacer().h(height * sy));
                        top += height + 4.0;
                    }
                    E::Spacer { height } => {
                        col = col.child(widget::spacer().h(height * sy));
                        top += height + 4.0;
                    }
                    _ => {}
                }
            }
        }
    }

    col
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(json: &str) -> yog_data::Value {
        yog_data::json::from_json_text(json).unwrap()
    }
    use crate::BookEntry;

    fn book(pages: Vec<BookPage>) -> Book {
        let mut book = Book {
            id: "t:b".into(),
            name: "B".into(),
            ..Default::default()
        };
        book.entries.push(BookEntry {
            id: "e".into(),
            name: "E".into(),
            pages,
            ..Default::default()
        });
        book
    }

    #[test]
    fn long_text_flows_onto_new_pages() {
        let long = "word ".repeat(400);
        let flowed = flow(book(vec![
            BookPage::Text {
                text: long.clone(),
                title: None,
                font: None,
                font_size: None,
            },
            BookPage::Empty,
        ]));
        let pages = &flowed.entries[0].pages;
        assert!(pages.len() > 3, "{}", pages.len());
        assert!(
            matches!(pages.last(), Some(BookPage::Empty)),
            "what came after stays after"
        );
        let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
        for p in &pages[..pages.len() - 1] {
            let BookPage::Text { text, .. } = p else {
                panic!("{p:?}")
            };
            assert!(text.lines().count() as f32 * line_h <= BODY_H + 1.0);
            assert!(
                text.lines()
                    .all(|l| yog_ui::text::str_width(l, 1.0) <= TEXT_W)
            );
        }
        let words: usize = pages
            .iter()
            .map(|p| match p {
                BookPage::Text { text, .. } => text.split_whitespace().count(),
                _ => 0,
            })
            .sum();
        assert_eq!(words, 400, "nothing lost");
    }

    #[test]
    fn edits_keep_the_reader_where_they_were() {
        let mut b = book(vec![
            BookPage::Text {
                text: "one".into(),
                title: None,
                font: None,
                font_size: None,
            },
            BookPage::Text {
                text: "two".into(),
                title: None,
                font: None,
                font_size: None,
            },
        ]);
        b.categories.push(crate::BookCategory {
            id: "c".into(),
            name: "C".into(),
            ..Default::default()
        });
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
        let mut b = book(vec![
            BookPage::Text {
                text: long,
                title: None,
                font: None,
                font_size: None,
            },
            BookPage::Empty,
        ]);
        b.categories.push(crate::BookCategory {
            id: "c".into(),
            name: "C".into(),
            ..Default::default()
        });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        assert!(r.show("e", 1));
        assert_eq!(
            r.shown(),
            Some(("e".into(), 1)),
            "the second page as written"
        );
        assert!(r.state.page > 1, "past the first page's flowed text");
        r.handle_event("prev_page");
        assert_eq!(
            r.shown(),
            Some(("e".into(), 0)),
            "flowed text belongs to the first"
        );
    }

    #[test]
    fn reads_yog_recipes() {
        let shaped = parse_recipe(&j(r##"{"type":"shaped","pattern":["AB"],"key":{"A":"m:a","B":["m:x","#c:t"]},"result":"m:out","count":2}"##)).unwrap();
        assert_eq!(
            (shaped.kind.as_str(), shaped.output.as_str(), shaped.count),
            ("shaped", "m:out", 2)
        );
        let grid = &shaped.slots["grid"];
        assert_eq!(
            (grid.len(), &grid[1]),
            (9, &vec!["m:x".to_string(), "#c:t".to_string()]),
            "cells by place on the 3×3 grid"
        );
        let blast =
            parse_recipe(&j(r##"{"type":"blasting","ingredient":"m:ore","result":"m:ingot"}"##))
                .unwrap();
        assert_eq!(
            (
                blast.slots["input"][0][0].as_str(),
                blast.values["cook_time"].as_str()
            ),
            ("m:ore", "100")
        );
        let own = parse_recipe(&j(r##"{"type":"m:infusion","slots":{"ring":["m:a",["m:b","m:c"]]},"values":{"time":40},"result":"m:out"}"##)).unwrap();
        assert_eq!(
            (own.slots["ring"].len(), own.values["time"].as_str()),
            (2, "40")
        );
        assert!(
            parse_recipe(&j(r#"{"type":"minecraft:crafting_shaped"}"#)).is_none(),
            "vanilla JSON isn't the book's"
        );
    }

    #[test]
    fn recipe_pages_are_drawn_by_layouts() {
        let theme = BookTheme::default();
        let mut recipes = HashMap::new();
        recipes.insert("m:r".to_string(), parse_recipe(&j(r##"{"type":"m:infusion","slots":{"ring":["m:a","m:b"]},"values":{"time":40},"result":"m:out"}"##)).unwrap());
        recipes.insert("m:s".to_string(), parse_recipe(&j(r##"{"type":"smithing","template":"m:t","base":"m:b","addition":"m:a","result":"m:out"}"##)).unwrap());
        recipes.insert("m:p".to_string(), parse_recipe(&j(r##"{"type":"brewing","from":"m:awkward","ingredient":"m:dust","result":"m:clarity"}"##)).unwrap());
        let layout: crate::layout::RecipeLayout = yog_data::from_text(
            r#"{ kinds: ["m:infusion"], size: [96, 40], parts: [
                { type: "slots", slot: "ring", at: [[0, 0], [20, 0]], frame: true },
                { type: "result", at: [76, 10] },
                { type: "text", text: "{time} ticks", at: [0, 30] },
            ] }"#,
        )
        .unwrap()
        .value;
        let draw = |id: &str| {
            let (mut bg, mut ov) = (Vec::new(), Vec::new());
            let page = BookPage::Recipe {
                recipe: id.into(),
                text: None,
            };
            build_page(
                Some(&page),
                0,
                &theme,
                &recipes,
                std::slice::from_ref(&layout),
                &[],
                &mut bg,
                &mut ov,
                0.0,
                0.0,
                1.0,
                1.0,
            );
            (bg, ov)
        };
        let (bg, ov) = draw("m:r");
        let lx = PAGE_W / 2.0 - 1.0 - 48.0;
        let items: Vec<(f32, f32)> = ov
            .iter()
            .filter_map(|o| match o {
                OverlayCmd::McItems { x, y, .. } | OverlayCmd::McItem { x, y, .. } => {
                    Some((*x, *y))
                }
                _ => None,
            })
            .collect();
        assert_eq!(items, [(lx, 10.0), (lx + 20.0, 10.0), (lx + 76.0, 20.0)]);
        assert_eq!(bg.len(), 2, "a frame behind each ring slot");
        assert!(
            ov.iter()
                .any(|o| matches!(o, OverlayCmd::McText { text, .. } if text == "40 ticks"))
        );
        // The game's kinds: built-in layouts (smithing had none before).
        let (bg, ov) = draw("m:s");
        assert!(
            bg.iter().any(|b| b.src.x == 11.0 && b.src.y == 135.0),
            "Patchouli's smithing strip"
        );
        assert_eq!(
            ov.iter()
                .filter(|o| matches!(o, OverlayCmd::McItems { .. }))
                .count(),
            3
        );
        // Brewing: Yog's own strip — ingredient, potion, the stand, result.
        let (bg, ov) = draw("m:p");
        assert!(bg.iter().any(|b| b.src.x == 11.0 && b.src.y == 181.0), "the brewing strip");
        // Its two slots (ingredient, potion); the stand and the result are drawn apart.
        assert_eq!(ov.iter().filter(|o| matches!(o, OverlayCmd::McItems { .. })).count(), 2);
    }

    #[test]
    fn multiblocks_turn_and_animate() {
        let pattern = vec![vec!["G".to_string()], vec!["S ".to_string()]];
        let mapping = [
            ("G".to_string(), "minecraft:glass".to_string()),
            ("S".to_string(), "minecraft:stone".to_string()),
        ]
        .into();
        let blocks = crate::multiblock_blocks(&pattern, &mapping);
        assert_eq!(
            blocks,
            [
                ((0, 1, 0), "minecraft:glass".to_string()),
                ((0, 0, 0), "minecraft:stone".to_string())
            ]
        );

        let mut b = book(vec![BookPage::Multiblock {
            name: None,
            pattern,
            mapping,
            text: None,
        }]);
        b.categories.push(crate::BookCategory {
            id: "c".into(),
            name: "C".into(),
            ..Default::default()
        });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        r.prepare(800.0, 600.0);
        assert!(!r.animated(), "the landing page is still");
        assert_eq!(landing_fit("short"), 1.0);
        assert!(
            landing_fit(&"many words here ".repeat(60)) < 1.0,
            "long landing texts shrink"
        );
        r.show("e", 0);
        r.prepare(800.0, 600.0);
        assert!(r.animated(), "a structure turns");
    }

    #[test]
    fn short_text_stays_put() {
        let pages = vec![BookPage::Text {
            text: "Hello".into(),
            title: Some("T".into()),
            font: None,
            font_size: None,
        }];
        let flowed = flow(book(pages.clone()));
        assert_eq!(flowed.entries[0].pages.len(), 1);
    }

    #[test]
    fn a_drawn_page_hands_its_box_and_data_to_the_mods_drawer() {
        let data = yog_data::json::from_json(&serde_json::json!({ "op_id": "hexcasting:manual" }));
        let page: BookPage = serde_json::from_value(serde_json::json!({
            "type": "drawn", "drawer": "hexcasting:pattern", "title": "Manual", "height": 64,
            "caption": "{u|entity} → {u|num}", "text": "Casts.", "data": { "op_id": "hexcasting:manual" }
        }))
        .unwrap();
        assert!(matches!(&page, BookPage::Drawn { data: d, height, .. } if *d == data && *height == 64.0));
        assert_eq!(content_above_text(&page), Some(14.0 + 68.0 + 14.0));
        let mut b = book(vec![page.clone()]);
        b.categories.push(crate::BookCategory { id: "c".into(), name: "C".into(), ..Default::default() });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        r.show("e", 0);
        r.prepare(800.0, 600.0);
        let drawn: Vec<_> = r.overlays.iter().filter_map(|o| match o {
            OverlayCmd::Drawn { drawer, data, w, h, .. } => Some((drawer.clone(), data.clone(), *w / *h)),
            _ => None,
        }).collect();
        assert_eq!(drawn.len(), 1);
        assert_eq!((drawn[0].0.as_str(), &drawn[0].1), ("hexcasting:pattern", &data));
        assert!((drawn[0].2 - PAGE_W / 64.0).abs() < 0.01, "the page's width, 64 high");
        // The JSON form keeps the data.
        assert_eq!(serde_json::to_value(&page).unwrap()["data"]["op_id"], "hexcasting:manual");
    }

    #[test]
    fn a_mods_page_type_lays_out_its_pages() {
        use crate::page_type::{PageElement as E, PageProperty, PageType, PropKind};
        let ty = PageType {
            id: "hexcasting:pattern".into(),
            properties: vec![
                PageProperty { name: "name".into(), kind: PropKind::BookText, default: None },
                PageProperty { name: "patterns".into(), kind: PropKind::Data, default: Some(yog_data::Value::List(vec![])) },
            ],
            body: vec![
                E::Header { text: "{x:Bind name}".into(), visible: None },
                E::Drawn { drawer: "hexcasting:pattern".into(), height: 64.0, data: "{x:Bind patterns}".into(), visible: Some("{x:Bind patterns}".into()) },
            ],
        };
        let page = |json: &str| BookPage::Custom(yog_data::read_value(&yog_data::json::from_json_text(json).unwrap()).unwrap());
        let mut b = book(vec![
            page(r#"{"type":"hexcasting:pattern","name":"Manual","patterns":[{"startdir":"EAST","signature":"qaq"}]}"#),
            page(r#"{"type":"hexcasting:pattern","name":"Bare"}"#),
        ]);
        b.page_types.push(ty);
        b.categories.push(crate::BookCategory { id: "c".into(), name: "C".into(), ..Default::default() });
        b.entries[0].category = "c".into();
        let mut r = BookRenderer::new(b);
        r.show("e", 0);
        r.prepare(800.0, 600.0);
        let drawn: Vec<_> = r.overlays.iter().filter_map(|o| match o {
            OverlayCmd::Drawn { drawer, data, .. } => Some((drawer.clone(), data.clone())),
            _ => None,
        }).collect();
        assert_eq!(drawn.len(), 1, "the second page has no patterns: no picture");
        assert_eq!(drawn[0].0, "hexcasting:pattern");
        assert!(matches!(&drawn[0].1, yog_data::Value::List(l) if l.len() == 1), "the field's whole value");
    }

    #[test]
    fn a_mods_page_flows_its_long_text_onto_text_pages() {
        use crate::page_type::{PageElement as E, PageProperty, PageType, PropKind};
        let ty = PageType {
            id: "m:p".into(),
            properties: vec![
                PageProperty { name: "patterns".into(), kind: PropKind::Data, default: None },
                PageProperty { name: "text".into(), kind: PropKind::BookText, default: None },
            ],
            body: vec![
                E::Drawn { drawer: "m:d".into(), height: 64.0, data: "{x:Bind patterns}".into(), visible: None },
                E::Text { text: "{x:Bind text}".into(), visible: None },
            ],
        };
        let long = "word ".repeat(120);
        let page = format!(r#"{{"type":"m:p","patterns":[1],"text":"{long}"}}"#);
        let mut b = book(vec![BookPage::Custom(yog_data::read_value(&yog_data::json::from_json_text(&page).unwrap()).unwrap())]);
        b.page_types.push(ty);
        let flowed = flow(b);
        let pages = &flowed.entries[0].pages;
        assert!(pages.len() >= 2, "{pages:?}");
        let BookPage::Custom(first) = &pages[0] else { panic!("{:?}", pages[0]) };
        let kept = first.fields.get("text").and_then(|v| v.as_str()).unwrap();
        let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
        assert!(kept.lines().count() as f32 * line_h <= BODY_H - 68.0 + 1.0, "under the picture: {kept}");
        let words: usize = kept.split_whitespace().count()
            + pages[1..].iter().map(|p| match p {
                BookPage::Text { text, .. } => text.split_whitespace().count(),
                _ => 0,
            }).sum::<usize>();
        assert_eq!(words, 120, "nothing lost");
    }

    #[test]
    fn a_translated_book_is_flowed_from_its_words_not_its_keys() {
        // A key too long for a page: flowed as it is, it'd be cut in pieces.
        fn tr(k: &str) -> Option<String> {
            (k.len() == 3000 && k.starts_with("mymod.")).then(|| "Short.".into())
        }
        let key = format!("mymod.{}", "x".repeat(2994));
        let mut b = book(vec![BookPage::Text {
            text: key,
            title: None,
            font: None,
            font_size: None,
        }]);
        b.i18n = true;
        let mut r = BookRenderer::new(b);
        r.set_translator(tr);
        r.localize();
        let pages = &r.book().entries[0].pages;
        assert_eq!(pages.len(), 1, "{pages:?}");
        assert!(matches!(&pages[0], BookPage::Text { text, .. } if text == "Short."), "{pages:?}");
    }
}
