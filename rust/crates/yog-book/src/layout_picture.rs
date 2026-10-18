//! What a recipe layout draws whatever the recipe — its images, its slot
//! frames, its stations — as pieces in the layout's own pixels: the book
//! and Yog-IDLE's recipe editor each draw these their own way.

use yog_formats::book::layout::{ICON_PX, LayoutPart, RecipeLayout};
use yog_gfx::Rect;
use yog_ids::yog::CRAFTING_SHEET;

/// A slot frame on [`CRAFTING_SHEET`], 24×24, the item inside at
/// [`FRAME_INSET`].
pub const FRAME_UV: [f32; 4] = [83.0, 71.0, 24.0, 24.0];

/// How far a slot frame reaches around its item.
pub const FRAME_INSET: f32 = 4.0;

/// One thing a layout draws whatever the recipe.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece<'a> {
    /// Region `uv` of image `source`, drawn at `at`.
    Image { source: &'a str, uv: Rect, at: Rect },
    /// Item `item`'s icon.
    Item { item: &'a str, at: Rect },
    /// Block state `block`, in 3D as the game shows blocks in slots.
    Block { block: &'a str, at: Rect },
}

/// `layout`'s pieces, in the order its parts list them.
#[must_use]
pub fn pieces(layout: &RecipeLayout) -> Vec<Piece<'_>> {
    let [fu, fv, fw, fh] = FRAME_UV;
    let frame = |[x, y]: [f32; 2]| Piece::Image {
        source: CRAFTING_SHEET,
        uv: Rect::new(fu, fv, fw, fh),
        at: Rect::new(x - FRAME_INSET, y - FRAME_INSET, fw, fh),
    };
    let mut out = Vec::new();
    for part in &layout.parts {
        match part {
            LayoutPart::Image { source, uv, at } => {
                let [u, v, w, h] = *uv;
                out.push(Piece::Image { source, uv: Rect::new(u, v, w, h), at: Rect::new(at[0], at[1], w, h) });
            }
            LayoutPart::Slots { at, frame: true, .. } => out.extend(at.iter().map(|a| frame(*a))),
            LayoutPart::Result { at, frame: true } => out.push(frame(*at)),
            LayoutPart::Item { item, at } => out.push(Piece::Item { item, at: Rect::new(at[0], at[1], ICON_PX, ICON_PX) }),
            LayoutPart::Block { block, at, size } => out.push(Piece::Block { block, at: Rect::new(at[0], at[1], *size, *size) }),
            LayoutPart::Slots { .. } | LayoutPart::Result { .. } | LayoutPart::Count { .. } | LayoutPart::Text { .. } => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_sit_around_their_items() {
        let l = yog_formats::book::layout::find(&[], "smelting").expect("a built-in layout");
        let p = pieces(l);
        assert!(p.iter().any(|p| matches!(p, Piece::Image { source, .. } if *source == CRAFTING_SHEET)));
    }
}
