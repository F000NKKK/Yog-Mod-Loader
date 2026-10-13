use crate::text;
use crate::widget::{Action, Dock, HAlign, Units, VAlign, Widget, WidgetKind};

mod grid;

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
        (x1 > x0 && y1 > y0).then_some(Rect {
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
    /// The widget's typed click target (see `Widget::action`).
    pub action: Option<Action>,
    /// `false` = collapsed (see `Widget::visible`): a zero rect, no
    /// children, never hit.
    pub visible: bool,
    /// A grid's rows and columns as laid out (`None` for other widgets).
    pub grid: Option<Box<GridLines>>,
}

/// Where a grid's rows and columns ended up — for drawing grid lines and
/// designer adorners.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridLines {
    /// Each row's top (absolute, scrolled like the children) and height.
    pub rows: Vec<(f32, f32)>,
    /// Each column's left (absolute) and width.
    pub cols: Vec<(f32, f32)>,
    /// Track sizes from a measure along an unbounded axis — the content
    /// size of `*` tracks, kept as their floor when the grid is then
    /// arranged at (at least) that size. Empty otherwise.
    pub(crate) floor_rows: Vec<f32>,
    pub(crate) floor_cols: Vec<f32>,
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
        if let Some(g) = &mut self.grid {
            for r in &mut g.rows {
                r.0 += dy;
            }
        }
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
            action: None,
            visible: true,
            grid: None,
        }
    }
}

/// Compute layout starting at (0,0) with given available size.
/// Returns the root LayoutNode with absolute coordinates.
///
/// Sizing rules (CSS-flexbox-like, single pass per container):
/// - A `Units::Percent` size resolves exactly once, against the parent's
///   content box (its size minus padding and scrollbar strip) on the same
///   axis — in both a Row and a Column, at any depth. While a parent is
///   still being measured for content on an axis (an auto size along an
///   unbounded axis), a percent along that axis acts as `Auto` until the
///   parent's final size is known.
/// - `flex` (and `Dock::Fill`) grows a child into the parent's free main-axis
///   space; the grown size sticks and the child's own children lay out
///   against it. A growing child with an auto main size starts from its min
///   size instead of its content (like WPF star sizing), so it fills exactly
///   the free space — a `scroll_view` scrolls the rest, and nothing nested
///   inside can push the child's siblings out of the parent.
pub fn compute(widget: &Widget, avail_w: f32, avail_h: f32) -> LayoutNode {
    let mut node = new_node(widget);
    if widget.visible {
        layout_widget(
            widget,
            &mut node,
            0.0,
            0.0,
            avail_w,
            avail_h,
            avail_w,
            avail_h,
            Depth::Full,
        );
    }
    node
}

/// Sizes at or above this mean "unbounded": the axis is being measured for
/// content (a Row's main axis while its children are measured, ...).
const UNBOUNDED: f32 = f32::MAX;

fn bounded(v: f32) -> bool {
    v < UNBOUNDED / 2.0
}

/// `u` against `basis`; a percent of an unbounded basis is `None` (auto).
fn resolve_size(u: Units, basis: f32) -> Option<f32> {
    match u {
        Units::Percent(_) if !bounded(basis) => None,
        _ => u.resolve(basis),
    }
}

fn new_node(w: &Widget) -> LayoutNode {
    LayoutNode {
        id: w.id.clone(),
        on_click: w.on_click.clone(),
        enabled: w.enabled,
        focused: w.focused,
        action: w.action,
        visible: w.visible,
        ..Default::default()
    }
}

/// How much of a subtree a layout call settles.
#[derive(Clone, Copy, PartialEq)]
enum Depth {
    /// Only the widget's own size (and its children's rects, which that
    /// size depends on) — grandchildren are left stale. Every measure is
    /// followed by an [`arrange`], which lays the subtree out for real;
    /// recursing here too would redo each level twice (2^depth work).
    Size,
    /// The whole subtree, final.
    Full,
}

/// Measure `w` (see [`Depth`]) at `(x, y)`. `max_w`/`max_h` cap its size
/// (`UNBOUNDED` = no cap: size to content); `basis_w`/`basis_h` are the
/// parent's content box that percent sizes resolve against.
fn measure(
    w: &Widget,
    node: &mut LayoutNode,
    max_w: f32,
    max_h: f32,
    basis_w: f32,
    basis_h: f32,
) {
    layout_widget(
        w,
        node,
        0.0,
        0.0,
        max_w,
        max_h,
        basis_w,
        basis_h,
        Depth::Size,
    );
}

