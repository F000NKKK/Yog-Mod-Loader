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

    match &widget.kind {
        WidgetKind::Panel => {
            // Panel just draws children
        }
        WidgetKind::Label(text) | WidgetKind::Button(text) => {
            // Text centered vertically, with padding
            let tx = r.x + s.pad[3] + 2.0;
            let ty = r.y + s.pad[0] + (r.h - s.pad[0] - s.pad[2] - 9.0) / 2.0;
            d2d.text(text, tx, ty.max(r.y + s.pad[0]), s.color, true);
        }
        WidgetKind::ItemSlot(item_id) => {
            // Draw item icon using Minecraft's item atlas
            // We approximate with a simple rect + text for now;
            // proper item rendering needs mc_texture with atlas coords.
            // For now, draw a slot background and text label.
            let inner_x = r.x + s.pad[3];
            let inner_y = r.y + s.pad[0];
            let inner_w = r.w - s.pad[1] - s.pad[3];
            let inner_h = r.h - s.pad[0] - s.pad[2];
            d2d.rect(inner_x, inner_y, inner_x + inner_w, inner_y + inner_h, 0xFF_444444);
            d2d.rect(inner_x + 1.0, inner_y + 1.0, inner_x + inner_w - 1.0, inner_y + inner_h - 1.0, 0xFF_888888);
            // Try to draw the item's texture
            let ns_path = item_id.replace(':', ":textures/item/");
            let tex_id = format!("{}:{}.png", item_id.split(':').next().unwrap_or("minecraft"), 
                item_id.split(':').nth(1).unwrap_or("air"));
            d2d.mc_texture(
                &format!("minecraft:textures/item/{}.png", item_id.split(':').nth(1).unwrap_or("air")),
                inner_x + 1.0, inner_y + 1.0,
                0.0, 0.0, 16.0, 16.0, 16.0, 16.0,
            );
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
