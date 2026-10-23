//! Flex layout of a panel's children inside its rect: each measured, then
//! placed — the flow forward along the main axis (growing into the free
//! space by `flex`), the far-docked (`Dock::Right` / `Dock::Bottom`) back
//! from the far edge, the absolutely positioned at their offsets.

use super::{
    Align, Depth, FlexDir, LayoutNode, Rect, Room, Shrink, UNBOUNDED, bounded, content_box, finish,
    less, measure, measure_aligned, new_node, place,
};
use crate::widget::{Dock, HAlign, Style, Units, VAlign, Widget, WidgetKind};

/// A panel being laid out: its direction, where its content box is, and
/// which of its axes are bounded.
struct Flex<'a> {
    w: &'a Widget,
    s: &'a Style,
    dir: FlexDir,
    x: f32,
    y: f32,
    content_w: f32,
    content_h: f32,
    main_bounded: bool,
    cross_bounded: bool,
    /// Measuring an auto-sized cross axis: its size isn't known yet (only
    /// a cap), so nothing is centred/end-aligned until the arrange pass.
    cross_shrinks: bool,
    depth: Depth,
}

/// What measuring the children found: their nodes, the flow's total flex,
/// and the main-axis space they take.
struct Measured {
    nodes: Vec<LayoutNode>,
    total_flex: f32,
    used_main: f32,
}

/// Flex layout of `w`'s children inside `node.rect` (size and position
/// already set). With `shrink`, auto axes then fit the content.
pub(super) fn layout(w: &Widget, node: &mut LayoutNode, shrink: Option<Shrink>, depth: Depth) {
    let s = &w.style;
    let Rect { x, y, .. } = node.rect;
    let dir = if matches!(w.kind, WidgetKind::Panel(_)) && w.flex_dir == FlexDir::Row {
        FlexDir::Row
    } else {
        FlexDir::Column
    };
    let (content_w, content_h, bar_w) = content_box(s, node.rect);
    let (main_bounded, cross_bounded) = match dir {
        FlexDir::Row => (bounded(content_w), bounded(content_h)),
        FlexDir::Column => (bounded(content_h), bounded(content_w)),
    };
    let cross_shrinks = shrink.as_ref().is_some_and(|f| match dir {
        FlexDir::Row => f.h,
        FlexDir::Column => f.w,
    });
    let flex = Flex {
        w,
        s,
        dir,
        x,
        y,
        content_w,
        content_h,
        main_bounded,
        cross_bounded,
        cross_shrinks,
        depth,
    };
    let Measured {
        mut nodes,
        total_flex,
        used_main,
    } = flex.measure_children();
    let pos = flex.place_flow(&mut nodes, total_flex, used_main);
    flex.place_far(&mut nodes, pos);
    flex.place_positioned(&mut nodes);
    finish(w, node, nodes, shrink, bar_w);
}

