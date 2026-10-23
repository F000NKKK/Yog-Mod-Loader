//! The open book, laid out: its two pages — the landing and the category
//! grid at home, else an entry's page and its category's list — with the
//! sprites and overlays around them.

use std::collections::HashMap;

use yog_formats::book::Book;
use yog_formats::book::layout::ICON_PX;
use yog_gfx::{Color, Rect, Vec2};
use yog_ui::{Align, FlexDir, UiRoot, widget};

use super::geometry::{
    BK_H, BK_W, BODY_Y, BOOK_NAME_X, BOOK_NAME_Y, CAT_CELL, CAT_COLUMNS, CAT_GRID_X, CAT_GRID_Y,
    CAT_ICON_INSET, COUNT_H, COUNT_SLACK, EDITION_SCALE, EDITION_X, EDITION_Y, GAP,
    LANDING_FIT_STEP, LANDING_MIN_FIT, LANDING_Y, LEFT_X, LIST_ICON, LIST_ICON_INSET, LIST_NAME_W,
    LIST_PAD_BOTTOM, LIST_ROW_H, LIST_Y, NAMEPLATE_H, NAMEPLATE_V, NAMEPLATE_W, NAMEPLATE_X,
    NAMEPLATE_Y, PAGE_H, PAGE_W, RIGHT_X, SEP_H, SEP_Y, TITLE_H, TOP_PAD,
};
use super::names::{ordinal, word};
use super::overlay::{BgSprite, OverlayCmd, SpriteSheet};
use super::recipe::RecipeVis;
use super::widgets::{Canvas, Nav, header_lbl, lbl, set_text_scale, text_scale, title_lbl};
use crate::state::{BookViewState, action};
use crate::theme::BookTheme;

/// What the open book shows, and what it's drawn with.
pub(crate) struct Spread<'a> {
    pub book: &'a Book,
    pub state: &'a BookViewState,
    pub theme: &'a BookTheme,
    pub recipes: &'a HashMap<String, RecipeVis>,
    /// The book's sheet as an image source.
    pub sheet: &'a str,
}

/// The widgets, the sprites drawn under them and the overlays drawn over them.
pub(crate) type Built = (UiRoot, Vec<BgSprite>, Vec<OverlayCmd>);

