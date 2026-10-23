//! The book's pieces of UI: its labels and buttons (the game font,
//! shadowless as Patchouli draws them, at the book's scale), titles that fit
//! their page, the navigation row — and the canvas a page is built on.

use std::cell::Cell;

use yog_gfx::{Rect, Vec2};
use yog_ui::{Align, FlexDir, widget};

use super::geometry::{
    ARROW_BACK_V, ARROW_H, ARROW_NEXT_V, ARROW_U, ARROW_W, HOME_H, HOME_U, HOME_V, NAV_H, PAGE_W,
    SEP_H, SEP_U, SEP_V, SEP_W, TITLE_INSET,
};
use super::overlay::{BgSprite, OverlayCmd, SpriteSheet};
use crate::look::theme::BookTheme;

thread_local! {
    /// The scale the book is drawn at (≤ 1): its text is drawn at it too,
    /// so it stays on the page however small the book gets.
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.0) };
}

pub(crate) fn set_text_scale(scale: f32) {
    TEXT_SCALE.with(|s| s.set(scale));
}

pub(crate) fn text_scale() -> f32 {
    TEXT_SCALE.with(|s| s.get())
}

/// A label.
pub(crate) fn lbl(text: impl Into<String>) -> widget::Widget {
    widget::label(text).shadow(false).font_scale(text_scale())
}

/// A title: the page's inside wide, so `align` can centre it; at the
/// text's size, a long one wrapping onto more lines.
pub(crate) fn title_lbl(text: impl Into<String>) -> widget::Widget {
    widget::label(text)
        .shadow(false)
        .font_scale(text_scale())
        .w((PAGE_W - TITLE_INSET) * text_scale())
}

/// A page's header title: one line, a long one drawn smaller till it fits
/// the page (as Patchouli does).
pub(crate) fn header_lbl(text: impl Into<String>) -> widget::Widget {
    let text = text.into();
    let room = (PAGE_W - TITLE_INSET) * text_scale();
    let wide = yog_ui::text::str_width_in(&text, text_scale(), "");
    let scale = match wide > room {
        true => text_scale() * room / wide,
        false => text_scale(),
    };
    title_lbl(text).font_scale(scale).no_wrap()
}

/// A row of pages' navigation: back and next (their actions), `label`
/// between them, and a home button when there is `home`.
pub(crate) struct Nav<'a> {
    pub prev: &'a str,
    pub next: &'a str,
    pub label: String,
    pub home: Option<&'a str>,
}

impl Nav<'_> {
    /// The row, its arrows drawn from the book's sheet `sheet`.
    pub(crate) fn row(self, sheet: &str, theme: &BookTheme, sx: f32, sy: f32) -> widget::Widget {
        let arrow = |region: Rect, action: &str| {
            widget::image(sheet)
                .region(region)
                .stretch(yog_ui::Stretch::Fill)
                .w(region.w * sx)
                .h(region.h * sy)
                .on_click(action)
                .id(action)
        };
        let back = arrow(
            Rect::new(ARROW_U, ARROW_BACK_V, ARROW_W, ARROW_H),
            self.prev,
        );
        let mut row = widget::panel(FlexDir::Row)
            .h(NAV_H * sy)
            .gap(NAV_GAP * sx)
            .align(Align::Center)
            .child(back);
        row = row.child(
            lbl(&self.label)
                .color(theme.nav)
                .flex(1.0)
                .align(Align::Center),
        );
        if let Some(home) = self.home {
            row = row.child(arrow(Rect::new(HOME_U, HOME_V, ARROW_W, HOME_H), home));
        }
        row.child(arrow(
            Rect::new(ARROW_U, ARROW_NEXT_V, ARROW_W, ARROW_H),
            self.next,
        ))
    }
}

/// The navigation's spacing.
const NAV_GAP: f32 = 2.0;

/// What a page is built onto: the book's scale and theme, and the sprites
/// and overlays drawn around its widgets.
pub(crate) struct Canvas<'a> {
    pub theme: &'a BookTheme,
    /// The book's sheet (its own, or Yog's) as an image source.
    pub sheet: &'a str,
    /// Book pixels to screen pixels, across and down.
    pub sx: f32,
    pub sy: f32,
    pub sprites: &'a mut Vec<BgSprite>,
    pub overlays: &'a mut Vec<OverlayCmd>,
}

impl Canvas<'_> {
    /// A page's navigation row, drawn from the book's sheet.
    pub(crate) fn nav(&self, nav: Nav<'_>) -> widget::Widget {
        nav.row(self.sheet, self.theme, self.sx, self.sy)
    }

    /// `r` (book pixels from `origin`, a point on screen) on screen.
    pub(crate) fn rect(&self, origin: Vec2, r: Rect) -> Rect {
        Rect::new(
            origin.x + r.x * self.sx,
            origin.y + r.y * self.sy,
            r.w * self.sx,
            r.h * self.sy,
        )
    }

    /// A separator's height on screen (a pixel at least).
    pub(crate) fn sep_h(&self) -> f32 {
        (SEP_H * self.sy).max(1.0)
    }

    /// The separator across a page whose top-left is `page` (on screen), `y` down it.
    pub(crate) fn separator(&mut self, page: Vec2, y: f32) {
        let x = page.x + (PAGE_W - SEP_W) / 2.0 * self.sx;
        let dst = Rect::new(x, page.y + y * self.sy, SEP_W * self.sx, self.sep_h());
        self.sprites.push(BgSprite {
            sheet: SpriteSheet::Book,
            src: Rect::new(SEP_U, SEP_V, SEP_W, SEP_H),
            dst,
        });
    }
}
