//! The view's contents at every scroll the tab takes — its background and
//! tree as one picture — so a tool slides it while the tree is dragged and
//! asks for nothing until the drag ends.

use yog_gfx::{Rect, Vec2};

use super::super::assets::Resources;
use super::super::{gfx, raster};
use super::{Args, SCALE, SCREEN, Tab, VIEW, draw_background, draw_tree, draw_window};

/// The most pixels the picture gets; a bigger tree is drawn per drag instead.
const MAX_PIXELS: u64 = 1 << 22;

/// The view's contents over every scroll, and the window to lay over them.
#[derive(Debug, Clone, yog_data::Schema)]
#[data(rename_all = "camelCase")]
pub(crate) struct Layer {
    pub(crate) image: String,
    /// The window and its title alone, the screen's size: its edge lies
    /// over the view's.
    pub(crate) window: String,
    pub(crate) size: [f32; 2],
    /// Where its top left is from the view's, at the drag drawn.
    pub(crate) at: [f32; 2],
    /// The drags the tab takes, per axis: the least and the most.
    pub(crate) drag_min: [f32; 2],
    pub(crate) drag_max: [f32; 2],
}

/// `tab`'s contents over every scroll it takes, placed for `drag`.
pub(super) fn render(args: &Args, tab: &Tab, drag: [f32; 2], res: &Resources) -> Option<Layer> {
    let [[x0, x1], [y0, y1]] = tab.range;
    let size = Vec2::new(x1 - x0 + VIEW.w, y1 - y0 + VIEW.h);
    let (w, h) = (size.x as u32, size.y as u32);
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    // The tree's place in it: the view's top left at the most scroll is its corner.
    let origin = Vec2::new(x1, y1);
    let ops = gfx::record(w as i32, h as i32, |ctx| {
        let d = &ctx.draw2d();
        draw_background(d, args, Rect::new(0.0, 0.0, size.x, size.y), origin);
        draw_tree(d, args, &tab.spots, origin);
    });
    let image = raster::rasterize(&ops, w, h, SCALE, res).png_url()?;
    let ops = gfx::record(SCREEN.0 as i32, SCREEN.1 as i32, |ctx| {
        draw_window(&ctx.draw2d(), args, tab);
    });
    let window = raster::rasterize(&ops, SCREEN.0 as u32, SCREEN.1 as u32, SCALE, res).png_url()?;
    let scroll = |centre: f32, drag: f32| (centre + drag).floor();
    Some(Layer {
        image,
        window,
        size: [size.x, size.y],
        at: [
            scroll(tab.centre.x, drag[0]) - origin.x,
            scroll(tab.centre.y, drag[1]) - origin.y,
        ],
        drag_min: [x0 - tab.centre.x, y0 - tab.centre.y],
        drag_max: [x1 - tab.centre.x, y1 - tab.centre.y],
    })
}
