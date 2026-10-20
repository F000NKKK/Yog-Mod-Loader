//! A page's content, by its kind: text, a spotlight, a recipe in its
//! layout, an image, an entity, a multiblock, a mod's picture, a mod's own
//! kind of page. Positions are in book pixels from the body's top-left.

use std::collections::HashMap;

use yog_formats::book::layout::{ICON_PX, LayoutPart, RecipeLayout};
use yog_formats::book::page_type::{CustomPage, PageElement, PageType};
use yog_formats::book::{Book, BookPage};
use yog_gfx::{Rect, Vec2};
use yog_ui::{Align, FlexDir, Stretch, widget};

use super::CRAFTING_SHEET;
use super::geometry::{
    BODY_Y, ENTITY_ROOM, ENTITY_Y, GAP, ITEM_ROOM, MB_NAME_H, MB_VIEW, PAGE_W, PICTURE,
    PICTURE_TITLE_H, PICTURE_TITLE_ROOM, RECIPE_ROOM, SEP_H, SEP_Y, SPOT_FRAME_H, SPOT_FRAME_V,
    SPOT_FRAME_W, SPOT_FRAME_Y, SPOT_ITEM_Y, SPOT_TEXT_Y, TEXT_W, TITLE_H,
};
use super::names::{entity_name, pretty_item_name, word};
use super::overlay::{BgSprite, OverlayCmd, SpriteSheet};
use super::recipe::RecipeVis;
use super::widgets::{Canvas, lbl, text_scale, title_lbl};

/// What a page is built with besides its canvas.
pub(crate) struct Context<'a> {
    pub recipes: &'a HashMap<String, RecipeVis>,
    pub book: &'a Book,
    /// The page's number in its entry (a later text page shows its title).
    pub number: usize,
    /// The body's top-left on screen.
    pub at: Vec2,
}

/// A text under the content, if the page has one.
fn with_text(col: widget::Widget, c: &Canvas<'_>, text: &Option<String>) -> widget::Widget {
    match text {
        Some(t) => col.child(lbl(t.as_str()).color(c.theme.text)),
        None => col,
    }
}

/// `page`'s content (a note when there is none).
pub(crate) fn build(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    page: Option<&BookPage>,
) -> widget::Widget {
    let col = widget::panel(FlexDir::Column).flex(1.0).gap(GAP);
    let Some(page) = page else {
        return col.child(lbl(word("no_entries", "")).color(c.theme.nav));
    };
    match page {
        BookPage::Text { .. } => text_page(c, cx, col, page),
        BookPage::Spotlight { .. } => spotlight(c, cx, col, page),
        BookPage::Recipe { .. } => recipe(c, cx, col, page),
        BookPage::Image { .. } => image(c, col, page),
        BookPage::Relations { .. } => relations(c, col, page),
        BookPage::Multiblock { .. } => multiblock(c, cx, col, page),
        BookPage::Entity { .. } => entity(c, cx, col, page),
        BookPage::Drawn { .. } => drawn(c, cx, col, page),
        BookPage::Custom(custom_page) => {
            match cx.book.page_types.iter().find(|t| t.id == custom_page.kind) {
                Some(kind) => custom(c, cx.at, col, kind, custom_page),
                None => col.child(lbl(format!("[{}]", custom_page.kind)).color(c.theme.nav)),
            }
        }
        BookPage::Empty => col,
    }
}

/// A picture's title over it.
fn picture_title(c: &Canvas<'_>, col: widget::Widget, title: &str) -> widget::Widget {
    col.child(
        title_lbl(title)
            .color(c.theme.title)
            .h(PICTURE_TITLE_H * c.sy),
    )
}

fn recipe(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Recipe { recipe, text } = page else {
        return col;
    };
    let found = cx.recipes.get(recipe.as_str()).and_then(|vis| {
        yog_formats::book::layout::find(&cx.book.layouts, &vis.kind).map(|l| (vis, l))
    });
    let col = match found {
        Some((vis, layout)) => recipe_layout(c, cx.at, col, vis, layout),
        None => col.child(lbl(format!("[{recipe}]")).color(c.theme.nav)),
    };
    with_text(col, c, text)
}

