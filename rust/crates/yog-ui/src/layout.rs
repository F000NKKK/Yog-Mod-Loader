use crate::text;
use crate::widget::{Dock, Widget, WidgetKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDir {
    Row,
    Column,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Width of the strip a [`scrollbar`](crate::widget::Style::scrollbar)
/// container reserves on its right edge.
pub const SCROLLBAR_W: f32 = 6.0;
/// Shortest a scrollbar knob gets.
pub const MIN_KNOB_H: f32 = 16.0;

impl Rect {
    /// Whether `(px, py)` lies inside (edges included).
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px <= self.x + self.w && py <= self.y + self.h
    }

    /// The overlap of two rects; `None` when they don't overlap.
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub rect: Rect,
    pub id: Option<String>,
    pub on_click: Option<String>,
    pub children: Vec<LayoutNode>,
    pub enabled: bool,
    pub focused: bool,
    /// Height of everything inside — children plus padding — before
    /// scrolling. Larger than `rect.h` when the children overflow.
    pub content_h: f32,
    /// Effective vertical scroll offset: the widget's
    /// [`scroll_y`](crate::widget::Style::scroll_y) clamped to
    /// `0..=`[`Self::max_scroll`]. Children are already shifted by it.
    pub scroll_y: f32,
    /// The widget clips its children (see `Style::clip`).
    pub clip: bool,
    /// The widget shows a scrollbar on overflow (see `Style::scrollbar`).
    pub scrollbar: bool,
}

impl LayoutNode {
    /// How far the children can scroll: `content_h - rect.h`, at least 0.
    pub fn max_scroll(&self) -> f32 {
        (self.content_h - self.rect.h).max(0.0)
    }

    /// Scrollbar track and knob rects — `None` unless the widget has a
    /// scrollbar and its content overflows.
    pub fn scrollbar_rects(&self) -> Option<(Rect, Rect)> {
        let max = self.max_scroll();
        if !self.scrollbar || max <= 0.0 {
            return None;
        }
        let r = self.rect;
        let track = Rect {
            x: r.x + r.w - SCROLLBAR_W,
            y: r.y,
            w: SCROLLBAR_W,
            h: r.h,
        };
        let knob_h = (r.h * r.h / self.content_h).max(MIN_KNOB_H).min(r.h);
        let knob = Rect {
            y: r.y + (r.h - knob_h) * (self.scroll_y / max),
            h: knob_h,
            ..track
        };
        Some((track, knob))
    }

    fn translate_y(&mut self, dy: f32) {
        self.rect.y += dy;
        for c in &mut self.children {
            c.translate_y(dy);
        }
    }
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self {
            rect: Rect::default(),
            id: None,
            on_click: None,
            children: Vec::new(),
            enabled: true,
            focused: false,
            content_h: 0.0,
            scroll_y: 0.0,
            clip: false,
            scrollbar: false,
        }
    }
}

/// Compute layout starting at (0,0) with given available size.
/// Returns the root LayoutNode with absolute coordinates.
pub fn compute(widget: &Widget, avail_w: f32, avail_h: f32) -> LayoutNode {
    let mut node = LayoutNode {
        id: widget.id.clone(),
        on_click: widget.on_click.clone(),
        enabled: widget.enabled,
        focused: widget.focused,
        ..Default::default()
    };
    layout_widget(widget, &mut node, 0.0, 0.0, avail_w, avail_h);
    node
}

