//! Book renderer: yog-ui lays the book out and draws its text and
//! buttons; everything else goes through `draw2d` too — the sprite
//! sheets, item icons, SVG pages, entities — so the game and Yog-IDLE's
//! book designer run the same code.

mod flow;
pub(crate) mod geometry;
mod names;
mod overlay;
mod page;
mod recipe;
mod spread;
mod widgets;

use std::collections::HashMap;
use std::time::Instant;

use yog_formats::book::Book;
use yog_gfx::{Color, GfxContext, Rect};
use yog_ui::UiRoot;

pub use geometry::{BLOCK_PITCH, SLOT_YAW};
pub use overlay::PageDrawer;
/// The image sources the book draws [`BOOK_PNG`] and [`CRAFTING_PNG`] as:
/// the loader ships them under `assets/yog/textures/gui/book/` (Yog-IDLE
/// resolves them to the bytes).
pub use yog_ids::yog::{BOOK_SHEET, CRAFTING_SHEET};

use crate::state::BookViewState;
use crate::theme::BookTheme;
use flow::{flow_traced, styled};
use geometry::{BK_H, BK_W, MIN_SCALE, SCREEN_FILL, SCREEN_MARGIN};
use overlay::{BgSprite, OverlayCmd};
use recipe::{RecipeVis, parse_recipe};
use spread::Spread;

/// The book's sprite sheet (cover, pages, separators, nameplate, arrows).
pub static BOOK_PNG: &[u8] = include_bytes!("../../assets/book_brown.png");
/// The crafting-grid sprite sheet.
pub static CRAFTING_PNG: &[u8] = include_bytes!("../../assets/crafting.png");
/// The book's own words (`yog.book.*`) per language, as the loader's jars
/// ship them in `assets/yog/lang/` (for hosts without them, like Yog-IDLE).
pub static LANG: &[(&str, &str)] = &[
    ("en_us", include_str!("../../assets/lang/en_us.json")),
    ("ru_ru", include_str!("../../assets/lang/ru_ru.json")),
];

/// A book being read: what it shows, laid out for the screen it was last drawn on.
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
    start: Instant,
    pub ui: Option<UiRoot>,
    bg_sprites: Vec<BgSprite>,
    overlays: Vec<OverlayCmd>,
    dirty: bool,
    /// The screen it was laid out for.
    last_screen: (f32, f32),
    /// i18n books: the untranslated book, the translator and the language
    /// probe (the book name's text) the shown book was made for.
    source: Option<Book>,
    translator: Option<names::Translator>,
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
        Self {
            book: flowed,
            origins,
            written,
            measured: false,
            start: Instant::now(),
            state: BookViewState::default(),
            theme,
            recipes: HashMap::new(),
            ui: None,
            bg_sprites: Vec::new(),
            overlays: Vec::new(),
            dirty: true,
            last_screen: (0.0, 0.0),
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
        names::set_translator(tr);
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
    /// the book's rect: about [`SCREEN_FILL`] of the screen whatever the
    /// GUI scale, at whole multiples of its size (272×180 GUI pixels) so
    /// the game's font stays sharp; smaller only on a screen it doesn't fit.
    fn prepare(&mut self, sw: f32, sh: f32) -> Rect {
        let room = ((sw - SCREEN_MARGIN) / BK_W).min((sh - SCREEN_MARGIN) / BK_H);
        let wanted = (sh * SCREEN_FILL / BK_H).round().max(1.0);
        let scale = if room >= 1.0 {
            wanted.min(room.floor())
        } else {
            room.max(MIN_SCALE)
        };
        let (bw, bh) = (BK_W * scale, BK_H * scale);
        let book_rect = Rect::new((sw - bw) / 2.0, (sh - bh) / 2.0, bw, bh);
        self.localize();
        if self.dirty || (sw, sh) != self.last_screen {
            let sheet = names::book_sheet(&self.book.book_texture);
            let spread = Spread {
                book: &self.book,
                state: &self.state,
                theme: &self.theme,
                recipes: &self.recipes,
                sheet: &sheet,
            };
            let (root, bg_sprites, overlays) = spread.build(book_rect);
            (self.ui, self.bg_sprites, self.overlays) = (Some(root), bg_sprites, overlays);
            self.dirty = false;
            self.last_screen = (sw, sh);
        }
        if let Some(ui) = &mut self.ui
            && ui.needs_layout
        {
            ui.layout(sw, sh);
        }
        book_rect
    }

    /// Draws the book into a `sw`×`sh` screen — all of it through `draw2d`
    /// (its sprite sheets as the image sources [`BOOK_SHEET`] and
    /// [`CRAFTING_SHEET`]); mods' pictures (`BookPage::Drawn`) are left
    /// empty — see [`BookRenderer::render_with`].
    pub fn render(&mut self, ctx: &GfxContext, sw: f32, sh: f32) {
        self.render_with(ctx, sw, sh, &mut |_, _, _, _| {});
    }

    /// Draws the book, mods' pictures with `draw(ctx, drawer, data, [x, y,
    /// w, h])` (screen pixels) — the game calls the mod that registered `drawer`.
    pub fn render_with(&mut self, ctx: &GfxContext, sw: f32, sh: f32, draw: PageDrawer<'_>) {
        // Text flows by the host's own font metrics once it can measure.
        yog_ui::text::use_host_metrics(ctx);
        if !self.measured {
            self.measured = true;
            self.reflow();
        }
        let book_rect = self.prepare(sw, sh);
        let sheet = names::book_sheet(&self.book.book_texture);
        ctx.draw2d().image_region(
            &sheet,
            book_rect,
            Some(Rect::new(0.0, 0.0, BK_W, BK_H)),
            Color::WHITE,
        );
        overlay::draw_sprites(ctx, &self.bg_sprites, &sheet);
        if let Some(ui) = &mut self.ui {
            ui.render(ctx);
        }
        // Entities and structures turn (Patchouli's pace), an ingredient's
        // alternatives take turns every second; items last (they change GL state).
        let elapsed = self.start.elapsed();
        overlay::draw_scenes(ctx, &self.overlays, elapsed.as_secs_f32(), draw);
        overlay::draw_items(ctx, &self.overlays, elapsed.as_secs() as usize);
    }

    /// The page shown moves by itself (an entity, a structure, an
    /// ingredient's alternatives): the host should keep redrawing it.
    pub fn animated(&self) -> bool {
        self.overlays.iter().any(OverlayCmd::animated)
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
        let Some(i) = self
            .state
            .entries_in_cat(&self.book)
            .iter()
            .position(|x| x.id == entry)
        else {
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

#[cfg(test)]
mod tests;