fn image(c: &Canvas<'_>, col: widget::Widget, page: &BookPage) -> widget::Widget {
    let BookPage::Image {
        texture,
        title,
        text,
        ..
    } = page
    else {
        return col;
    };
    let col = match title {
        Some(t) => col.child(title_lbl(t.as_str()).color(c.theme.title)),
        None => col,
    };
    let picture = widget::image(texture)
        .region(Rect::new(0.0, 0.0, PICTURE, PICTURE))
        .stretch(Stretch::Fill)
        .w(PICTURE)
        .h(PICTURE);
    with_text(col.child(picture), c, text)
}

fn relations(c: &Canvas<'_>, col: widget::Widget, page: &BookPage) -> widget::Widget {
    let BookPage::Relations { entries, text } = page else {
        return col;
    };
    let mut col =
        with_text(col, c, text).child(title_lbl(word("see_also", "")).color(c.theme.title));
    for e in entries {
        col = col.child(lbl(format!("{BULLET} {e}")).color(c.theme.nav));
    }
    col
}

/// A multiblock: its structure turning under its name.
fn multiblock(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Multiblock {
        name,
        pattern,
        mapping,
        text,
    } = page
    else {
        return col;
    };
    let (col, top) = match name {
        Some(n) => (picture_title(c, col, n), MB_NAME_H),
        None => (col, 0.0),
    };
    let at = c.rect(
        cx.at,
        Rect::new((PAGE_W - MB_VIEW) / 2.0, top, MB_VIEW, MB_VIEW),
    );
    c.overlays.push(OverlayCmd::Blocks {
        blocks: yog_formats::book::multiblock_blocks(pattern, mapping),
        at,
        turning: true,
    });
    with_text(col.child(widget::spacer().h(MB_VIEW * c.sy)), c, text)
}

/// An entity standing under its name.
fn entity(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Entity { entity, name, text } = page else {
        return col;
    };
    let display = name.clone().unwrap_or_else(|| entity_name(entity));
    let at = c.rect(
        cx.at,
        Rect::new((PAGE_W - PICTURE) / 2.0, ENTITY_Y, PICTURE, PICTURE),
    );
    c.overlays.push(OverlayCmd::Entity {
        entity: entity.clone(),
        at,
    });
    let col = col
        .child(title_lbl(display).color(c.theme.title))
        .child(widget::spacer().h(ENTITY_ROOM * c.sy));
    with_text(col, c, text)
}

/// A mod's picture: a box the page's width, drawn by the mod's code.
fn drawn(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Drawn {
        drawer,
        title,
        height,
        caption,
        text,
        data,
    } = page
    else {
        return col;
    };
    let (mut col, top) = match title {
        Some(t) => (picture_title(c, col, t), PICTURE_TITLE_ROOM),
        None => (col, 0.0),
    };
    if *height > 0.0 {
        let at = c.rect(cx.at, Rect::new(0.0, top, PAGE_W, *height));
        c.overlays.push(OverlayCmd::Drawn {
            drawer: drawer.clone(),
            data: data.clone(),
            at,
        });
        col = col.child(widget::spacer().h(height * c.sy));
    }
    with_text(with_text(col, c, caption), c, text)
}

/// What a relations page's entries are listed with.
const BULLET: &str = "•";

/// A text page: a later page's own title over a separator, then its text
/// (in its own font, `font_size` pixels a line, if it has one).
fn text_page(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    mut col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Text {
        text,
        title,
        font,
        font_size,
    } = page
    else {
        return col;
    };
    if cx.number > 0
        && let Some(t) = title
    {
        col = col.child(
            title_lbl(t.as_str())
                .color(c.theme.title)
                .h(TITLE_H * c.sy)
                .align(Align::Center)
                .no_wrap(),
        );
        c.separator(cx.at, SEP_Y);
        let sep_h = c.sep_h();
        col = col
            .child(widget::spacer().h((SEP_Y - TITLE_H) * c.sy))
            .child(widget::spacer().h(sep_h))
            .child(widget::spacer().h((BODY_Y - SEP_Y - SEP_H) * c.sy));
    }
    // One label: its lines are as far apart as the flowing counted.
    let mut l = lbl(text.as_str()).color(c.theme.text);
    if let Some(f) = font {
        let line = yog_ui::text::LINE_H;
        l = l
            .font(f.as_str())
            .font_scale(text_scale() * font_size.unwrap_or(line) / line);
    }
    col.child(l)
}