fn layout_widget(w: &Widget, node: &mut LayoutNode, x: f32, y: f32, max_w: f32, max_h: f32) {
    let s = &w.style;
    let has_children = !w.children.is_empty();

    // Determine own size
    let mut ww = s.w.resolve(max_w).map(|v| v.min(max_w)).unwrap_or(max_w);
    let mut hh = s.h.resolve(max_h).map(|v| v.min(max_h)).unwrap_or(max_h);

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
                        .max(s.min_h)
                        .min(max_h);
                } else {
                    let tw = text::str_width(t, s.font_scale);
                    ww = (tw + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                    hh = (text::LINE_H * s.font_scale + s.pad[0] + s.pad[2])
                        .max(s.min_h)
                        .min(max_h);
                }
            }
            WidgetKind::ItemSlot(_) | WidgetKind::InvSlot { .. } => {
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
        if let Some(v) = s.w.resolve(max_w) {
            ww = v.min(max_w);
        }
        if let Some(v) = s.h.resolve(max_h) {
            hh = v.min(max_h);
        }
    }

    node.rect = Rect { x, y, w: ww, h: hh };
    node.content_h = hh;
    node.scroll_y = 0.0;
    node.clip = s.clip;
    node.scrollbar = s.scrollbar;

    if !has_children {
        return;
    }

    // Flex layout for children
    let dir = if matches!(w.kind, WidgetKind::Panel(_)) && w.flex_dir == FlexDir::Row {
        FlexDir::Row
    } else {
        FlexDir::Column
    };
    let bar_w = if s.scrollbar { SCROLLBAR_W } else { 0.0 };
    let content_w = (ww - s.pad[1] - s.pad[3] - bar_w).max(0.0);
    let content_h = hh - s.pad[0] - s.pad[2];

    // Helpers for Dock
    // Returns effective flex factor (Dock::Fill implies at least 1.0).
    let effective_flex = |child: &Widget| -> f32 {
        if child.style.dock == Dock::Fill {
            child.style.flex.max(1.0)
        } else {
            child.style.flex
        }
    };
    // Returns true if this child consumes main-axis space in the normal forward pass.
    let in_flow = |child: &Widget| -> bool {
        child.style.pos.is_none()
            && !matches!(
                (dir, child.style.dock),
                (FlexDir::Row, Dock::Right) | (FlexDir::Column, Dock::Bottom)
            )
    };

    // Measure children
    let mut child_nodes: Vec<LayoutNode> = Vec::new();
    let mut total_flex: f32 = 0.0;
    let mut used_main: f32 = 0.0;

    for child in &w.children {
        let dock = child.style.dock;
        let mut cn = LayoutNode {
            id: child.id.clone(),
            on_click: child.on_click.clone(),
            enabled: child.enabled,
            focused: child.focused,
            ..Default::default()
        };
        // Absolutely positioned: measured against what's left of the content
        // box past its offset, placed after the flow passes.
        if let Some((px, py)) = child.style.pos {
            let mw = (content_w - px).max(0.0);
            let mh = (content_h - py).max(0.0);
            layout_widget(child, &mut cn, 0.0, 0.0, mw, mh);
            child_nodes.push(cn);
            continue;
        }
        // Determine measurement constraints based on Dock + direction.
        let (cmw, cmh) = match (dir, dock) {
            // Fill: constrain both axes so text can wrap to container dimensions.
            (FlexDir::Row, Dock::Fill) => (content_w, content_h),
            (FlexDir::Column, Dock::Fill) => (content_w, content_h),
            // Cross-axis fill: constrain cross axis, unlimited main axis.
            (FlexDir::Row, Dock::Left | Dock::Right) => (f32::MAX, content_h),
            (FlexDir::Column, Dock::Top | Dock::Bottom) => (content_w, f32::MAX),
            // Default flex behaviour.
            (FlexDir::Row, _) => (f32::MAX, content_h),
            (FlexDir::Column, _) => (content_w, f32::MAX),
        };
        layout_widget(child, &mut cn, 0.0, 0.0, cmw, cmh);
        if in_flow(child) {
            if dir == FlexDir::Row {
                used_main += cn.rect.w;
            } else {
                used_main += cn.rect.h;
            }
        }
        total_flex += effective_flex(child);
        child_nodes.push(cn);
    }
    let flow_count = w.children.iter().filter(|c| in_flow(c)).count();
    let gaps = s.gap * (flow_count.saturating_sub(1) as f32);
    used_main += gaps;

    let available = (if dir == FlexDir::Row {
        content_w
    } else {
        content_h
    }) - used_main;
    let mut pos = if dir == FlexDir::Row {
        s.pad[3]
    } else {
        s.pad[0]
    };

    // --- Forward pass: position in-flow children (not Dock::Right / Dock::Bottom) ---
    for (i, child) in w.children.iter().enumerate() {
        if !in_flow(child) {
            continue;
        }
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
            cn.rect.y = y
                + s.pad[0]
                + match s.align {
                    Align::Center => (content_h - cn.rect.h) / 2.0,
                    Align::End => content_h - cn.rect.h,
                    _ => 0.0,
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
            cn.rect.x = x
                + s.pad[3]
                + match s.align {
                    Align::Center => (content_w - cn.rect.w) / 2.0,
                    Align::End => content_w - cn.rect.w,
                    _ => 0.0,
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
        if in_flow(child) || child.style.pos.is_some() {
            continue;
        }
        let dock = child.style.dock;
        let cn = &mut child_nodes[i];
        if dir == FlexDir::Row {
            if dock == Dock::Fill || dock == Dock::Left || dock == Dock::Right {
                cn.rect.h = content_h;
            }
            rpos -= cn.rect.w;
            cn.rect.x = rpos;
            cn.rect.y = y
                + s.pad[0]
                + match s.align {
                    Align::Center => (content_h - cn.rect.h) / 2.0,
                    Align::End => content_h - cn.rect.h,
                    _ => 0.0,
                };
            rpos -= s.gap;
        } else {
            if dock == Dock::Fill || dock == Dock::Top || dock == Dock::Bottom {
                cn.rect.w = content_w;
            }
            rpos -= cn.rect.h;
            cn.rect.y = rpos;
            cn.rect.x = x
                + s.pad[3]
                + match s.align {
                    Align::Center => (content_w - cn.rect.w) / 2.0,
                    Align::End => content_w - cn.rect.w,
                    _ => 0.0,
                };
            rpos -= s.gap;
        }
        if !child.children.is_empty() {
            layout_widget(child, cn, cn.rect.x, cn.rect.y, cn.rect.w, cn.rect.h);
        }
    }

    // --- Absolutely positioned children (see `Style::pos`) ---
    for (i, child) in w.children.iter().enumerate() {
        let Some((px, py)) = child.style.pos else {
            continue;
        };
        let cn = &mut child_nodes[i];
        cn.rect.x = x + s.pad[3] + px;
        cn.rect.y = y + s.pad[0] + py;
        if !child.children.is_empty() {
            layout_widget(child, cn, cn.rect.x, cn.rect.y, cn.rect.w, cn.rect.h);
        }
    }

    // Auto-size: shrink to content
    if s.w.is_auto() {
        let cw: f32 = child_nodes
            .iter()
            .map(|c| c.rect.x - x + c.rect.w)
            .fold(0.0f32, f32::max);
        node.rect.w = (cw + s.pad[1] + s.pad[3] + bar_w).max(s.min_w).min(max_w);
    }
    let ch: f32 = child_nodes
        .iter()
        .map(|c| c.rect.y - y + c.rect.h)
        .fold(0.0f32, f32::max);
    if s.h.is_auto() {
        node.rect.h = (ch + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
    }

    // Scrolling: children positioned from the content's top, then shifted up.
    // (`ch` already includes the top padding: child y is relative to `y`.)
    node.content_h = ch + s.pad[2];
    node.scroll_y = s.scroll_y.clamp(0.0, node.max_scroll());
    if node.scroll_y > 0.0 {
        for c in &mut child_nodes {
            c.translate_y(-node.scroll_y);
        }
    }

    node.children = child_nodes;
}

/// Hit-test: find deepest clickable, enabled node at (mx, my).
/// Children only count where they're inside their parent's rect, so rows
/// scrolled out of a clipping container can't be clicked.
pub fn hit_test(node: &LayoutNode, mx: f32, my: f32) -> Option<&LayoutNode> {
    if !node.rect.contains(mx, my) {
        return None;
    }
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test(child, mx, my) {
            return Some(hit);
        }
    }
    if node.on_click.is_some() && node.enabled {
        Some(node)
    } else {
        None
    }
}

/// Find the resolved rect of the (first) node tagged `id` (see `Widget::id`).
/// Meant for baking a widget tree's computed positions into some other
/// system at "measure time" (e.g. a mod reporting real-slot pixel offsets to
/// its host) — a WinForms-style measure-then-use pass, not a per-frame call.
pub fn find_by_id(node: &LayoutNode, id: &str) -> Option<Rect> {
    if node.id.as_deref() == Some(id) {
        return Some(node.rect);
    }
    node.children.iter().find_map(|c| find_by_id(c, id))
}

/// The (first) node tagged `id` — its rect plus scrolling state
/// ([`LayoutNode::scroll_y`], [`LayoutNode::scrollbar_rects`], ...).
pub fn find_node<'a>(node: &'a LayoutNode, id: &str) -> Option<&'a LayoutNode> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_node(c, id))
}

