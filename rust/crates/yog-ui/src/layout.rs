use crate::text;
use crate::widget::{Dock, Widget, WidgetKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDir { Row, Column }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align { Start, Center, End }

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub rect: Rect,
    pub id: Option<String>,
    pub on_click: Option<String>,
    pub children: Vec<LayoutNode>,
    pub enabled: bool,
    pub focused: bool,
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self { rect: Rect::default(), id: None, on_click: None,
               children: Vec::new(), enabled: true, focused: false }
    }
}

/// Compute layout starting at (0,0) with given available size.
/// Returns the root LayoutNode with absolute coordinates.
pub fn compute(widget: &Widget, avail_w: f32, avail_h: f32) -> LayoutNode {
    let mut node = LayoutNode {
        id: widget.id.clone(), on_click: widget.on_click.clone(),
        enabled: widget.enabled, focused: widget.focused,
        ..Default::default()
    };
    layout_widget(widget, &mut node, 0.0, 0.0, avail_w, avail_h);
    node
}

fn layout_widget(w: &Widget, node: &mut LayoutNode, x: f32, y: f32, max_w: f32, max_h: f32) {
    let s = &w.style;
    let has_children = !w.children.is_empty();

    // Determine own size
    let mut ww = if s.w > 0.0 { s.w.min(max_w) } else { max_w };
    let mut hh = if s.h > 0.0 { s.h.min(max_h) } else { max_h };

    if !has_children {
        // Leaf: size to content (text, item slot, spacer)
        match &w.kind {
            WidgetKind::Label(t) | WidgetKind::Button(t) => {
                let avail_w = (max_w - s.pad[1] - s.pad[3]).max(0.0);
                // Wrap only when max_w is a real constraint (not "unlimited")
                // and the widget doesn't opt out of wrapping entirely.
                if avail_w < 4096.0 && !s.no_wrap {
                    ww = (avail_w + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                    hh = (text::text_height(t, avail_w, s.font_scale) + s.pad[0] + s.pad[2])
                        .max(s.min_h).min(max_h);
                } else {
                    let tw = text::str_width(t, s.font_scale);
                    ww = (tw + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                    hh = (text::LINE_H * s.font_scale + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
                }
            }
            WidgetKind::ItemSlot(_) => {
                ww = (18.0 + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                hh = (18.0 + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
            }
            WidgetKind::Spacer => {
                ww = s.min_w.max(1.0).min(max_w);
                hh = s.min_h.max(1.0).min(max_h);
            }
            WidgetKind::Panel(_) => {} // panel with no children → size to min or available
            WidgetKind::McImage { img_w, img_h, .. } => {
                ww = (*img_w + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                hh = (*img_h + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
            }
        }
        // Explicit width/height always wins over content sizing.
        if s.w > 0.0 { ww = s.w.min(max_w); }
        if s.h > 0.0 { hh = s.h.min(max_h); }
    }

    node.rect = Rect { x, y, w: ww, h: hh };

    if !has_children { return; }

    // Flex layout for children
    let dir = if matches!(w.kind, WidgetKind::Panel(_)) && w.flex_dir == FlexDir::Row { FlexDir::Row } else { FlexDir::Column };
    let content_w = ww - s.pad[1] - s.pad[3];
    let content_h = hh - s.pad[0] - s.pad[2];

    // Helpers for Dock
    // Returns effective flex factor (Dock::Fill implies at least 1.0).
    let effective_flex = |child: &Widget| -> f32 {
        if child.style.dock == Dock::Fill { child.style.flex.max(1.0) } else { child.style.flex }
    };
    // Returns true if this child consumes main-axis space in the normal forward pass.
    let in_flow = |child: &Widget| -> bool {
        match (dir, child.style.dock) {
            (FlexDir::Row,    Dock::Right)  => false,
            (FlexDir::Column, Dock::Bottom) => false,
            _ => true,
        }
    };

    // Measure children
    let mut child_nodes: Vec<LayoutNode> = Vec::new();
    let mut total_flex: f32 = 0.0;
    let mut used_main: f32  = 0.0;

    for child in &w.children {
        let dock = child.style.dock;
        let mut cn = LayoutNode {
            id: child.id.clone(), on_click: child.on_click.clone(),
            enabled: child.enabled, focused: child.focused,
            ..Default::default()
        };
        // Determine measurement constraints based on Dock + direction.
        let (cmw, cmh) = match (dir, dock) {
            // Fill: constrain both axes so text can wrap to container dimensions.
            (FlexDir::Row,    Dock::Fill) => (content_w, content_h),
            (FlexDir::Column, Dock::Fill) => (content_w, content_h),
            // Cross-axis fill: constrain cross axis, unlimited main axis.
            (FlexDir::Row,    Dock::Left | Dock::Right) => (f32::MAX, content_h),
            (FlexDir::Column, Dock::Top  | Dock::Bottom) => (content_w, f32::MAX),
            // Default flex behaviour.
            (FlexDir::Row,    _) => (f32::MAX, content_h),
            (FlexDir::Column, _) => (content_w, f32::MAX),
        };
        layout_widget(child, &mut cn, 0.0, 0.0, cmw, cmh);
        if in_flow(child) {
            if dir == FlexDir::Row { used_main += cn.rect.w; }
            else                   { used_main += cn.rect.h; }
        }
        total_flex += effective_flex(child);
        child_nodes.push(cn);
    }
    let flow_count = w.children.iter().filter(|c| in_flow(c)).count();
    let gaps = s.gap * (flow_count.saturating_sub(1) as f32);
    used_main += gaps;

    let available = (if dir == FlexDir::Row { content_w } else { content_h }) - used_main;
    let mut pos = if dir == FlexDir::Row { s.pad[3] } else { s.pad[0] };

    // --- Forward pass: position in-flow children (not Dock::Right / Dock::Bottom) ---
    for (i, child) in w.children.iter().enumerate() {
        if !in_flow(child) { continue; }
        let dock = child.style.dock;
        let cn = &mut child_nodes[i];
        if dir == FlexDir::Row {
            let ef = effective_flex(child);
            if ef > 0.0 && total_flex > 0.0 && available > 0.0 {
                cn.rect.w += available * ef / total_flex;
            }
            if dock == Dock::Fill || dock == Dock::Left || dock == Dock::Right {
                cn.rect.h = content_h; // stretch cross axis
            }
            cn.rect.x = x + pos;
            cn.rect.y = y + s.pad[0] + match s.align {
                Align::Center => (content_h - cn.rect.h) / 2.0,
                Align::End    => content_h - cn.rect.h,
                _             => 0.0,
            };
            pos += cn.rect.w + s.gap;
        } else {
            let ef = effective_flex(child);
            if ef > 0.0 && total_flex > 0.0 && available > 0.0 {
                cn.rect.h += available * ef / total_flex;
            }
            if dock == Dock::Fill || dock == Dock::Top || dock == Dock::Bottom {
                cn.rect.w = content_w; // stretch cross axis
            }
            cn.rect.x = x + s.pad[3] + match s.align {
                Align::Center => (content_w - cn.rect.w) / 2.0,
                Align::End    => content_w - cn.rect.w,
                _             => 0.0,
            };
            cn.rect.y = y + pos;
            pos += cn.rect.h + s.gap;
        }
        if !child.children.is_empty() {
            layout_widget(child, cn, cn.rect.x, cn.rect.y, cn.rect.w, cn.rect.h);
        }
    }

    // --- Reverse pass: position Dock::Right / Dock::Bottom children from the far edge ---
    let mut rpos = if dir == FlexDir::Row {
        x + s.pad[3] + content_w
    } else {
        y + s.pad[0] + content_h
    };
    for (i, child) in w.children.iter().enumerate() {
        if in_flow(child) { continue; }
        let dock = child.style.dock;
        let cn = &mut child_nodes[i];
        if dir == FlexDir::Row {
            if dock == Dock::Fill || dock == Dock::Left || dock == Dock::Right {
                cn.rect.h = content_h;
            }
            rpos -= cn.rect.w;
            cn.rect.x = rpos;
            cn.rect.y = y + s.pad[0] + match s.align {
                Align::Center => (content_h - cn.rect.h) / 2.0,
                Align::End    => content_h - cn.rect.h,
                _             => 0.0,
            };
            rpos -= s.gap;
        } else {
            if dock == Dock::Fill || dock == Dock::Top || dock == Dock::Bottom {
                cn.rect.w = content_w;
            }
            rpos -= cn.rect.h;
            cn.rect.y = rpos;
            cn.rect.x = x + s.pad[3] + match s.align {
                Align::Center => (content_w - cn.rect.w) / 2.0,
                Align::End    => content_w - cn.rect.w,
                _             => 0.0,
            };
            rpos -= s.gap;
        }
        if !child.children.is_empty() {
            layout_widget(child, cn, cn.rect.x, cn.rect.y, cn.rect.w, cn.rect.h);
        }
    }

    // Auto-size: shrink to content
    if s.w <= 0.0 {
        let cw: f32 = child_nodes.iter().map(|c| c.rect.x - x + c.rect.w).fold(0.0f32, f32::max);
        node.rect.w = (cw + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
    }
    if s.h <= 0.0 {
        let ch: f32 = child_nodes.iter().map(|c| c.rect.y - y + c.rect.h).fold(0.0f32, f32::max);
        node.rect.h = (ch + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
    }

    node.children = child_nodes;
}

/// Hit-test: find deepest clickable, enabled node at (mx, my).
pub fn hit_test(node: &LayoutNode, mx: f32, my: f32) -> Option<&LayoutNode> {
    let r = &node.rect;
    if mx < r.x || my < r.y || mx > r.x + r.w || my > r.y + r.h { return None; }
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test(child, mx, my) { return Some(hit); }
    }
    if node.on_click.is_some() && node.enabled { Some(node) } else { None }
}

/// Walk tree and set `focused = true` on the node whose id matches, false on all others.
pub fn set_focus(node: &mut LayoutNode, focused_id: Option<&str>) {
    node.focused = focused_id.is_some() && node.id.as_deref() == focused_id;
    for child in &mut node.children { set_focus(child, focused_id); }
}