#[allow(clippy::too_many_arguments)]
fn layout_widget(
    w: &Widget,
    node: &mut LayoutNode,
    x: f32,
    y: f32,
    max_w: f32,
    max_h: f32,
    basis_w: f32,
    basis_h: f32,
    depth: Depth,
) {
    let s = &w.style;
    let has_children = is_container(w);
    let explicit_w = resolve_size(s.w, basis_w).map(|v| v.min(max_w));
    let explicit_h = resolve_size(s.h, basis_h).map(|v| v.min(max_h));

    // Determine own size
    let mut ww = explicit_w.unwrap_or(max_w);
    let mut hh = explicit_h.unwrap_or(max_h);

    if !has_children {
        // Leaf: size to content (text, item slot, spacer)
        match &w.kind {
            WidgetKind::Label(t) | WidgetKind::Button(t) => {
                // Wrap at the explicit width when there is one.
                let box_w = explicit_w.unwrap_or(max_w);
                let avail_w = (box_w - s.pad[1] - s.pad[3]).max(0.0);
                // Wrap only when the width is a real constraint (not
                // "unlimited") and the widget doesn't opt out of wrapping.
                if avail_w < 4096.0 && !s.no_wrap {
                    ww = (avail_w + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                    hh = (text::text_height_in(t, avail_w, s.font_scale, &s.font)
                        + s.pad[0]
                        + s.pad[2])
                        .max(s.min_h)
                        .min(max_h);
                } else {
                    let tw = text::str_width_in(t, s.font_scale, &s.font);
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
            WidgetKind::Spacer | WidgetKind::Surface(_) => {
                ww = s.min_w.max(1.0).min(max_w);
                hh = s.min_h.max(1.0).min(max_h);
            }
            // Panel with no children: the available space, or its min size
            // along an unbounded axis.
            WidgetKind::Panel(_) | WidgetKind::Grid(_) => {
                if !bounded(ww) {
                    ww = s.min_w;
                }
                if !bounded(hh) {
                    hh = s.min_h;
                }
            }
            // Text box: the available width (100 px when unbounded), one
            // line of text plus its border and padding.
            WidgetKind::TextBox(_) => {
                if !bounded(ww) {
                    ww = 100.0;
                }
                ww = ww.max(s.min_w).min(max_w);
                hh = (text::LINE_H * s.font_scale + 7.0 + s.pad[0] + s.pad[2])
                    .max(s.min_h)
                    .min(max_h);
            }
            WidgetKind::Image(image) => {
                let (nw, nh) = crate::image::natural_size(image);
                ww = (nw + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
                hh = (nh + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
            }
        }
        // Explicit width/height always wins over content sizing.
        if let Some(v) = explicit_w {
            ww = v;
        }
        if let Some(v) = explicit_h {
            hh = v;
        }
    }

    node.rect = Rect { x, y, w: ww, h: hh };
    node.grid = None;
    node.content_h = hh;
    node.scroll_y = 0.0;
    node.clip = s.clip;
    node.scrollbar = s.scrollbar;

    if has_children {
        let shrink = Shrink {
            w: explicit_w.is_none(),
            h: explicit_h.is_none(),
            max_w,
            max_h,
        };
        layout_children(w, node, Some(shrink), depth);
    }
}

/// Give `w` (already measured into `node`) its final rect and lay its
/// children out against that size — nothing is re-resolved or shrunk.
fn arrange(w: &Widget, node: &mut LayoutNode, x: f32, y: f32) {
    node.rect.x = x;
    node.rect.y = y;
    node.content_h = node.rect.h;
    if is_container(w) {
        layout_children(w, node, None, Depth::Full);
    }
}

/// Lays out children (a grid does even with none: its tracks size it).
fn is_container(w: &Widget) -> bool {
    !w.children.is_empty() || matches!(w.kind, WidgetKind::Grid(_))
}

/// Which axes of a container size to its content after its children are
/// laid out (the measure pass), and the caps on that.
struct Shrink {
    w: bool,
    h: bool,
    max_w: f32,
    max_h: f32,
}

/// Places a measured child at `(x, y)`: [`arrange`]s it, or with
/// [`Depth::Size`] only moves its rect.
fn place(child: &Widget, cn: &mut LayoutNode, x: f32, y: f32, depth: Depth) {
    match depth {
        Depth::Full => arrange(child, cn, x, y),
        Depth::Size => {
            cn.rect.x = x;
            cn.rect.y = y;
        }
    }
}

/// Flex layout of `w`'s children inside `node.rect` (size and position
/// already set). With `shrink`, auto axes then fit the content.
fn layout_children(w: &Widget, node: &mut LayoutNode, shrink: Option<Shrink>, depth: Depth) {
    if let WidgetKind::Grid(spec) = &w.kind {
        return grid::layout(w, spec, node, shrink, depth);
    }
    let s = &w.style;
    let Rect { x, y, w: ww, h: hh } = node.rect;

    let dir = if matches!(w.kind, WidgetKind::Panel(_)) && w.flex_dir == FlexDir::Row {
        FlexDir::Row
    } else {
        FlexDir::Column
    };
    let bar_w = if s.scrollbar { SCROLLBAR_W } else { 0.0 };
    let content_w = if bounded(ww) {
        (ww - s.pad[1] - s.pad[3] - bar_w).max(0.0)
    } else {
        UNBOUNDED
    };
    let content_h = if bounded(hh) {
        (hh - s.pad[0] - s.pad[2]).max(0.0)
    } else {
        UNBOUNDED
    };
    let (main_bounded, cross_bounded) = match dir {
        FlexDir::Row => (bounded(content_w), bounded(content_h)),
        FlexDir::Column => (bounded(content_h), bounded(content_w)),
    };
    // Measuring an auto-sized cross axis: its size isn't known yet (only a
    // cap), so nothing is centred/end-aligned until the arrange pass.
    let cross_shrinks = shrink.as_ref().is_some_and(|f| match dir {
        FlexDir::Row => f.h,
        FlexDir::Column => f.w,
    });
    // A child's own cross-axis placement (`halign` in a Column, `valign` in
    // a Row): `Some(None)` = stretch, `Some(Some(a))` = aligned, `None` =
    // the container's `align`.
    let self_place = |child: &Widget| -> Option<Option<Align>> {
        match dir {
            FlexDir::Column => child.style.halign.map(|a| match a {
                HAlign::Left => Some(Align::Start),
                HAlign::Center => Some(Align::Center),
                HAlign::Right => Some(Align::End),
                HAlign::Stretch => None,
            }),
            FlexDir::Row => child.style.valign.map(|a| match a {
                VAlign::Top => Some(Align::Start),
                VAlign::Center => Some(Align::Center),
                VAlign::Bottom => Some(Align::End),
                VAlign::Stretch => None,
            }),
        }
    };
    // Offset of a child `size` long (margins included) across `content`.
    let cross_offset = |child: &Widget, content: f32, size: f32| -> f32 {
        if !cross_bounded || cross_shrinks {
            return 0.0;
        }
        match self_place(child).flatten().unwrap_or(s.align) {
            Align::Center => (content - size) / 2.0,
            Align::End => content - size,
            Align::Start => 0.0,
        }
    };
    // Margins along the main axis (leading, total) and across it (leading, total).
    let main_m = |c: &Widget| -> (f32, f32) {
        let m = c.style.margin;
        match dir {
            FlexDir::Row => (m[3], m[1] + m[3]),
            FlexDir::Column => (m[0], m[0] + m[2]),
        }
    };
    let cross_m = |c: &Widget| -> (f32, f32) {
        let m = c.style.margin;
        match dir {
            FlexDir::Row => (m[0], m[0] + m[2]),
            FlexDir::Column => (m[3], m[1] + m[3]),
        }
    };

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
        child.visible
            && child.style.pos.is_none()
            && !matches!(
                (dir, child.style.dock),
                (FlexDir::Row, Dock::Right) | (FlexDir::Column, Dock::Bottom)
            )
    };
    let stretches = |child: &Widget| -> bool {
        let dock = child.style.dock;
        let auto_cross = match dir {
            FlexDir::Row => child.style.h.is_auto(),
            FlexDir::Column => child.style.w.is_auto(),
        };
        cross_bounded
            && (match dir {
                FlexDir::Row => matches!(dock, Dock::Fill | Dock::Left | Dock::Right),
                FlexDir::Column => matches!(dock, Dock::Fill | Dock::Top | Dock::Bottom),
            } || (auto_cross && self_place(child) == Some(None)))
    };

    // Measure children
    let mut child_nodes: Vec<LayoutNode> = Vec::with_capacity(w.children.len());
    let mut total_flex: f32 = 0.0;
    let mut used_main: f32 = 0.0;

    for child in &w.children {
        let mut cn = new_node(child);
        // Collapsed: a zero rect at the content box's corner, out of the flow.
        if !child.visible {
            cn.rect = Rect {
                x: x + s.pad[3],
                y: y + s.pad[0],
                w: 0.0,
                h: 0.0,
            };
            child_nodes.push(cn);
            continue;
        }
        let m = child.style.margin;
        // Absolutely positioned: measured against what's left of the content
        // box past its offset (percent sizes: against the whole content box),
        // placed after the flow passes.
        if let Some((px, py)) = child.style.pos {
            let mw = less(content_w, px + m[1] + m[3]);
            let mh = less(content_h, py + m[0] + m[2]);
            measure(child, &mut cn, mw, mh, content_w, content_h);
            child_nodes.push(cn);
            continue;
        }
        // Main axis unbounded (children size to content along it), except a
        // Dock::Fill child which may wrap text to the container.
        let (cmw, cmh) = match (dir, child.style.dock) {
            (_, Dock::Fill) => (content_w, content_h),
            (FlexDir::Row, _) => (UNBOUNDED, content_h),
            (FlexDir::Column, _) => (content_w, UNBOUNDED),
        };
        let (cmw, cmh) = (less(cmw, m[1] + m[3]), less(cmh, m[0] + m[2]));
        if dir == FlexDir::Column
            && child.style.dock != Dock::Fill
            && matches!(self_place(child), Some(Some(_)))
        {
            measure_aligned(child, &mut cn, cmw, cmh, content_w, content_h);
        } else {
            measure(child, &mut cn, cmw, cmh, content_w, content_h);
        }
        let ef = effective_flex(child);
        // A growing child with an auto main size starts from its min size,
        // not its content (WPF star sizing / CSS `flex: 1 1 0`): it gets
        // exactly its share of the free space, so a long list nested deep
        // inside can't push its siblings out of the parent.
        if ef > 0.0 && main_bounded && in_flow(child) {
            match dir {
                FlexDir::Row if child.style.w.is_auto() => cn.rect.w = child.style.min_w,
                FlexDir::Column if child.style.h.is_auto() => cn.rect.h = child.style.min_h,
                _ => {}
            }
        }
        // Flow and far-edge (Dock::Right / Dock::Bottom) children alike
        // take main-axis space.
        used_main += main_m(child).1
            + match dir {
                FlexDir::Row => cn.rect.w,
                FlexDir::Column => cn.rect.h,
            };
        if in_flow(child) {
            total_flex += ef;
        }
        child_nodes.push(cn);
    }
    let placed = w
        .children
        .iter()
        .filter(|c| c.visible && c.style.pos.is_none())
        .count();
    used_main += s.gap * (placed.saturating_sub(1) as f32);

    // Free main-axis space to grow into — none while measuring for content.
    let available = if main_bounded {
        (match dir {
            FlexDir::Row => content_w,
            FlexDir::Column => content_h,
        }) - used_main
    } else {
        0.0
    };
    let mut pos = match dir {
        FlexDir::Row => s.pad[3],
        FlexDir::Column => s.pad[0],
    };

    // --- Forward pass: position in-flow children (not Dock::Right / Dock::Bottom) ---
    for (i, child) in w.children.iter().enumerate() {
        if !in_flow(child) {
            continue;
        }
        let cn = &mut child_nodes[i];
        let ef = effective_flex(child);
        let grow = if ef > 0.0 && total_flex > 0.0 && available > 0.0 {
            available * ef / total_flex
        } else {
            0.0
        };
        let stretch = stretches(child);
        let (ml, mt) = main_m(child);
        let (cl, ct) = cross_m(child);
        let (cx, cy) = if dir == FlexDir::Row {
            cn.rect.w += grow;
            if stretch {
                cn.rect.h = (content_h - ct).max(0.0);
            }
            let p = (
                x + pos + ml,
                y + s.pad[0] + cl + cross_offset(child, content_h, cn.rect.h + ct),
            );
            pos += cn.rect.w + mt + s.gap;
            p
        } else {
            cn.rect.h += grow;
            if stretch {
                cn.rect.w = (content_w - ct).max(0.0);
            }
            let p = (
                x + s.pad[3] + cl + cross_offset(child, content_w, cn.rect.w + ct),
                y + pos + ml,
            );
            pos += cn.rect.h + mt + s.gap;
            p
        };
        place(child, cn, cx, cy, depth);
    }

    // --- Reverse pass: position Dock::Right / Dock::Bottom children from the far edge ---
    // While measuring for content there is no far edge: they line up right
    // after the flow, keeping their right-to-left (bottom-to-top) order.
    let mut rpos = if main_bounded {
        match dir {
            FlexDir::Row => x + s.pad[3] + content_w,
            FlexDir::Column => y + s.pad[0] + content_h,
        }
    } else {
        let docked: Vec<f32> = w
            .children
            .iter()
            .zip(&child_nodes)
            .filter(|(c, _)| c.visible && !in_flow(c) && c.style.pos.is_none())
            .map(|(c, n)| {
                main_m(c).1
                    + match dir {
                        FlexDir::Row => n.rect.w,
                        FlexDir::Column => n.rect.h,
                    }
            })
            .collect();
        let span: f32 =
            docked.iter().sum::<f32>() + s.gap * (docked.len().saturating_sub(1) as f32);
        (match dir {
            FlexDir::Row => x,
            FlexDir::Column => y,
        }) + pos
            + span
    };
    for (i, child) in w.children.iter().enumerate() {
        if !child.visible || in_flow(child) || child.style.pos.is_some() {
            continue;
        }
        let cn = &mut child_nodes[i];
        let stretch = stretches(child);
        let (ml, mt) = main_m(child);
        let (cl, ct) = cross_m(child);
        // The trailing margin first (it's on the far side).
        rpos -= mt - ml;
        let (cx, cy) = if dir == FlexDir::Row {
            if stretch {
                cn.rect.h = (content_h - ct).max(0.0);
            }
            rpos -= cn.rect.w;
            (
                rpos,
                y + s.pad[0] + cl + cross_offset(child, content_h, cn.rect.h + ct),
            )
        } else {
            if stretch {
                cn.rect.w = (content_w - ct).max(0.0);
            }
            rpos -= cn.rect.h;
            (
                x + s.pad[3] + cl + cross_offset(child, content_w, cn.rect.w + ct),
                rpos,
            )
        };
        rpos -= ml + s.gap;
        place(child, cn, cx, cy, depth);
    }

    // --- Absolutely positioned children (see `Style::pos`) ---
    for (i, child) in w.children.iter().enumerate() {
        let Some((px, py)) = child.style.pos else {
            continue;
        };
        if !child.visible {
            continue;
        }
        let m = child.style.margin;
        let cn = &mut child_nodes[i];
        place(
            child,
            cn,
            x + s.pad[3] + px + m[3],
            y + s.pad[0] + py + m[0],
            depth,
        );
    }

    finish(w, node, child_nodes, shrink, bar_w);
}

/// Measures `w` aligned (not stretched) across a `max_w` slot: at its
/// content width — text takes the width it needs instead of wrapping at
/// (and filling) the slot — unless that's wider than the slot.
fn measure_aligned(
    w: &Widget,
    node: &mut LayoutNode,
    max_w: f32,
    max_h: f32,
    basis_w: f32,
    basis_h: f32,
) {
    if bounded(max_w) && w.style.w.is_auto() {
        measure(w, node, UNBOUNDED, max_h, basis_w, basis_h);
        if node.rect.w <= max_w {
            return;
        }
    }
    measure(w, node, max_w, max_h, basis_w, basis_h);
}

/// Margin `side` of a visible child (a collapsed one has none).
fn vis_margin(c: &Widget, side: usize) -> f32 {
    if c.visible {
        c.style.margin[side]
    } else {
        0.0
    }
}

/// `v` less `by` (at least 0); unbounded stays unbounded.
fn less(v: f32, by: f32) -> f32 {
    if bounded(v) {
        (v - by).max(0.0)
    } else {
        UNBOUNDED
    }
}

/// The end of a container's layout: with `shrink`, auto axes fit the
/// children (margins included); then the content height and scrolling.
fn finish(
    w: &Widget,
    node: &mut LayoutNode,
    mut child_nodes: Vec<LayoutNode>,
    shrink: Option<Shrink>,
    bar_w: f32,
) {
    let s = &w.style;
    let Rect { x, y, .. } = node.rect;
    let extent = |f: &dyn Fn(&Widget, &LayoutNode) -> f32| -> f32 {
        w.children
            .iter()
            .zip(&child_nodes)
            .map(|(c, n)| f(c, n))
            .fold(0.0f32, f32::max)
    };
    let ch = extent(&|c, n| n.rect.y - y + n.rect.h + vis_margin(c, 2));
    if let Some(fit) = shrink {
        if fit.w {
            let cw = extent(&|c, n| n.rect.x - x + n.rect.w + vis_margin(c, 1));
            node.rect.w = (cw + s.pad[1] + bar_w).max(s.min_w).min(fit.max_w);
        }
        if fit.h {
            node.rect.h = (ch + s.pad[2]).max(s.min_h).min(fit.max_h);
        }
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
    if !node.visible || !node.rect.contains(mx, my) {
        return None;
    }
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test(child, mx, my) {
            return Some(hit);
        }
    }
    if node.is_clickable() {
        Some(node)
    } else {
        None
    }
}

/// The child-index path (as in `Widget::at_path`) of the node
/// [`hit_test`] finds at `(mx, my)`; `None` on a miss.
pub fn hit_path(node: &LayoutNode, mx: f32, my: f32) -> Option<Vec<u16>> {
    fn walk(node: &LayoutNode, mx: f32, my: f32, path: &mut Vec<u16>) -> bool {
        if !node.visible || !node.rect.contains(mx, my) {
            return false;
        }
        for (i, child) in node.children.iter().enumerate().rev() {
            path.push(i as u16);
            if walk(child, mx, my, path) {
                return true;
            }
            path.pop();
        }
        node.is_clickable()
    }
    let mut path = Vec::new();
    walk(node, mx, my, &mut path).then_some(path)
}

/// The path of the deepest visible node at `(mx, my)`, clickable or not
/// (for hover routing: which scroll view is under the cursor).
pub fn deepest_path(node: &LayoutNode, mx: f32, my: f32) -> Option<Vec<u16>> {
    if !node.visible || !node.rect.contains(mx, my) {
        return None;
    }
    let mut path = Vec::new();
    let mut n = node;
    'outer: loop {
        for (i, child) in n.children.iter().enumerate().rev() {
            if child.visible && child.rect.contains(mx, my) {
                path.push(i as u16);
                n = child;
                continue 'outer;
            }
        }
        return Some(path);
    }
}

/// The node at child-index `path` (`&[]` = `node`).
pub fn node_at<'a>(node: &'a LayoutNode, path: &[u16]) -> Option<&'a LayoutNode> {
    let mut n = node;
    for &i in path {
        n = n.children.get(i as usize)?;
    }
    Some(n)
}

/// [`node_at`], mutably.
pub fn node_at_mut<'a>(node: &'a mut LayoutNode, path: &[u16]) -> Option<&'a mut LayoutNode> {
    let mut n = node;
    for &i in path {
        n = n.children.get_mut(i as usize)?;
    }
    Some(n)
}

