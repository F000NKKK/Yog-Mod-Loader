//! Flexbox-inspired layout engine.

use crate::widget::Widget;

/// 2D size in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Size { pub w: f32, pub h: f32 }

/// Position + size rectangle.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

/// Layout direction for flex containers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDir { Row, Column }

/// Cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align { Start, Center, End }

/// Computed layout node — positions are absolute screen coordinates.
#[derive(Debug, Clone, Default)]
pub struct LayoutNode {
    pub rect: Rect,
    pub id: Option<String>,
    pub on_click: Option<String>,
    pub children: Vec<LayoutNode>,
}

/// Recursively compute layout for the widget tree inside given screen bounds.
pub fn compute(widget: &Widget, screen_w: f32, screen_h: f32) -> LayoutNode {
    let ctx = LayoutCtx {
        available_w: screen_w - widget.style.pad[1] - widget.style.pad[3]
                      - widget.style.margin[1] - widget.style.margin[3],
        available_h: screen_h - widget.style.pad[0] - widget.style.pad[2]
                      - widget.style.margin[0] - widget.style.margin[2],
    };
    let mut node = LayoutNode {
        id: widget.id.clone(),
        on_click: widget.on_click.clone(),
        ..Default::default()
    };
    layout_node(widget, &mut node, &ctx, 0.0, 0.0,
                screen_w - widget.style.margin[1] - widget.style.margin[3],
                screen_h - widget.style.margin[0] - widget.style.margin[2]);
    node
}

struct LayoutCtx { available_w: f32, available_h: f32 }

fn layout_node(w: &Widget, node: &mut LayoutNode, ctx: &LayoutCtx,
               px: f32, py: f32, max_w: f32, max_h: f32) {
    let s = &w.style;
    // Apply explicit size or auto
    let ww = if s.w > 0.0 { s.w } else { max_w };
    let hh = if s.h > 0.0 { s.h } else { max_h };

    node.rect = Rect { x: px + s.margin[3], y: py + s.margin[0], w: ww, h: hh };

    if w.children.is_empty() {
        // Auto-size leaf: use min-size or available
        if s.w <= 0.0 { node.rect.w = s.min_w.max(ww.min(ctx.available_w)).max(1.0); }
        if s.h <= 0.0 { node.rect.h = s.min_h.max(hh.min(ctx.available_h)).max(1.0); }
        return;
    }

    let dir = if w.flex_dir == FlexDir::Row { FlexDir::Row } else { FlexDir::Column };
    let content_w = node.rect.w - s.pad[1] - s.pad[3];
    let content_h = node.rect.h - s.pad[0] - s.pad[2];

    // First pass: measure non-flex children
    let mut main_size: f32 = 0.0;
    let mut total_flex: f32 = 0.0;
    for child in &w.children {
        let mut cn = LayoutNode::default();
        layout_node(child, &mut cn, ctx,
            node.rect.x + s.pad[3], node.rect.y + s.pad[0],
            if dir == FlexDir::Row { f32::MAX / 4.0 } else { content_w },
            if dir == FlexDir::Column { f32::MAX / 4.0 } else { content_h },
        );
        if dir == FlexDir::Row { main_size += cn.rect.w; }
        else { main_size += cn.rect.h; }
        total_flex += child.style.flex;
        node.children.push(cn);
    }
    let gap = (s.gap * (w.children.len().saturating_sub(1) as f32)).max(0.0);
    main_size += gap;

    let available = (if dir == FlexDir::Row { content_w } else { content_h }) - main_size;
    let mut pos = if dir == FlexDir::Row { s.pad[3] } else { s.pad[0] };

    // Second pass: position children
    for (i, child) in w.children.iter().enumerate() {
        let cn = &mut node.children[i];
        if dir == FlexDir::Row {
            if child.style.flex > 0.0 && total_flex > 0.0 {
                cn.rect.w += available * child.style.flex / total_flex;
            }
            cn.rect.x = node.rect.x + pos;
            cn.rect.y = node.rect.y + s.pad[0] + align_offset(s.align, content_h, cn.rect.h);
            pos += cn.rect.w + s.gap;
        } else {
            if child.style.flex > 0.0 && total_flex > 0.0 {
                cn.rect.h += available * child.style.flex / total_flex;
            }
            cn.rect.x = node.rect.x + s.pad[3] + align_offset(s.align, content_w, cn.rect.w);
            cn.rect.y = node.rect.y + pos;
            pos += cn.rect.h + s.gap;
        }
    }

    // Shrink to content
    if s.w <= 0.0 {
        let cw: f32 = node.children.iter().map(|c| c.rect.x - node.rect.x + c.rect.w).fold(0.0f32, f32::max);
        node.rect.w = (cw + s.pad[1] + s.pad[3]).max(s.min_w);
    }
    if s.h <= 0.0 {
        let ch: f32 = node.children.iter().map(|c| c.rect.y - node.rect.y + c.rect.h).fold(0.0f32, f32::max);
        node.rect.h = (ch + s.pad[0] + s.pad[2]).max(s.min_h);
    }
}

fn align_offset(align: Align, container: f32, child: f32) -> f32 {
    let diff = container - child;
    if diff <= 0.0 { return 0.0; }
    match align {
        Align::Start  => 0.0,
        Align::Center => diff / 2.0,
        Align::End    => diff,
    }
}

/// Find the deepest clickable node at `(mx, my)`.
pub fn hit_test(node: &LayoutNode, mx: f32, my: f32) -> Option<&LayoutNode> {
    if mx < node.rect.x || my < node.rect.y
        || mx > node.rect.x + node.rect.w || my > node.rect.y + node.rect.h { return None; }
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test(child, mx, my) { return Some(hit); }
    }
    if node.on_click.is_some() { Some(node) } else { None }
}
