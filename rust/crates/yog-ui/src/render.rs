//! GPU rendering for the UI tree via `yog-gfx` draw2d.
//! Only valid inside `on_hud_render`.

use crate::layout::LayoutNode;
use crate::widget::{Widget, WidgetKind};
use yog_gfx::draw2d::Draw2D;

/// Recursively render a widget tree starting from the computed layout.
pub fn render_node(d2d: &Draw2D, widget: &Widget, node: &LayoutNode) {
    let r = &node.rect;
    let s = &widget.style;

    // Background
    if s.bg != 0 {
        d2d.rect(r.x, r.y, r.x + r.w, r.y + r.h, s.bg);
    }
    // Focus ring — 1px amber outline for focused widgets
    if node.focused {
        d2d.rect(r.x,             r.y,             r.x + r.w,     r.y + 1.0,     0xFF_FFE040);
        d2d.rect(r.x,             r.y + r.h - 1.0, r.x + r.w,     r.y + r.h,     0xFF_FFE040);
        d2d.rect(r.x,             r.y,             r.x + 1.0,     r.y + r.h,     0xFF_FFE040);
        d2d.rect(r.x + r.w - 1.0, r.y,            r.x + r.w,     r.y + r.h,     0xFF_FFE040);
    }

    match &widget.kind {
        WidgetKind::Panel(_) => {
            // Panel just draws children
        }
        WidgetKind::Label(text) | WidgetKind::Button(text) => {
            // Text centered vertically, with padding
            let tx = r.x + s.pad[3] + 2.0;
            let ty = r.y + s.pad[0] + (r.h - s.pad[0] - s.pad[2] - 9.0) / 2.0;
            d2d.text(text, tx, ty.max(r.y + s.pad[0]), s.color, true);
        }
        WidgetKind::ItemSlot(item_id) => {
            let ix = r.x + s.pad[3]; let iy = r.y + s.pad[0];
            let sz = 18.0;
            d2d.rect(ix, iy, ix + sz, iy + sz, 0xFF_444444);
            d2d.rect(ix+1.0, iy+1.0, ix+sz-1.0, iy+sz-1.0, 0xFF_888888);
            if let Some((ns, name)) = item_id.split_once(':') {
                d2d.mc_texture(&format!("{ns}:textures/item/{name}.png"),
                    ix+2.0, iy+2.0, 0.0, 0.0, 14.0, 14.0, 16.0, 16.0);
            }
        }
        WidgetKind::McImage { id, img_w, img_h } => {
            d2d.mc_texture(id, r.x + s.pad[3], r.y + s.pad[0],
                0.0, 0.0, *img_w, *img_h, *img_w, *img_h);
        }
        WidgetKind::Spacer => {} // invisible
    }

    // Render children
    for (i, child_widget) in widget.children.iter().enumerate() {
        if let Some(child_node) = node.children.get(i) {
            render_node(d2d, child_widget, child_node);
        }
    }
}