impl LayoutNode {
    /// Clicking it does something: it has an action or `on_click`, is
    /// enabled and visible.
    pub fn is_clickable(&self) -> bool {
        self.visible && self.enabled && (self.action.is_some() || self.on_click.is_some())
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

    fn pct(v: f32) -> Units {
        Units::Percent(v)
    }

    fn r(n: &LayoutNode) -> (f32, f32, f32, f32) {
        (n.rect.x, n.rect.y, n.rect.w, n.rect.h)
    }

    #[test]
    fn percent_width_in_a_row_resolves_once_against_the_content_box() {
        let root = widget::panel(FlexDir::Row)
            .w(400.0)
            .h(100.0)
            .padding(5.0, 10.0, 5.0, 10.0)
            .child(
                widget::panel(FlexDir::Column)
                    .w(pct(50.0))
                    .child(widget::spacer().min_w(10.0).min_h(10.0)),
            )
            .child(widget::spacer().w(pct(25.0)).h(pct(50.0)));
        let node = compute(&root, 800.0, 600.0);
        // Content box: 380 x 90.
        assert_eq!(r(&node.children[0]).2, 190.0);
        assert_eq!(r(&node.children[1]), (10.0 + 190.0 + 2.0, 5.0, 95.0, 45.0));
    }

    #[test]
    fn percent_sizes_in_a_column() {
        let root = widget::panel(FlexDir::Column)
            .w(200.0)
            .h(300.0)
            .padding(10.0, 0.0, 10.0, 0.0)
            .gap(0.0)
            .child(
                widget::panel(FlexDir::Row)
                    .w(pct(50.0))
                    .h(pct(50.0))
                    .child(widget::spacer()),
            )
            .child(widget::spacer().w(pct(100.0)).h(pct(10.0)));
        let node = compute(&root, 800.0, 600.0);
        assert_eq!(r(&node.children[0]), (0.0, 10.0, 100.0, 140.0));
        assert_eq!(r(&node.children[1]), (0.0, 150.0, 200.0, 28.0));
    }

    #[test]
    fn nested_percents_compound_per_level_only() {
        let root = widget::panel(FlexDir::Row).w(400.0).h(200.0).child(
            widget::panel(FlexDir::Column)
                .w(pct(50.0))
                .h(pct(100.0))
                .child(
                    widget::panel(FlexDir::Row)
                        .w(pct(50.0))
                        .h(pct(50.0))
                        .child(widget::spacer().w(pct(50.0)).h(pct(50.0))),
                ),
        );
        let node = compute(&root, 400.0, 200.0);
        let a = &node.children[0];
        let b = &a.children[0];
        let c = &b.children[0];
        assert_eq!(r(a), (0.0, 0.0, 200.0, 200.0));
        assert_eq!(r(b), (0.0, 0.0, 100.0, 100.0));
        assert_eq!(r(c), (0.0, 0.0, 50.0, 50.0));
    }

    #[test]
    fn percent_resolves_against_the_scrollbar_free_box() {
        let root = widget::scroll_view()
            .w(106.0)
            .h(50.0)
            .child(widget::spacer().w(pct(50.0)).h(10.0));
        let node = compute(&root, 400.0, 400.0);
        assert_eq!(node.children[0].rect.w, 50.0);
    }

    #[test]
    fn grow_shares_free_space_with_percent_siblings() {
        let root = widget::panel(FlexDir::Row)
            .w(300.0)
            .h(50.0)
            .gap(4.0)
            .child(widget::spacer().w(pct(50.0)))
            .child(
                widget::panel(FlexDir::Column)
                    .flex(1.0)
                    .child(widget::spacer()),
            )
            .child(widget::spacer().w(pct(10.0)));
        let node = compute(&root, 300.0, 50.0);
        // 300 - 150 - 30 - 2 * 4 = 112 for the grown panel.
        assert_eq!(r(&node.children[0]).2, 150.0);
        assert_eq!(r(&node.children[1]), (154.0, 0.0, 112.0, 4.0));
        assert_eq!(r(&node.children[2]), (270.0, 0.0, 30.0, 4.0));
    }

    #[test]
    fn grown_panel_keeps_its_size_and_lays_children_out_against_it() {
        let root = widget::panel(FlexDir::Column)
            .w(200.0)
            .h(200.0)
            .gap(0.0)
            .child(widget::spacer().min_h(20.0))
            .child(
                widget::panel(FlexDir::Row)
                    .flex(1.0)
                    .w(pct(100.0))
                    .gap(0.0)
                    .child(widget::spacer().w(pct(25.0)).h(pct(50.0)))
                    .child(
                        widget::panel(FlexDir::Column)
                            .flex(1.0)
                            .child(widget::spacer()),
                    )
                    .child(widget::spacer().dock(Dock::Right).min_w(10.0)),
            );
        let node = compute(&root, 400.0, 400.0);
        let grown = &node.children[1];
        assert_eq!(r(grown), (0.0, 20.0, 200.0, 180.0));
        assert_eq!(r(&grown.children[0]), (0.0, 20.0, 50.0, 90.0));
        // The inner flex child grows into what's left of the grown width...
        assert_eq!(r(&grown.children[1]).2, 200.0 - 50.0 - 10.0);
        // ... and the right-docked one hugs the grown panel's far edge.
        assert_eq!(r(&grown.children[2]), (190.0, 20.0, 10.0, 180.0));
    }

    #[test]
    fn long_list_deep_inside_a_grown_row_does_not_push_siblings_out() {
        // The Modlist screen: header, a flex row holding a 100%-high column
        // with a growing scroll view of many rows, and a footer.
        let mut list = widget::scroll_view().flex(1.0).w(pct(100.0));
        for _ in 0..60 {
            list = list.child(widget::spacer().h(25.0));
        }
        let root = widget::panel(FlexDir::Column)
            .w(400.0)
            .h(252.0)
            .gap(0.0)
            .child(widget::spacer().h(20.0))
            .child(
                widget::panel(FlexDir::Row).flex(1.0).w(pct(100.0)).child(
                    widget::panel(FlexDir::Column)
                        .w(pct(50.0))
                        .h(pct(100.0))
                        .child(widget::spacer().h(16.0))
                        .child(list),
                ),
            )
            .child(widget::spacer().h(26.0));
        let node = compute(&root, 400.0, 252.0);
        assert_eq!(r(&node.children[1]), (0.0, 20.0, 400.0, 206.0));
        let footer = r(&node.children[2]);
        assert_eq!((footer.1, footer.3), (226.0, 26.0));
        let column = &node.children[1].children[0];
        assert_eq!(column.rect.h, 206.0);
        let list = &column.children[1];
        assert!(list.rect.y + list.rect.h <= 226.0, "{:?}", list.rect);
        assert!(list.content_h > list.rect.h);
    }

    #[test]
    fn grown_column_panel_fills_its_parent_and_children_grow_too() {
        let root = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(100.0)
            .gap(0.0)
            .child(
                widget::panel(FlexDir::Column)
                    .flex(1.0)
                    .gap(0.0)
                    .child(widget::spacer().flex(1.0)),
            );
        let node = compute(&root, 100.0, 100.0);
        assert_eq!(node.children[0].rect.h, 100.0);
        assert_eq!(node.children[0].children[0].rect.h, 100.0);
    }

    #[test]
    fn empty_auto_panel_in_a_row_takes_its_min_width() {
        let root = widget::panel(FlexDir::Row)
            .w(100.0)
            .h(20.0)
            .child(widget::panel(FlexDir::Column).min_w(7.0))
            .child(widget::spacer());
        let node = compute(&root, 100.0, 20.0);
        assert_eq!(node.children[0].rect.w, 7.0);
        assert_eq!(node.children[1].rect.x, 9.0);
    }

    #[test]
    fn padding_counts_once_in_auto_size() {
        let root = widget::panel(FlexDir::Column)
            .padding(3.0, 4.0, 5.0, 6.0)
            .child(widget::spacer().min_w(10.0).min_h(10.0));
        let node = compute(&root, 300.0, 300.0);
        assert_eq!((node.rect.w, node.rect.h), (20.0, 18.0));
        assert_eq!(
            (node.children[0].rect.x, node.children[0].rect.y),
            (6.0, 3.0)
        );
    }

    #[test]
    fn auto_row_in_a_column_measures_to_content() {
        // Align::Center on an auto-height row must not centre in "infinity".
        let root = widget::panel(FlexDir::Column).w(200.0).h(200.0).child(
            widget::panel(FlexDir::Row)
                .align(Align::Center)
                .gap(0.0)
                .child(widget::spacer().min_w(10.0).min_h(10.0))
                .child(widget::spacer().min_w(10.0).min_h(20.0))
                .child(widget::spacer().min_w(5.0).dock(Dock::Right)),
        );
        let node = compute(&root, 200.0, 200.0);
        let row = &node.children[0];
        assert_eq!(row.rect.h, 20.0);
        assert_eq!(row.children[0].rect.y, 5.0);
        assert_eq!(row.children[1].rect.y, 0.0);
    }

    #[test]
    fn positioned_auto_row_measures_to_content() {
        // Measured against the rest of the screen (a bounded cap), a centred
        // auto-height row must not centre in that cap.
        let pill = widget::panel(FlexDir::Row)
            .padding(1.0, 1.0, 1.0, 1.0)
            .child(
                widget::panel(FlexDir::Row)
                    .align(Align::Center)
                    .child(widget::spacer().min_w(10.0).min_h(9.0)),
            );
        let root = widget::panel(FlexDir::Column)
            .w(500.0)
            .h(500.0)
            .child(pill.at(3.0, 4.0));
        let node = compute(&root, 500.0, 500.0);
        let r = node.children[0].rect;
        assert_eq!((r.x, r.y, r.w, r.h), (3.0, 4.0, 12.0, 11.0));
        assert_eq!(node.children[0].children[0].children[0].rect.y, 5.0);
    }

    #[test]
    fn row_with_right_dock_measured_on_unbounded_axis() {
        // An auto-width row inside a row: its Dock::Right child lines up after
        // the flow instead of at "infinity".
        let root = widget::panel(FlexDir::Row).w(300.0).h(20.0).child(
            widget::panel(FlexDir::Row)
                .gap(1.0)
                .child(widget::spacer().min_w(10.0))
                .child(widget::spacer().min_w(5.0).dock(Dock::Right))
                .child(widget::spacer().min_w(7.0).dock(Dock::Right)),
        );
        let node = compute(&root, 300.0, 20.0);
        let inner = &node.children[0];
        assert_eq!(inner.rect.w, 10.0 + 1.0 + 7.0 + 1.0 + 5.0);
        assert_eq!(inner.children[1].rect.x, 19.0);
        assert_eq!(inner.children[2].rect.x, 11.0);
    }

    #[test]
    fn align_center_and_end_in_both_directions() {
        let col = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(50.0)
            .align(Align::Center)
            .child(widget::spacer().min_w(20.0).min_h(5.0));
        assert_eq!(compute(&col, 100.0, 50.0).children[0].rect.x, 40.0);
        let row = widget::panel(FlexDir::Row)
            .w(100.0)
            .h(50.0)
            .padding(10.0, 0.0, 0.0, 0.0)
            .align(Align::End)
            .child(widget::spacer().min_w(20.0).h(pct(20.0)));
        // Content height 40, child 8 tall, bottom-aligned.
        assert_eq!(compute(&row, 100.0, 50.0).children[0].rect.y, 10.0 + 32.0);
    }

    #[test]
    fn positioned_children_with_percent_sizes() {
        let root = widget::panel(FlexDir::Column)
            .w(200.0)
            .h(100.0)
            .padding(10.0, 10.0, 10.0, 10.0)
            .child(widget::spacer().w(pct(50.0)).h(pct(50.0)).at(20.0, 10.0))
            .child(
                widget::panel(FlexDir::Row)
                    .w(pct(100.0))
                    .h(pct(25.0))
                    .at(0.0, 60.0)
                    .gap(0.0)
                    .child(widget::spacer().w(pct(50.0)))
                    .child(widget::spacer().flex(1.0)),
            );
        let node = compute(&root, 400.0, 400.0);
        // Content box 180 x 80.
        assert_eq!(r(&node.children[0]), (30.0, 20.0, 90.0, 40.0));
        // 100% of 180 but capped to what's left past x=0, i.e. 180; 25% of 80
        // = 20 but capped to 80 - 60.
        let p = &node.children[1];
        assert_eq!(r(p), (10.0, 70.0, 180.0, 20.0));
        assert_eq!(r(&p.children[0]), (10.0, 70.0, 90.0, 4.0));
        assert_eq!(r(&p.children[1]), (100.0, 70.0, 90.0, 4.0));
    }

    #[test]
    fn growing_scroll_view_fills_free_space_and_scrolls_the_rest() {
        let list = widget::scroll_view()
            .id("list")
            .flex(1.0)
            .w(pct(100.0))
            .gap(0.0)
            .padding(2.0, 2.0, 2.0, 2.0)
            .scroll_y(1000.0)
            .children((0..10).map(|_| widget::spacer().w(pct(100.0)).min_h(20.0)));
        let root = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(120.0)
            .gap(0.0)
            .child(widget::spacer().min_h(20.0))
            .child(list);
        let node = compute(&root, 100.0, 120.0);
        let v = find_node(&node, "list").unwrap();
        assert_eq!(r(v), (0.0, 20.0, 100.0, 100.0));
        assert_eq!(v.content_h, 204.0);
        assert_eq!(v.scroll_y, 104.0);
        // Rows span the scrollbar-free content width.
        assert_eq!(v.children[0].rect.w, 100.0 - 4.0 - SCROLLBAR_W);
        // The last row's bottom sits on the view's inner bottom edge.
        let last = v.children[9].rect;
        assert_eq!(last.y + last.h, 20.0 + 100.0 - 2.0);
        assert!(v.scrollbar_rects().is_some());
    }

    #[test]
    fn label_wraps_at_its_explicit_width() {
        let root = widget::panel(FlexDir::Column)
            .w(300.0)
            .child(widget::label("aaaa bbbb cccc").w(40.0));
        let node = compute(&root, 300.0, 300.0);
        let l = node.children[0].rect;
        assert_eq!(l.w, 40.0);
        assert!(l.h > text::LINE_H, "wrapped onto several lines: {l:?}");
    }

    #[test]
    fn percent_caps_at_the_cross_axis_and_ignores_unbounded_basis() {
        // 150% across a column caps at the content width.
        let root = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(100.0)
            .child(widget::spacer().w(pct(150.0)));
        assert_eq!(compute(&root, 100.0, 100.0).children[0].rect.w, 100.0);
        // An auto-width row measured inside a row resolves its children's
        // percents against its final (content) width.
        let root = widget::panel(FlexDir::Row).w(200.0).h(20.0).child(
            widget::panel(FlexDir::Row)
                .gap(0.0)
                .child(widget::spacer().min_w(40.0))
                .child(widget::spacer().w(pct(50.0))),
        );
        let node = compute(&root, 200.0, 20.0);
        let inner = &node.children[0];
        assert_eq!(inner.rect.w, 44.0);
        assert_eq!(inner.children[1].rect.w, 22.0);
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
