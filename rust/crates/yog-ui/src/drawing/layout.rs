use crate::text;
use crate::widget::{Action, SLOT_SIZE, Style, Units, Widget, WidgetKind};

mod flex;
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

use yog_gfx::Vec2;
pub use yog_gfx::graphics::Rect;

/// Width of the strip a [`scrollbar`](crate::widget::Style::scrollbar)
/// container reserves on its right edge.
pub const SCROLLBAR_W: f32 = 6.0;
/// Shortest a scrollbar knob gets.
pub const MIN_KNOB_H: f32 = 16.0;

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
        let room = Room::new(avail_w, avail_h, avail_w, avail_h);
        layout_widget(widget, &mut node, Vec2::ZERO, room, Depth::Full);
    }
    node
}

/// Sizes at or above this mean "unbounded": the axis is being measured for
/// content (a Row's main axis while its children are measured, ...).
const UNBOUNDED: f32 = f32::MAX;

/// Labels and buttons at least this wide don't wrap: the width is "no
/// real limit" rather than a constraint.
const NO_WRAP_W: f32 = 4096.0;
/// A text box's width along an unbounded axis.
const TEXT_BOX_W: f32 = 100.0;
/// What a text box adds to its line height: border and inner margin.
const TEXT_BOX_CHROME_H: f32 = 7.0;

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

/// What a widget is laid out in: the caps on its size (`UNBOUNDED`: no
/// cap, size to content) and the parent's content box its percent sizes
/// resolve against.
#[derive(Clone, Copy)]
struct Room {
    max_w: f32,
    max_h: f32,
    basis_w: f32,
    basis_h: f32,
}

impl Room {
    fn new(max_w: f32, max_h: f32, basis_w: f32, basis_h: f32) -> Room {
        Room {
            max_w,
            max_h,
            basis_w,
            basis_h,
        }
    }
}

/// Measure `w` (see [`Depth`]) in `room`.
fn measure(w: &Widget, node: &mut LayoutNode, room: Room) {
    layout_widget(w, node, Vec2::ZERO, room, Depth::Size);
}