impl Spread<'_> {
    /// The book laid out in `book_rect` (on screen).
    pub(crate) fn build(&self, book_rect: Rect) -> Built {
        let (mut sprites, mut overlays) = (Vec::new(), Vec::new());
        let (sx, sy) = (book_rect.w / BK_W, book_rect.h / BK_H);
        set_text_scale(sx);
        let mut c = Canvas {
            theme: self.theme,
            sheet: self.sheet,
            sx,
            sy,
            sprites: &mut sprites,
            overlays: &mut overlays,
        };
        let page_at = |x: f32| Vec2::new(book_rect.x + x * sx, book_rect.y + TOP_PAD * sy);
        let (left, right) = (page_at(LEFT_X), page_at(RIGHT_X));
        // The right page's header separator: under "Categories" or the category's name.
        c.separator(right, SEP_Y);
        let (left_page, right_page) = if self.state.at_home {
            c.sprites.push(BgSprite {
                sheet: SpriteSheet::Book,
                src: Rect::new(0.0, NAMEPLATE_V, NAMEPLATE_W, NAMEPLATE_H),
                dst: c.rect(
                    Vec2::new(book_rect.x, book_rect.y),
                    Rect::new(NAMEPLATE_X, NAMEPLATE_Y, NAMEPLATE_W, NAMEPLATE_H),
                ),
            });
            (
                self.landing(&mut c, book_rect),
                self.categories(&mut c, right),
            )
        } else {
            (self.entry(&mut c, left), self.entries(&mut c, right))
        };
        // The pages sit exactly at LEFT_X / RIGHT_X (no flex gap): the
        // overlays are placed at those positions.
        let spine_gap = (RIGHT_X - LEFT_X - PAGE_W) * sx;
        let pages = widget::panel(FlexDir::Row)
            .w(book_rect.w)
            .h(book_rect.h)
            .gap(0.0)
            .padding(TOP_PAD * sy, 0.0, 0.0, LEFT_X * sx)
            .child(left_page)
            .child(widget::spacer().w(spine_gap))
            .child(right_page);
        // Layout starts at (0, 0): the book is put in place by a screen-wide panel's padding.
        let screen = widget::panel(FlexDir::Row)
            .padding(book_rect.y, 0.0, 0.0, book_rect.x)
            .child(pages);
        (UiRoot::new(&self.book.id, screen), sprites, overlays)
    }

    fn page(&self, c: &Canvas<'_>) -> widget::Widget {
        widget::panel(FlexDir::Column)
            .w(PAGE_W * c.sx)
            .h(PAGE_H * c.sy)
            .gap(0.0)
    }

    /// The landing page: the book's name and edition on the nameplate, its
    /// landing text (as small as it takes), how many entries it has.
    fn landing(&self, c: &mut Canvas<'_>, book_rect: Rect) -> widget::Widget {
        let book = self.book;
        let on_book = |x: f32, y: f32| Vec2::new(book_rect.x + x * c.sx, book_rect.y + y * c.sy);
        c.overlays.push(OverlayCmd::McText {
            text: book.name.clone(),
            at: on_book(BOOK_NAME_X, BOOK_NAME_Y),
            color: c.theme.nameplate,
            scale: c.sx,
        });
        // Its edition ("1st Edition"), else its subtitle — as Patchouli.
        let second = match book.version {
            0 => book.subtitle.clone(),
            n => Some(word("edition", &ordinal(n))),
        };
        if let Some(text) = second {
            c.overlays.push(OverlayCmd::McText {
                text,
                at: on_book(EDITION_X, EDITION_Y),
                color: c.theme.nameplate,
                scale: c.sx * EDITION_SCALE,
            });
        }
        let mut col = self
            .page(c)
            .padding(LANDING_Y * c.sy, PAGE_PAD_RIGHT, GAP, GAP);
        let fit = landing_fit(&book.landing_text, c.sx, c.sy);
        for para in book.landing_text.split('\n') {
            col = col.child(match para.is_empty() {
                true => widget::spacer().h(GAP * c.sy),
                false => lbl(para).color(c.theme.text).font_scale(text_scale() * fit),
            });
        }
        let count = title_lbl(word("entry_count", &book.entries.len().to_string()))
            .color(c.theme.nav)
            .h(COUNT_H * c.sy)
            .align(Align::Center);
        col.child(widget::spacer().flex(1.0)).child(count)
    }

    /// The category grid: each category's icon in a cell, a click opens it.
    fn categories(&self, c: &mut Canvas<'_>, page: Vec2) -> widget::Widget {
        let header_h = TITLE_H * c.sy;
        let mut col = self.page(c).padding(0.0, GAP, GAP, 0.0);
        col = col.child(
            header_lbl(word("categories", ""))
                .color(c.theme.divider)
                .h(header_h)
                .align(Align::Center),
        );
        col = col.child(widget::spacer().h(CAT_GRID_Y * c.sy - header_h));
        let icon = ICON_PX * c.sx.min(c.sy);
        for (row_i, row_cats) in self.book.categories.chunks(CAT_COLUMNS).enumerate() {
            let mut row = widget::panel(FlexDir::Row)
                .h(CAT_CELL * c.sy)
                .gap(0.0)
                .padding(0.0, 0.0, 0.0, CAT_GRID_X * c.sx);
            for (col_i, cat) in row_cats.iter().enumerate() {
                let idx = row_i * CAT_COLUMNS + col_i;
                if let Some(icon_id) = &cat.icon {
                    let cell = Vec2::new(
                        CAT_GRID_X + col_i as f32 * CAT_CELL + CAT_ICON_INSET,
                        CAT_GRID_Y + row_i as f32 * CAT_CELL + CAT_ICON_INSET,
                    );
                    let at = c.rect(page, Rect::new(cell.x, cell.y, 0.0, 0.0));
                    c.overlays
                        .push(OverlayCmd::icon(icon_id, Rect::new(at.x, at.y, icon, icon)));
                }
                let selected = !self.state.at_home && idx == self.state.cat;
                let bg = if selected {
                    c.theme.nav_selected_bg
                } else {
                    Color::TRANSPARENT
                };
                let cell = widget::panel(FlexDir::Column)
                    .w(CAT_CELL * c.sx)
                    .h(CAT_CELL * c.sy)
                    .bg(bg)
                    .on_click(action::category(idx))
                    .id(format!("book_cat_{idx}"));
                row = row.child(cell);
            }
            col = col.child(row);
        }
        col
    }

    /// The entry's page shown: its title over a separator, the page, the navigation.
    fn entry(&self, c: &mut Canvas<'_>, page: Vec2) -> widget::Widget {
        let entry = self.state.current_entry(self.book);
        let shown = entry.and_then(|e| e.pages.get(self.state.page));
        c.separator(page, SEP_Y);
        let nav = c.nav(Nav {
            prev: action::PREV_PAGE,
            next: action::NEXT_PAGE,
            label: format!(
                "{}/{}",
                self.state.page + 1,
                self.state.page_count(self.book)
            ),
            home: Some(action::HOME),
        });
        let body = Vec2::new(page.x, page.y + BODY_Y * c.sy);
        let content = super::page::build(
            c,
            &super::page::Context {
                recipes: self.recipes,
                book: self.book,
                number: self.state.page,
                at: body,
            },
            shown,
        );
        let title = entry.map(|e| e.name.as_str()).unwrap_or("");
        let sep_h = c.sep_h();
        self.page(c)
            .padding(0.0, PAGE_PAD_RIGHT, GAP, GAP)
            .child(
                header_lbl(title)
                    .color(c.theme.title)
                    .h(TITLE_H * c.sy)
                    .align(Align::Center),
            )
            .child(widget::spacer().h((SEP_Y - TITLE_H) * c.sy))
            .child(widget::spacer().h(sep_h))
            .child(widget::spacer().h((BODY_Y - SEP_Y - SEP_H) * c.sy))
            .child(content)
            .child(nav)
    }

    /// The category's entries (a spread of them), the selected one marked.
    fn entries(&self, c: &mut Canvas<'_>, page: Vec2) -> widget::Widget {
        let book = self.book;
        let cat_name = book
            .categories
            .get(self.state.cat)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| word("entries", ""));
        let icon = LIST_ICON * c.sx.min(c.sy);
        let mut col = self.page(c).padding(0.0, GAP, LIST_PAD_BOTTOM, 0.0);
        col = col.child(
            title_lbl(cat_name)
                .color(c.theme.divider)
                .h(TITLE_H * c.sy)
                .align(Align::Center),
        );
        col = col.child(widget::spacer().h((SEP_Y - TITLE_H) * c.sy));
        col = col.child(widget::spacer().h(c.sep_h()));
        col = col.child(widget::spacer().h((LIST_Y - SEP_Y - SEP_H) * c.sy));
        let start = self.state.list_spread_start(book);
        // Rows above this entry's: a long name takes more than one.
        let mut row_at = 0;
        for (i, entry) in self.state.entries_visible(book).iter().enumerate() {
            let rows = crate::state::list_rows(&entry.name);
            let abs_i = start + i;
            let selected = abs_i == self.state.entry;
            if let Some(icon_id) = &entry.icon {
                let at = c.rect(
                    page,
                    Rect::new(
                        LIST_ICON_INSET,
                        LIST_Y + row_at as f32 * LIST_ROW_H + LIST_ICON_INSET,
                        0.0,
                        0.0,
                    ),
                );
                c.overlays
                    .push(OverlayCmd::icon(icon_id, Rect::new(at.x, at.y, icon, icon)));
            }
            let (bg, color) = if selected {
                (c.theme.nav_selected_bg, c.theme.nav_selected)
            } else {
                (Color::TRANSPARENT, c.theme.nav)
            };
            // Every name at the text's size: a long one wraps onto more rows.
            let name = lbl(&entry.name).color(color).w(LIST_NAME_W * text_scale());
            row_at += rows;
            let row = widget::panel(FlexDir::Row)
                .h(rows as f32 * LIST_ROW_H * c.sy)
                .gap(LIST_GAP)
                .bg(bg)
                .on_click(action::entry(abs_i))
                .id(format!("book_entry_{abs_i}"))
                .padding(LIST_ICON_INSET, LIST_GAP, LIST_ICON_INSET, LIST_GAP)
                .child(widget::spacer().w(icon + LIST_GAP))
                .child(name);
            col = col.child(row);
        }
        let spreads = self.state.list_spread_count(book);
        if spreads > 1 {
            let label = format!("{}/{}", self.state.list_spread + 1, spreads);
            col = col.child(widget::spacer().flex(1.0)).child(c.nav(Nav {
                prev: action::PREV_LIST,
                next: action::NEXT_LIST,
                label,
                home: None,
            }));
        }
        col
    }
}

/// A page's padding on the right (the spine's side), and between a list's icon and name.
const PAGE_PAD_RIGHT: f32 = 6.0;
const LIST_GAP: f32 = 2.0;

/// The scale (≤ 1) the landing text fits its page at, above the entry
/// count: the page's own layout measured (on screen, the book `sx`×`sy`
/// its size), as the label wraps it.
pub(super) fn landing_fit(text: &str, sx: f32, sy: f32) -> f32 {
    use yog_ui::text::text_height_in;
    let width = PAGE_W * sx - PAGE_PAD_RIGHT - GAP;
    let room = (PAGE_H - LANDING_Y - COUNT_H - COUNT_SLACK) * sy - GAP;
    let height = |fit: f32| -> f32 {
        text.split('\n')
            .map(|p| match p.is_empty() {
                true => GAP * sy,
                false => text_height_in(p, width, sx * fit, ""),
            })
            .sum()
    };
    let mut fit = 1.0;
    while fit > LANDING_MIN_FIT && height(fit) > room {
        fit -= LANDING_FIT_STEP;
    }
    fit
}
