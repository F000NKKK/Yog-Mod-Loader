//! What's drawn around the widgets: sprites from the sheets before them,
//! and after them what yog-ui doesn't draw — items, blocks, entities, the
//! game font's text, mods' pictures.

use yog_gfx::{Color, GfxContext, Rect, Vec2};

use super::geometry::{
    BLOCK_PITCH, ENTITY_TURN, ENTITY_YAW, MULTIBLOCK_SWAY, MULTIBLOCK_YAW, SLOT_YAW,
};
use super::{CRAFTING_SHEET, recipe::normalize_item_id};

#[derive(Clone, PartialEq)]
pub(crate) enum SpriteSheet {
    Book,
    Crafting,
    /// Any image source (a recipe layout's).
    Image(String),
}

/// A part of a sheet drawn before yog-ui.
#[derive(Clone)]
pub(crate) struct BgSprite {
    pub sheet: SpriteSheet,
    /// Source rect in pixels on the sheet.
    pub src: Rect,
    /// Destination on screen.
    pub dst: Rect,
}

/// Draws a mod's picture (`BookPage::Drawn`): `(ctx, drawer, data, [x, y, w, h])`.
pub type PageDrawer<'a> = &'a mut dyn FnMut(&GfxContext, &str, &yog_data::Value, [f32; 4]);

/// Something drawn over the widgets, in its box on screen.
#[derive(Clone)]
pub(crate) enum OverlayCmd {
    /// A mod's picture: its drawer (`BookPage::Drawn`) draws `data` in the box.
    Drawn {
        drawer: String,
        data: yog_data::Value,
        at: Rect,
    },
    /// One of `items` at a time, a new one every second (an ingredient's
    /// alternatives, its tags' items among them).
    McItems { items: Vec<String>, at: Rect },
    /// Blocks in 3D: slowly turning (a multiblock on show), else at a
    /// slot's angle (a station).
    Blocks {
        blocks: Vec<((i32, i32, i32), String)>,
        at: Rect,
        turning: bool,
    },
    /// A living entity standing in the box.
    Entity { entity: String, at: Rect },
    /// The game font's text (the nameplate's title and subtitle).
    /// Text in the game's font, at the book's scale.
    McText {
        text: String,
        at: Vec2,
        color: Color,
        scale: f32,
    },
    /// An item's icon.
    McItem { item_id: String, at: Rect },
    /// A texture drawn whole (16×16) — an icon naming a texture rather
    /// than an item, as Patchouli's `BookIcon.from` reads one.
    McTexture { path: String, at: Rect },
}

impl OverlayCmd {
    /// A book author's icon (a category's, an entry's): a `.png` is a
    /// texture, anything else an item (Patchouli's `BookIcon.from`).
    pub(crate) fn icon(icon: &str, at: Rect) -> OverlayCmd {
        if icon.ends_with(yog_ids::res::PNG) {
            OverlayCmd::McTexture {
                path: icon.to_string(),
                at,
            }
        } else {
            OverlayCmd::McItem {
                item_id: icon.to_string(),
                at,
            }
        }
    }

    /// It moves by itself: the host should keep redrawing.
    pub(crate) fn animated(&self) -> bool {
        match self {
            OverlayCmd::Entity { .. } | OverlayCmd::Blocks { .. } => true,
            OverlayCmd::McItems { items, .. } => items.iter().filter(|i| !is_tag(i)).count() > 1,
            _ => false,
        }
    }
}

/// An ingredient's tag (`#ns:path`), not an item.
fn is_tag(item: &str) -> bool {
    item.starts_with(yog_ids::sep::TAG_SHOWN)
}

/// The sprites, onto `book_sheet` / the crafting sheet / their own images.
pub(crate) fn draw_sprites(ctx: &GfxContext, sprites: &[BgSprite], book_sheet: &str) {
    let d2 = ctx.draw2d();
    for spr in sprites {
        let sheet = match &spr.sheet {
            SpriteSheet::Book => book_sheet,
            SpriteSheet::Crafting => CRAFTING_SHEET,
            SpriteSheet::Image(source) => source.as_str(),
        };
        d2.image_region(sheet, spr.dst, Some(spr.src), Color::WHITE);
    }
}

/// The overlays drawn in 3D or by mods, `t` seconds into the book being open.
pub(crate) fn draw_scenes(
    ctx: &GfxContext,
    overlays: &[OverlayCmd],
    t: f32,
    draw: super::PageDrawer<'_>,
) {
    let d2 = ctx.draw2d();
    for ov in overlays {
        match ov {
            OverlayCmd::Drawn { drawer, data, at } => {
                draw(ctx, drawer, data, [at.x, at.y, at.w, at.h])
            }
            OverlayCmd::Entity { entity, at } => {
                d2.entity(entity, *at, ENTITY_YAW + t * ENTITY_TURN, 0.0)
            }
            OverlayCmd::Blocks {
                blocks,
                at,
                turning,
            } => {
                let list: Vec<((i32, i32, i32), &str)> =
                    blocks.iter().map(|(p, b)| (*p, b.as_str())).collect();
                let yaw = if *turning {
                    MULTIBLOCK_YAW + t * MULTIBLOCK_SWAY
                } else {
                    SLOT_YAW
                };
                d2.blocks(&list, *at, yaw, BLOCK_PITCH);
            }
            _ => {}
        }
    }
}

/// The item stacks and the game font's text — last: item rendering changes
/// GL state. An ingredient's alternatives turn every `second`.
pub(crate) fn draw_items(ctx: &GfxContext, overlays: &[OverlayCmd], second: usize) {
    let d2 = ctx.draw2d();
    for ov in overlays {
        match ov {
            OverlayCmd::McItems { items, at } => {
                let shown: Vec<&String> = items.iter().filter(|i| !is_tag(i)).collect();
                if !shown.is_empty() {
                    let item = shown[second % shown.len()];
                    d2.item(
                        &normalize_item_id(item),
                        Vec2::new(at.x, at.y),
                        at.w.min(at.h),
                    );
                }
            }
            OverlayCmd::McItem { item_id, at } => d2.item(
                &normalize_item_id(item_id),
                Vec2::new(at.x, at.y),
                at.w.min(at.h),
            ),
            OverlayCmd::McText {
                text,
                at,
                color,
                scale,
            } => d2.text_scaled(
                text,
                *at,
                &yog_gfx::draw2d::TextStyle {
                    font: "",
                    scale: *scale,
                    color: *color,
                    shadow: false,
                },
            ),
            // A texture of its own, not an atlas region: drawn whole into the box.
            OverlayCmd::McTexture { path, at } => d2.image(path, *at),
            _ => {}
        }
    }
}