/// A spotlight: the item framed under its name, its text under that.
fn spotlight(
    c: &mut Canvas<'_>,
    cx: &Context<'_>,
    col: widget::Widget,
    page: &BookPage,
) -> widget::Widget {
    let BookPage::Spotlight { item, title, text } = page else {
        return col;
    };
    let frame = Rect::new(
        (PAGE_W - SPOT_FRAME_W) / 2.0,
        SPOT_FRAME_Y,
        SPOT_FRAME_W,
        SPOT_FRAME_H,
    );
    let dst = c.rect(cx.at, frame);
    c.sprites.push(BgSprite {
        sheet: SpriteSheet::Crafting,
        src: Rect::new(0.0, SPOT_FRAME_V, SPOT_FRAME_W, SPOT_FRAME_H),
        dst,
    });
    let icon = ICON_PX * c.sx.min(c.sy);
    let at = c.rect(
        cx.at,
        Rect::new((PAGE_W - ICON_PX) / 2.0, SPOT_ITEM_Y, 0.0, 0.0),
    );
    c.overlays.push(OverlayCmd::McItem {
        item_id: item.to_string(),
        at: Rect::new(at.x, at.y, icon, icon),
    });
    let name = title.clone().unwrap_or_else(|| pretty_item_name(item));
    let col = col
        .child(
            title_lbl(name)
                .color(c.theme.title)
                .h(PICTURE_TITLE_H * c.sy)
                .align(Align::Center),
        )
        .child(widget::spacer().h((SPOT_TEXT_Y - PICTURE_TITLE_H) * c.sy));
    with_text(col, c, text)
}

/// `vis` drawn the way `layout` lays it out, under its result's name,
/// centred on the page.
fn recipe_layout(
    c: &mut Canvas<'_>,
    body: Vec2,
    col: widget::Widget,
    vis: &RecipeVis,
    layout: &RecipeLayout,
) -> widget::Widget {
    let col = col.child(
        title_lbl(pretty_item_name(&vis.output))
            .color(c.theme.title)
            .h(PICTURE_TITLE_H * c.sy)
            .align(Align::Center),
    );
    let place = Place {
        origin: body,
        at: Vec2::new((PAGE_W - layout.size[0]) / 2.0 - 1.0, PICTURE_TITLE_H),
        icon: ICON_PX * c.sx.min(c.sy),
    };
    layout_picture(c, &place, layout);
    recipe_parts(c, &place, vis, layout);
    col.child(widget::spacer().h((layout.size[1] + RECIPE_ROOM) * c.sy))
}

/// Where a recipe's layout sits: the body on screen, the layout's corner
/// in it (book pixels), an icon's size on screen.
struct Place {
    origin: Vec2,
    at: Vec2,
    icon: f32,
}

impl Place {
    /// `p` (layout pixels), `w`×`h` book pixels, on screen.
    fn rect(&self, c: &Canvas<'_>, p: [f32; 2], w: f32, h: f32) -> Rect {
        c.rect(
            self.origin,
            Rect::new(self.at.x + p[0], self.at.y + p[1], w, h),
        )
    }

    /// An icon at `p`.
    fn icon(&self, c: &Canvas<'_>, p: [f32; 2]) -> Rect {
        let r = self.rect(c, p, 0.0, 0.0);
        Rect::new(r.x, r.y, self.icon, self.icon)
    }
}

/// The layout's own picture: its frames, items and blocks.
fn layout_picture(c: &mut Canvas<'_>, place: &Place, layout: &RecipeLayout) {
    use crate::layout_picture::Piece;
    for piece in crate::layout_picture::pieces(layout) {
        match piece {
            Piece::Image { source, uv, at: r } => {
                let sheet = if source == CRAFTING_SHEET {
                    SpriteSheet::Crafting
                } else {
                    SpriteSheet::Image(source.to_string())
                };
                let dst = place.rect(c, [r.x, r.y], r.w, r.h);
                c.sprites.push(BgSprite {
                    sheet,
                    src: uv,
                    dst,
                });
            }
            Piece::Item { item, at: r } => {
                let at = place.icon(c, [r.x, r.y]);
                c.overlays.push(OverlayCmd::McItem {
                    item_id: item.to_string(),
                    at,
                });
            }
            Piece::Block { block, at: r } => {
                let side = place.icon * r.w / ICON_PX;
                let at = place.rect(c, [r.x, r.y], 0.0, 0.0);
                c.overlays.push(OverlayCmd::Blocks {
                    blocks: vec![((0, 0, 0), block.to_string())],
                    at: Rect::new(at.x, at.y, side, side),
                    turning: false,
                });
            }
        }
    }
}