/// Walk tree and set `focused = true` on the node whose id matches, false on all others.
pub fn set_focus(node: &mut LayoutNode, focused_id: Option<&str>) {
    node.focused = focused_id.is_some() && node.id.as_deref() == focused_id;
    for child in &mut node.children {
        set_focus(child, focused_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{self, Units};

    #[test]
    fn positioned_children_leave_the_flow() {
        let root = widget::panel(FlexDir::Column)
            .w(200.0)
            .h(100.0)
            .padding(4.0, 4.0, 4.0, 4.0)
            .child(widget::spacer().min_w(10.0).min_h(10.0))
            .child(widget::button("Hi").w(40.0).h(20.0).at(50.0, 30.0))
            .child(widget::spacer().min_w(10.0).min_h(10.0));
        let node = compute(&root, 400.0, 400.0);
        let r = node.children[1].rect;
        assert_eq!((r.x, r.y, r.w, r.h), (54.0, 34.0, 40.0, 20.0));
        // The flow children stack as if the positioned one weren't there.
        assert_eq!(node.children[0].rect.y, 4.0);
        assert_eq!(node.children[2].rect.y, 4.0 + 10.0 + 2.0);
    }

    #[test]
    fn positioned_container_lays_out_its_children_in_place() {
        let root = widget::panel(FlexDir::Column)
            .w(Units::Percent(100.0))
            .h(Units::Percent(100.0))
            .child(
                widget::panel(FlexDir::Row)
                    .w(100.0)
                    .h(20.0)
                    .gap(0.0)
                    .at(10.0, 20.0)
                    .child(widget::spacer().min_w(30.0).min_h(5.0))
                    .child(widget::spacer().min_w(30.0).min_h(5.0)),
            );
        let node = compute(&root, 320.0, 240.0);
        let panel = &node.children[0];
        assert_eq!((panel.rect.x, panel.rect.y), (10.0, 20.0));
        assert_eq!(panel.children[1].rect.x, 40.0);
        assert_eq!(panel.children[1].rect.y, 20.0);
    }

    #[test]
    fn auto_sized_parent_grows_to_fit_positioned_children() {
        let root = widget::panel(FlexDir::Column)
            .child(widget::spacer().min_w(10.0).min_h(10.0).at(30.0, 40.0));
        let node = compute(&root, 300.0, 300.0);
        assert_eq!((node.rect.w, node.rect.h), (40.0, 50.0));
    }

    #[test]
    fn find_mut_and_set_text() {
        let mut root = widget::panel(FlexDir::Column).child(widget::label("a").id("title"));
        assert!(root.find_mut("title").unwrap().set_text("b"));
        match &root.children[0].kind {
            crate::widget::WidgetKind::Label(t) => assert_eq!(t, "b"),
            _ => unreachable!(),
        }
        assert!(root.find_mut("missing").is_none());
    }
}