/// Lays `w` out at `at` in `room`: its own size, then its children.
fn layout_widget(w: &Widget, node: &mut LayoutNode, at: Vec2, room: Room, depth: Depth) {
    let s = &w.style;
    let Room { max_w, max_h, .. } = room;
    let has_children = is_container(w);
    let explicit_w = resolve_size(s.w, room.basis_w).map(|v| v.min(max_w));
    let explicit_h = resolve_size(s.h, room.basis_h).map(|v| v.min(max_h));
    let (ww, hh) = if has_children {
        (explicit_w.unwrap_or(max_w), explicit_h.unwrap_or(max_h))
    } else {
        // Explicit width/height always wins over content sizing.
        let (cw, ch) = leaf_size(w, explicit_w.unwrap_or(max_w), room);
        (explicit_w.unwrap_or(cw), explicit_h.unwrap_or(ch))
    };

    node.rect = Rect {
        x: at.x,
        y: at.y,
        w: ww,
        h: hh,
    };
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

/// A leaf's size: its content (text, item slot, spacer) plus padding,
/// within the min size and the caps; `ww`: the width it has, the explicit
/// one if any (text wraps at it).
fn leaf_size(w: &Widget, ww: f32, room: Room) -> (f32, f32) {
    let s = &w.style;
    let Room { max_w, max_h, .. } = room;
    let (mut ww, mut hh) = (ww, max_h);
    let fit_w = |content: f32| (content + s.pad[1] + s.pad[3]).max(s.min_w).min(max_w);
    let fit_h = |content: f32| (content + s.pad[0] + s.pad[2]).max(s.min_h).min(max_h);
    match &w.kind {
        WidgetKind::Label(t) | WidgetKind::Button(t) => {
            // Wrap at the explicit width when there is one.
            let avail_w = (ww - s.pad[1] - s.pad[3]).max(0.0);
            // Wrap only when the width is a real constraint (not
            // "unlimited") and the widget doesn't opt out of wrapping.
            if avail_w < NO_WRAP_W && !s.no_wrap {
                ww = fit_w(avail_w);
                hh = fit_h(text::text_height_in(t, avail_w, s.font_scale, &s.font));
            } else {
                ww = fit_w(text::str_width_in(t, s.font_scale, &s.font));
                hh = fit_h(text::LINE_H * s.font_scale);
            }
        }
        WidgetKind::ItemSlot(_) | WidgetKind::InvSlot { .. } => {
            ww = fit_w(SLOT_SIZE);
            hh = fit_h(SLOT_SIZE);
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
        // Text box: the available width (`TEXT_BOX_W` when unbounded),
        // one line of text plus its border and padding.
        WidgetKind::TextBox(_) => {
            if !bounded(ww) {
                ww = TEXT_BOX_W;
            }
            ww = ww.max(s.min_w).min(max_w);
            hh = fit_h(text::LINE_H * s.font_scale + TEXT_BOX_CHROME_H);
        }
        WidgetKind::Image(image) => {
            let Vec2 { x: nw, y: nh } = crate::image::natural_size(image);
            ww = fit_w(nw);
            hh = fit_h(nh);
        }
    }
    (ww, hh)
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

/// `w`'s children inside `node.rect` (size and position already set): a
/// grid's in its tracks, a panel's flex. With `shrink`, auto axes then fit
/// the content.
fn layout_children(w: &Widget, node: &mut LayoutNode, shrink: Option<Shrink>, depth: Depth) {
    match &w.kind {
        WidgetKind::Grid(spec) => grid::layout(w, spec, node, shrink, depth),
        _ => flex::layout(w, node, shrink, depth),
    }
}

/// Measures `w` aligned (not stretched) across a `max_w` slot: at its
/// content width — text takes the width it needs instead of wrapping at
/// (and filling) the slot — unless that's wider than the slot (`room.max_w`).
fn measure_aligned(w: &Widget, node: &mut LayoutNode, room: Room) {
    if bounded(room.max_w) && w.style.w.is_auto() {
        measure(
            w,
            node,
            Room {
                max_w: UNBOUNDED,
                ..room
            },
        );
        if node.rect.w <= room.max_w {
            return;
        }
    }
    measure(w, node, room);
}

/// Margin `side` of a visible child (a collapsed one has none).
fn vis_margin(c: &Widget, side: usize) -> f32 {
    if c.visible { c.style.margin[side] } else { 0.0 }
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

    // (`ch` already includes the top padding: child y is relative to `y`.)
    scroll(node, s, ch + s.pad[2], &mut child_nodes);
    node.children = child_nodes;
}

/// Scrolling: `content_h` tall, the children (positioned from the
/// content's top) shifted up by the style's `scroll_y`, clamped.
fn scroll(node: &mut LayoutNode, s: &Style, content_h: f32, children: &mut [LayoutNode]) {
    node.content_h = content_h;
    node.scroll_y = s.scroll_y.clamp(0.0, node.max_scroll());
    if node.scroll_y > 0.0 {
        for c in children {
            c.translate_y(-node.scroll_y);
        }
    }
}

/// A container's content box inside `rect` (less padding and the
/// scrollbar's strip; an unbounded axis stays unbounded): its width,
/// height, and the strip's width.
fn content_box(s: &Style, rect: Rect) -> (f32, f32, f32) {
    let bar_w = if s.scrollbar { SCROLLBAR_W } else { 0.0 };
    let content_w = if bounded(rect.w) {
        (rect.w - s.pad[1] - s.pad[3] - bar_w).max(0.0)
    } else {
        UNBOUNDED
    };
    let content_h = if bounded(rect.h) {
        (rect.h - s.pad[0] - s.pad[2]).max(0.0)
    } else {
        UNBOUNDED
    };
    (content_w, content_h, bar_w)
}

/// Hit-test: find deepest clickable, enabled node at (mx, my).
/// Children only count where they're inside their parent's rect, so rows
/// scrolled out of a clipping container can't be clicked.
pub fn hit_test(node: &LayoutNode, mx: f32, my: f32) -> Option<&LayoutNode> {
    if !node.visible || !node.rect.contains(Vec2::new(mx, my)) {
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
        if !node.visible || !node.rect.contains(Vec2::new(mx, my)) {
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
    if !node.visible || !node.rect.contains(Vec2::new(mx, my)) {
        return None;
    }
    let mut path = Vec::new();
    let mut n = node;
    'outer: loop {
        for (i, child) in n.children.iter().enumerate().rev() {
            if child.visible && child.rect.contains(Vec2::new(mx, my)) {
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
mod tests;