/// What the recipe puts in its layout: ingredients, result, count, texts.
fn recipe_parts(c: &mut Canvas<'_>, place: &Place, vis: &RecipeVis, layout: &RecipeLayout) {
    for part in &layout.parts {
        let cmd = match part {
            LayoutPart::Slots {
                slot, at: places, ..
            } => {
                let ingredients = vis.slots.get(slot);
                for (i, p) in places.iter().enumerate() {
                    if let Some(items) =
                        ingredients.and_then(|l| l.get(i)).filter(|x| !x.is_empty())
                    {
                        let at = place.icon(c, *p);
                        c.overlays.push(OverlayCmd::McItems {
                            items: crate::ingredient_items(items),
                            at,
                        });
                    }
                }
                continue;
            }
            LayoutPart::Result { at: p, .. } => OverlayCmd::McItem {
                item_id: vis.output.clone(),
                at: place.icon(c, *p),
            },
            LayoutPart::Count { at: p } if vis.count > 1 => {
                let r = place.icon(c, *p);
                OverlayCmd::McText {
                    text: format!("x{}", vis.count),
                    at: Vec2::new(r.x, r.y),
                    color: c.theme.title,
                }
            }
            LayoutPart::Text { text, at: p } => {
                let shown = vis.values.iter().fold(text.clone(), |t, (name, value)| {
                    t.replace(&format!("{{{name}}}"), value)
                });
                let r = place.icon(c, *p);
                OverlayCmd::McText {
                    text: shown,
                    at: Vec2::new(r.x, r.y),
                    color: c.theme.text,
                }
            }
            _ => continue,
        };
        c.overlays.push(cmd);
    }
}

/// A mod's own kind of page: its type's elements, top to bottom.
fn custom(
    c: &mut Canvas<'_>,
    body: Vec2,
    mut col: widget::Widget,
    kind: &PageType,
    page: &CustomPage,
) -> widget::Widget {
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    // How far down the next element starts, in book pixels (pictures are overlays).
    let mut top = 0.0;
    for el in &kind.body {
        let (widget, height) = match el {
            PageElement::Header { text, visible } if kind.visible(page, visible) => (
                title_lbl(kind.text(page, text))
                    .color(c.theme.title)
                    .h(PICTURE_TITLE_H * c.sy),
                PICTURE_TITLE_ROOM,
            ),
            PageElement::Text { text, visible } if kind.visible(page, visible) => {
                let words = kind.text(page, text);
                let lines: usize = words
                    .split('\n')
                    .map(|p| yog_ui::text::wrap_text(p, TEXT_W, 1.0).len().max(1))
                    .sum();
                (lbl(words).color(c.theme.text), lines as f32 * line_h + GAP)
            }
            PageElement::Item { item, visible } if kind.visible(page, visible) => {
                let id = yog_formats::book::page_type::words(&kind.value(page, item));
                let at = c.rect(
                    body,
                    Rect::new((PAGE_W - ICON_PX) / 2.0, top, ICON_PX, ICON_PX),
                );
                c.overlays.push(OverlayCmd::icon(&id, at));
                (widget::spacer().h(ICON_PX * c.sy), ITEM_ROOM)
            }
            PageElement::Image {
                image,
                height,
                visible,
            } if kind.visible(page, visible) => {
                let src = yog_formats::book::page_type::words(&kind.value(page, image));
                (
                    widget::image(src)
                        .stretch(Stretch::Fill)
                        .w(PAGE_W * c.sx)
                        .h(height * c.sy),
                    height + GAP,
                )
            }
            PageElement::Drawn {
                drawer,
                height,
                data,
                visible,
            } if kind.visible(page, visible) => {
                let at = c.rect(body, Rect::new(0.0, top, PAGE_W, *height));
                c.overlays.push(OverlayCmd::Drawn {
                    drawer: drawer.clone(),
                    data: kind.value(page, data),
                    at,
                });
                (widget::spacer().h(height * c.sy), height + GAP)
            }
            PageElement::Spacer { height } => (widget::spacer().h(height * c.sy), height + GAP),
            _ => continue,
        };
        col = col.child(widget);
        top += height;
    }
    col
}