impl Flex<'_> {
    /// A child's own cross-axis placement (`halign` in a Column, `valign`
    /// in a Row): `Some(None)` = stretch, `Some(Some(a))` = aligned,
    /// `None` = the container's `align`.
    fn self_place(&self, child: &Widget) -> Option<Option<Align>> {
        match self.dir {
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
    }

    /// Offset of a child `size` long (margins included) across `content`.
    fn cross_offset(&self, child: &Widget, content: f32, size: f32) -> f32 {
        if !self.cross_bounded || self.cross_shrinks {
            return 0.0;
        }
        match self.self_place(child).flatten().unwrap_or(self.s.align) {
            Align::Center => (content - size) / 2.0,
            Align::End => content - size,
            Align::Start => 0.0,
        }
    }

    /// Margins along the main axis: (leading, total).
    fn main_margin(&self, c: &Widget) -> (f32, f32) {
        let m = c.style.margin;
        match self.dir {
            FlexDir::Row => (m[3], m[1] + m[3]),
            FlexDir::Column => (m[0], m[0] + m[2]),
        }
    }

    /// Margins across the main axis: (leading, total).
    fn cross_margin(&self, c: &Widget) -> (f32, f32) {
        let m = c.style.margin;
        match self.dir {
            FlexDir::Row => (m[0], m[0] + m[2]),
            FlexDir::Column => (m[3], m[1] + m[3]),
        }
    }

    /// A node's length along the main axis.
    fn main_len(&self, n: &LayoutNode) -> f32 {
        match self.dir {
            FlexDir::Row => n.rect.w,
            FlexDir::Column => n.rect.h,
        }
    }

    /// Its flex factor (`Dock::Fill` implies at least 1).
    fn effective_flex(child: &Widget) -> f32 {
        if child.style.dock == Dock::Fill {
            child.style.flex.max(1.0)
        } else {
            child.style.flex
        }
    }

    /// Whether it takes main-axis space in the forward pass (not hidden,
    /// placed, or docked at the far edge).
    fn in_flow(&self, child: &Widget) -> bool {
        child.visible
            && child.style.pos.is_none()
            && !matches!(
                (self.dir, child.style.dock),
                (FlexDir::Row, Dock::Right) | (FlexDir::Column, Dock::Bottom)
            )
    }

    /// Whether it stretches across the content box.
    fn stretches(&self, child: &Widget) -> bool {
        let dock = child.style.dock;
        let auto_cross = match self.dir {
            FlexDir::Row => child.style.h.is_auto(),
            FlexDir::Column => child.style.w.is_auto(),
        };
        let docked = match self.dir {
            FlexDir::Row => matches!(dock, Dock::Fill | Dock::Left | Dock::Right),
            FlexDir::Column => matches!(dock, Dock::Fill | Dock::Top | Dock::Bottom),
        };
        self.cross_bounded && (docked || (auto_cross && self.self_place(child) == Some(None)))
    }

    /// Every child measured: collapsed ones a zero rect at the content
    /// box's corner, positioned ones in what's left past their offset,
    /// the rest along an unbounded main axis.
    fn measure_children(&self) -> Measured {
        let (s, w) = (self.s, self.w);
        let mut nodes: Vec<LayoutNode> = Vec::with_capacity(w.children.len());
        let mut total_flex: f32 = 0.0;
        let mut used_main: f32 = 0.0;
        for child in &w.children {
            let mut cn = new_node(child);
            if !child.visible {
                cn.rect = Rect {
                    x: self.x + s.pad[3],
                    y: self.y + s.pad[0],
                    w: 0.0,
                    h: 0.0,
                };
            } else if let Some(pos) = child.style.pos {
                self.measure_positioned(child, &mut cn, pos);
            } else {
                self.measure_flowing(child, &mut cn);
                // Flow and far-edge children alike take main-axis space.
                used_main += self.main_margin(child).1 + self.main_len(&cn);
                if self.in_flow(child) {
                    total_flex += Self::effective_flex(child);
                }
            }
            nodes.push(cn);
        }
        let placed = w
            .children
            .iter()
            .filter(|c| c.visible && c.style.pos.is_none())
            .count();
        used_main += s.gap * (placed.saturating_sub(1) as f32);
        Measured {
            nodes,
            total_flex,
            used_main,
        }
    }

    /// An absolutely positioned child: measured against what's left of the
    /// content box past its offset (percent sizes: against the whole
    /// content box). A size in pixels is kept whatever fits — a screen
    /// bigger than the window keeps its proportions and is cut at the
    /// window's edges, as the game's are, not squeezed.
    fn measure_positioned(&self, child: &Widget, cn: &mut LayoutNode, (px, py): (f32, f32)) {
        let m = child.style.margin;
        let fixed = |u: Units| matches!(u, Units::Px(_));
        let mw = if fixed(child.style.w) {
            UNBOUNDED
        } else {
            less(self.content_w, px + m[1] + m[3])
        };
        let mh = if fixed(child.style.h) {
            UNBOUNDED
        } else {
            less(self.content_h, py + m[0] + m[2])
        };
        measure(child, cn, Room::new(mw, mh, self.content_w, self.content_h));
    }

    /// A child in the flow (or docked at the far edge): its main axis
    /// unbounded (it sizes to content along it), except a `Dock::Fill`
    /// child's, which may wrap text to the container.
    fn measure_flowing(&self, child: &Widget, cn: &mut LayoutNode) {
        let m = child.style.margin;
        let (cmw, cmh) = match (self.dir, child.style.dock) {
            (_, Dock::Fill) => (self.content_w, self.content_h),
            (FlexDir::Row, _) => (UNBOUNDED, self.content_h),
            (FlexDir::Column, _) => (self.content_w, UNBOUNDED),
        };
        let room = Room::new(
            less(cmw, m[1] + m[3]),
            less(cmh, m[0] + m[2]),
            self.content_w,
            self.content_h,
        );
        if self.dir == FlexDir::Column
            && child.style.dock != Dock::Fill
            && matches!(self.self_place(child), Some(Some(_)))
        {
            measure_aligned(child, cn, room);
        } else {
            measure(child, cn, room);
        }
        // A growing child with an auto main size starts from its min size,
        // not its content (WPF star sizing / CSS `flex: 1 1 0`): it gets
        // exactly its share of the free space, so a long list nested deep
        // inside can't push its siblings out of the parent.
        if Self::effective_flex(child) > 0.0 && self.main_bounded && self.in_flow(child) {
            match self.dir {
                FlexDir::Row if child.style.w.is_auto() => cn.rect.w = child.style.min_w,
                FlexDir::Column if child.style.h.is_auto() => cn.rect.h = child.style.min_h,
                _ => {}
            }
        }
    }

    /// The child stretched across the content box if it stretches, and
    /// where it goes across: (leading margin + offset).
    fn across(&self, child: &Widget, cn: &mut LayoutNode) -> f32 {
        let (cl, ct) = self.cross_margin(child);
        let stretch = self.stretches(child);
        match self.dir {
            FlexDir::Row => {
                if stretch {
                    cn.rect.h = (self.content_h - ct).max(0.0);
                }
                self.y
                    + self.s.pad[0]
                    + cl
                    + self.cross_offset(child, self.content_h, cn.rect.h + ct)
            }
            FlexDir::Column => {
                if stretch {
                    cn.rect.w = (self.content_w - ct).max(0.0);
                }
                self.x
                    + self.s.pad[3]
                    + cl
                    + self.cross_offset(child, self.content_w, cn.rect.w + ct)
            }
        }
    }

    /// `(main, cross)` as `(x, y)`.
    fn point(&self, main: f32, cross: f32) -> (f32, f32) {
        match self.dir {
            FlexDir::Row => (main, cross),
            FlexDir::Column => (cross, main),
        }
    }

    /// The forward pass: the flow (not far-docked) in order, growing into
    /// the free main-axis space — none while measuring for content.
    /// Returns where the flow ends, from the content box's start.
    fn place_flow(&self, nodes: &mut [LayoutNode], total_flex: f32, used_main: f32) -> f32 {
        let s = self.s;
        let (origin, content, mut pos) = match self.dir {
            FlexDir::Row => (self.x, self.content_w, s.pad[3]),
            FlexDir::Column => (self.y, self.content_h, s.pad[0]),
        };
        let available = if self.main_bounded {
            content - used_main
        } else {
            0.0
        };
        for (child, cn) in self.w.children.iter().zip(nodes.iter_mut()) {
            if !self.in_flow(child) {
                continue;
            }
            let ef = Self::effective_flex(child);
            let grow = if ef > 0.0 && total_flex > 0.0 && available > 0.0 {
                available * ef / total_flex
            } else {
                0.0
            };
            match self.dir {
                FlexDir::Row => cn.rect.w += grow,
                FlexDir::Column => cn.rect.h += grow,
            }
            let (ml, mt) = self.main_margin(child);
            let cross = self.across(child, cn);
            let (cx, cy) = self.point(origin + pos + ml, cross);
            pos += self.main_len(cn) + mt + s.gap;
            place(child, cn, cx, cy, self.depth);
        }
        pos
    }

    /// The reverse pass: `Dock::Right` / `Dock::Bottom` children from the
    /// far edge. While measuring for content there is no far edge: they
    /// line up right after the flow (ending at `pos`), keeping their
    /// right-to-left (bottom-to-top) order.
    fn place_far(&self, nodes: &mut [LayoutNode], pos: f32) {
        let s = self.s;
        let far = |c: &Widget| c.visible && !self.in_flow(c) && c.style.pos.is_none();
        let (origin, start, content) = match self.dir {
            FlexDir::Row => (self.x, self.x + s.pad[3], self.content_w),
            FlexDir::Column => (self.y, self.y + s.pad[0], self.content_h),
        };
        let mut rpos = if self.main_bounded {
            start + content
        } else {
            let docked: Vec<f32> = self
                .w
                .children
                .iter()
                .zip(nodes.iter())
                .filter(|(c, _)| far(c))
                .map(|(c, n)| self.main_margin(c).1 + self.main_len(n))
                .collect();
            let span = docked.iter().sum::<f32>() + s.gap * (docked.len().saturating_sub(1) as f32);
            origin + pos + span
        };
        for (child, cn) in self.w.children.iter().zip(nodes.iter_mut()) {
            if !far(child) {
                continue;
            }
            let (ml, mt) = self.main_margin(child);
            // The trailing margin first (it's on the far side).
            rpos -= mt - ml;
            let cross = self.across(child, cn);
            rpos -= self.main_len(cn);
            let (cx, cy) = self.point(rpos, cross);
            rpos -= ml + s.gap;
            place(child, cn, cx, cy, self.depth);
        }
    }

    /// Absolutely positioned children (see `Style::pos`), at their offsets
    /// from the content box.
    fn place_positioned(&self, nodes: &mut [LayoutNode]) {
        let s = self.s;
        for (child, cn) in self.w.children.iter().zip(nodes.iter_mut()) {
            let Some((px, py)) = child.style.pos else {
                continue;
            };
            if !child.visible {
                continue;
            }
            let m = child.style.margin;
            place(
                child,
                cn,
                self.x + s.pad[3] + px + m[3],
                self.y + s.pad[0] + py + m[0],
                self.depth,
            );
        }
    }
}
