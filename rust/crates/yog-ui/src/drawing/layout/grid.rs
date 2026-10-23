//! WPF-style grid layout: see [`crate::widget::grid`].
//!
//! Each axis is sized on its own, like WPF's `Grid`:
//! 1. pixel tracks take their size; `Auto` tracks (and `*` tracks along an
//!    unbounded axis, which act as `Auto`) start at their min;
//! 2. children whose span has an auto track and no `*` track (along a
//!    bounded axis) are measured and grow the auto tracks — single-span
//!    children first, then wider spans, which only add what's missing,
//!    shared equally by the span's auto tracks;
//! 3. along a bounded axis the `*` tracks share what's left in proportion
//!    to their weights, honouring min/max (a track clamped to a limit drops
//!    out and the rest is shared again).
//!
//! Columns are sized first, so rows measure their children at their final
//! column widths (wrapped text gets its real height). Each child then gets
//! the rect of its cell(s) less its margin: it fills it (`Stretch`, the
//! default, unless it has an explicit size) or sits at its
//! `halign`/`valign` with its own measured size, capped to the cell.

use super::{
    Depth, GridLines, LayoutNode, Rect, Room, Shrink, UNBOUNDED, bounded, content_box, less,
    measure, measure_aligned, new_node, place, scroll,
};
use crate::widget::{GridLength, GridSpec, HAlign, Style, Track, VAlign, Widget};

/// The rows / columns a child spans (`start..end`).
#[derive(Clone, Copy)]
struct Span {
    r0: usize,
    r1: usize,
    c0: usize,
    c1: usize,
}

const ONE_STAR: [Track; 1] = [Track::star(1.0)];

/// A track sized to content: `Auto`, or `*` along an unbounded axis.
fn auto_like(t: &Track, flex: bool) -> bool {
    match t.size {
        GridLength::Auto => true,
        GridLength::Star(_) => !flex,
        GridLength::Px(_) => false,
    }
}

fn is_star(t: &Track, flex: bool) -> bool {
    flex && matches!(t.size, GridLength::Star(_))
}

/// `v` within the track's limits (min wins over max, like WPF).
fn clamp(t: &Track, v: f32) -> f32 {
    v.min(t.max).max(t.min)
}

/// The space `start..end` covers, spacing between them included.
fn extent(sizes: &[f32], spacing: f32, start: usize, end: usize) -> f32 {
    sizes[start..end].iter().sum::<f32>() + spacing * (end - start).saturating_sub(1) as f32
}

/// A child's size along an axis when it's measured for auto tracks: the
/// space of its span when that's all pixels, the whole axis (a cap) when it
/// has a `*` track, else unbounded.
fn measure_cap(tracks: &[Track], spacing: f32, avail: f32, start: usize, end: usize) -> f32 {
    let flex = bounded(avail);
    let span = &tracks[start..end];
    if span.iter().all(|t| matches!(t.size, GridLength::Px(_))) {
        let sizes: Vec<f32> = span
            .iter()
            .map(|t| match t.size {
                GridLength::Px(v) => clamp(t, v),
                _ => 0.0,
            })
            .collect();
        extent(&sizes, spacing, 0, sizes.len())
    } else if span.iter().any(|t| is_star(t, flex)) {
        avail
    } else {
        UNBOUNDED
    }
}

/// Less than this is nothing: what float sums leave over.
const SLACK: f32 = 0.01;

/// Track sizes along one axis of `avail` (`UNBOUNDED` = measure to content).
/// `items` are the children that size auto tracks: `(start, end, size)`;
/// `floor` the sizes a measure along an unbounded axis gave (see
/// [`GridLines`](super::GridLines)) — `*` tracks keep them when they fit.
fn size_axis(
    tracks: &[Track],
    avail: f32,
    spacing: f32,
    items: &mut [(usize, usize, f32)],
    floor: &[f32],
) -> Vec<f32> {
    let flex = bounded(avail);
    let mut size: Vec<f32> = tracks
        .iter()
        .map(|t| match t.size {
            GridLength::Px(v) => clamp(t, v),
            _ => clamp(t, 0.0),
        })
        .collect();
    grow_auto(tracks, flex, spacing, items, &mut size);
    if flex {
        share_stars(tracks, avail, spacing, floor, &mut size);
    }
    size
}

/// Auto tracks grow to their children: single spans, then wider ones only
/// add what's missing, shared by the span's auto tracks.
fn grow_auto(
    tracks: &[Track],
    flex: bool,
    spacing: f32,
    items: &mut [(usize, usize, f32)],
    size: &mut [f32],
) {
    items.sort_by_key(|&(s, e, _)| e - s);
    for &(s, e, want) in items.iter() {
        if e - s == 1 {
            if auto_like(&tracks[s], flex) {
                size[s] = clamp(&tracks[s], size[s].max(want));
            }
            continue;
        }
        let mut missing = want - extent(size, spacing, s, e);
        while missing > SLACK {
            let growable: Vec<usize> = (s..e)
                .filter(|&i| auto_like(&tracks[i], flex) && size[i] < tracks[i].max)
                .collect();
            if growable.is_empty() {
                break;
            }
            let share = missing / growable.len() as f32;
            for i in growable {
                let add = share.min(tracks[i].max - size[i]);
                size[i] += add;
                missing -= add;
            }
        }
    }
}

/// Star tracks share what's left of `avail`.
fn share_stars(tracks: &[Track], avail: f32, spacing: f32, floor: &[f32], size: &mut [f32]) {
    let open: Vec<usize> = (0..tracks.len())
        .filter(|&i| is_star(&tracks[i], true))
        .collect();
    let fixed: f32 = (0..tracks.len())
        .filter(|i| !open.contains(i))
        .map(|i| size[i])
        .sum();
    let rest = avail - fixed - spacing * tracks.len().saturating_sub(1) as f32;
    // Measured to content, then arranged at that size (an auto-height
    // grid in a scroll view): `*` tracks keep their content size —
    // unless it doesn't fit, so a long list in a `*` row still can't
    // push its siblings out.
    let floored: Vec<Track>;
    let tracks = if floor.len() == tracks.len()
        && open.iter().map(|&i| floor[i]).sum::<f32>() <= rest + SLACK
    {
        floored = tracks
            .iter()
            .zip(floor)
            .map(|(t, &f)| Track {
                min: t.min.max(f),
                ..*t
            })
            .collect();
        &floored[..]
    } else {
        tracks
    };
    share(tracks, open, rest, size);
}

/// `rest` shared by the `open` star tracks by weight: a track whose share
/// breaks a limit takes the limit and drops out, and the others share
/// again — min violations first.
fn share(tracks: &[Track], mut open: Vec<usize>, mut rest: f32, size: &mut [f32]) {
    let weight = |i: usize| match tracks[i].size {
        GridLength::Star(w) => w.max(0.0),
        _ => 0.0,
    };
    while !open.is_empty() {
        let total: f32 = open.iter().map(|&i| weight(i)).sum();
        if total <= 0.0 {
            for &i in &open {
                size[i] = clamp(&tracks[i], 0.0);
            }
            return;
        }
        let per = rest.max(0.0) / total;
        let under: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&i| per * weight(i) < tracks[i].min)
            .collect();
        let over: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&i| per * weight(i) > tracks[i].max.max(tracks[i].min))
            .collect();
        let fix = if !under.is_empty() { under } else { over };
        if fix.is_empty() {
            for &i in &open {
                size[i] = per * weight(i);
            }
            return;
        }
        for i in fix {
            size[i] = clamp(&tracks[i], per * weight(i));
            rest -= size[i];
            open.retain(|&o| o != i);
        }
    }
}

/// A grid being laid out: its content box, tracks and spacings.
struct Grid<'a> {
    w: &'a Widget,
    s: &'a Style,
    x: f32,
    y: f32,
    content_w: f32,
    content_h: f32,
    rows: &'a [Track],
    cols: &'a [Track],
    rsp: f32,
    csp: f32,
    depth: Depth,
}

/// Where the tracks went: each one's size and start.
struct Tracks {
    col_size: Vec<f32>,
    row_size: Vec<f32>,
    col_x: Vec<f32>,
    row_y: Vec<f32>,
}

/// Lays out a grid's children inside `node.rect` (size set, maybe
/// unbounded along an axis being measured).
pub(super) fn layout(
    w: &Widget,
    spec: &GridSpec,
    node: &mut LayoutNode,
    shrink: Option<Shrink>,
    depth: Depth,
) {
    let s = &w.style;
    // Arranging after a measure: that measure's floors (see `size_axis`).
    let floors = match (&shrink, node.grid.take()) {
        (None, Some(g)) => [g.floor_cols, g.floor_rows],
        _ => [Vec::new(), Vec::new()],
    };
    let (content_w, content_h, bar_w) = content_box(s, node.rect);
    let g = Grid {
        w,
        s,
        x: node.rect.x,
        y: node.rect.y,
        content_w,
        content_h,
        rows: tracks_or_one(&spec.rows),
        cols: tracks_or_one(&spec.cols),
        rsp: spec.row_spacing,
        csp: spec.col_spacing,
        depth,
    };
    let spans = g.spans();
    let t = g.size_tracks(&spans, &floors);
    let total = [
        extent(&t.col_size, g.csp, 0, g.cols.len()),
        extent(&t.row_size, g.rsp, 0, g.rows.len()),
    ];
    if let Some(fit) = &shrink {
        if fit.w {
            node.rect.w = (total[0] + s.pad[1] + s.pad[3] + bar_w)
                .max(s.min_w)
                .min(fit.max_w);
        }
        if fit.h {
            node.rect.h = (total[1] + s.pad[0] + s.pad[2]).max(s.min_h).min(fit.max_h);
        }
    }
    node.grid = Some(Box::new(g.lines(&t, floors)));
    let mut child_nodes: Vec<LayoutNode> = w
        .children
        .iter()
        .zip(&spans)
        .map(|(c, sp)| match sp {
            Some(sp) => g.in_cell(c, sp, &t),
            None => g.outside_cells(c, total),
        })
        .collect();
    g.scroll(node, &mut child_nodes, total[1]);
    node.children = child_nodes;
}

/// `tracks`, or one `*` track when there are none.
fn tracks_or_one(tracks: &[Track]) -> &[Track] {
    if tracks.is_empty() { &ONE_STAR } else { tracks }
}

/// Lines `sizes` up from `origin`: each track's start and size.
fn lines(sizes: &[f32], starts: &[f32]) -> Vec<(f32, f32)> {
    starts.iter().copied().zip(sizes.iter().copied()).collect()
}

/// Where each track starts, from `origin`.
fn starts(sizes: &[f32], spacing: f32, origin: f32) -> Vec<f32> {
    let mut at = origin;
    sizes
        .iter()
        .map(|v| {
            let p = at;
            at += v + spacing;
            p
        })
        .collect()
}

impl Grid<'_> {
    /// The columns sized (children measured for width only), then the rows
    /// (children measured at their final column widths), and where each
    /// starts; `floors`: a measure's column and row sizes.
    fn size_tracks(&self, spans: &[Option<Span>], floors: &[Vec<f32>; 2]) -> Tracks {
        let s = self.s;
        let col_size = size_axis(
            self.cols,
            self.content_w,
            self.csp,
            &mut self.col_items(spans),
            &floors[0],
        );
        let row_size = size_axis(
            self.rows,
            self.content_h,
            self.rsp,
            &mut self.row_items(spans, &col_size),
            &floors[1],
        );
        Tracks {
            col_x: starts(&col_size, self.csp, self.x + s.pad[3]),
            row_y: starts(&row_size, self.rsp, self.y + s.pad[0]),
            col_size,
            row_size,
        }
    }

    /// The grid's lines for its node: the tracks, and the floors an arrange
    /// keeps — this measure's sizes along an unbounded axis.
    fn lines(&self, t: &Tracks, floors: [Vec<f32>; 2]) -> GridLines {
        let [floor_cols, floor_rows] = floors;
        GridLines {
            rows: lines(&t.row_size, &t.row_y),
            cols: lines(&t.col_size, &t.col_x),
            floor_rows: if bounded(self.content_h) {
                floor_rows
            } else {
                t.row_size.clone()
            },
            floor_cols: if bounded(self.content_w) {
                floor_cols
            } else {
                t.col_size.clone()
            },
        }
    }

    /// Each placed child's cells, clamped to the grid (like WPF).
    fn spans(&self) -> Vec<Option<Span>> {
        let (rows, cols) = (self.rows.len(), self.cols.len());
        self.w
            .children
            .iter()
            .map(|c| {
                (c.visible && c.style.pos.is_none()).then(|| {
                    let cell = c.style.cell;
                    let r0 = (cell.row as usize).min(rows - 1);
                    let c0 = (cell.col as usize).min(cols - 1);
                    Span {
                        r0,
                        r1: (r0 + cell.row_span.max(1) as usize).min(rows),
                        c0,
                        c1: (c0 + cell.col_span.max(1) as usize).min(cols),
                    }
                })
            })
            .collect()
    }

    /// Whether a span of `tracks` sizes to its children: an auto track and no `*` one.
    fn sizes_auto(tracks: &[Track], avail: f32, a: usize, b: usize) -> bool {
        let flex = bounded(avail);
        let span = &tracks[a..b];
        span.iter().any(|t| auto_like(t, flex)) && !span.iter().any(|t| is_star(t, flex))
    }

    /// The children that size auto columns, measured for width only.
    fn col_items(&self, spans: &[Option<Span>]) -> Vec<(usize, usize, f32)> {
        let mut items = Vec::new();
        for (c, sp) in self.w.children.iter().zip(spans) {
            let Some(sp) = sp else { continue };
            if !Self::sizes_auto(self.cols, self.content_w, sp.c0, sp.c1) {
                continue;
            }
            let m = c.style.margin;
            let cap_h = less(
                measure_cap(self.rows, self.rsp, self.content_h, sp.r0, sp.r1),
                m[0] + m[2],
            );
            let mut tmp = LayoutNode::default();
            measure(
                c,
                &mut tmp,
                Room::new(UNBOUNDED, cap_h, UNBOUNDED, UNBOUNDED),
            );
            items.push((sp.c0, sp.c1, tmp.rect.w + m[1] + m[3]));
        }
        items
    }

    /// The children that size auto rows, measured at their final column widths.
    fn row_items(&self, spans: &[Option<Span>], col_size: &[f32]) -> Vec<(usize, usize, f32)> {
        let mut items = Vec::new();
        for (c, sp) in self.w.children.iter().zip(spans) {
            let Some(sp) = sp else { continue };
            if !Self::sizes_auto(self.rows, self.content_h, sp.r0, sp.r1) {
                continue;
            }
            let m = c.style.margin;
            let cell_w = less(extent(col_size, self.csp, sp.c0, sp.c1), m[1] + m[3]);
            let mut tmp = LayoutNode::default();
            measure(c, &mut tmp, Room::new(cell_w, UNBOUNDED, cell_w, UNBOUNDED));
            items.push((sp.r0, sp.r1, tmp.rect.h + m[0] + m[2]));
        }
        items
    }

    /// A child out of the cells: collapsed (a zero rect at the content
    /// box's corner), or positioned — as in a panel, inside the content
    /// box (`total`: the tracks' size, where an axis is unbounded).
    fn outside_cells(&self, c: &Widget, total: [f32; 2]) -> LayoutNode {
        let (s, m) = (self.s, c.style.margin);
        let mut cn = new_node(c);
        if !c.visible {
            cn.rect = Rect {
                x: self.x + s.pad[3],
                y: self.y + s.pad[0],
                w: 0.0,
                h: 0.0,
            };
        } else if let Some((px, py)) = c.style.pos {
            let cw = if bounded(self.content_w) {
                self.content_w
            } else {
                total[0]
            };
            let ch = if bounded(self.content_h) {
                self.content_h
            } else {
                total[1]
            };
            let room = Room::new(
                less(cw, px + m[1] + m[3]),
                less(ch, py + m[0] + m[2]),
                self.content_w,
                self.content_h,
            );
            measure(c, &mut cn, room);
            place(
                c,
                &mut cn,
                self.x + s.pad[3] + px + m[3],
                self.y + s.pad[0] + py + m[0],
                self.depth,
            );
        }
        cn
    }

    /// A child in its cell(s), less its margin: it fills them (`Stretch`,
    /// the default, unless it has an explicit size — then it's centred,
    /// like WPF) or sits at its `halign`/`valign` with its own measured
    /// size, capped to the cell.
    fn in_cell(&self, c: &Widget, sp: &Span, t: &Tracks) -> LayoutNode {
        let m = c.style.margin;
        let mut cn = new_node(c);
        let inner_w = (extent(&t.col_size, self.csp, sp.c0, sp.c1) - m[1] - m[3]).max(0.0);
        let inner_h = (extent(&t.row_size, self.rsp, sp.r0, sp.r1) - m[0] - m[2]).max(0.0);
        let room = Room::new(inner_w, inner_h, inner_w, inner_h);
        let halign = c.style.halign.unwrap_or(HAlign::Stretch);
        if halign == HAlign::Stretch {
            measure(c, &mut cn, room);
        } else {
            measure_aligned(c, &mut cn, room);
        }
        let fx = match halign {
            HAlign::Stretch if c.style.w.is_auto() => {
                cn.rect.w = inner_w;
                0.0
            }
            HAlign::Stretch | HAlign::Center => CENTRE,
            HAlign::Left => 0.0,
            HAlign::Right => 1.0,
        };
        let fy = match c.style.valign.unwrap_or(VAlign::Stretch) {
            VAlign::Stretch if c.style.h.is_auto() => {
                cn.rect.h = inner_h;
                0.0
            }
            VAlign::Stretch | VAlign::Center => CENTRE,
            VAlign::Top => 0.0,
            VAlign::Bottom => 1.0,
        };
        cn.rect.w = cn.rect.w.min(inner_w);
        cn.rect.h = cn.rect.h.min(inner_h);
        let cx = t.col_x[sp.c0] + m[3] + (inner_w - cn.rect.w) * fx;
        let cy = t.row_y[sp.r0] + m[0] + (inner_h - cn.rect.h) * fy;
        place(c, &mut cn, cx, cy, self.depth);
        cn
    }

    /// Scrolling (a grid with `scroll_y`): the tracks or the children,
    /// whichever reach further down (`total_h`: the tracks').
    fn scroll(&self, node: &mut LayoutNode, child_nodes: &mut [LayoutNode], total_h: f32) {
        let s = self.s;
        let below = self
            .w
            .children
            .iter()
            .zip(child_nodes.iter())
            .filter(|(c, _)| c.visible)
            .map(|(c, n)| n.rect.y - self.y + n.rect.h + c.style.margin[2])
            .fold(s.pad[0] + total_h, f32::max);
        scroll(node, s, below + s.pad[2], child_nodes);
        if node.scroll_y > 0.0
            && let Some(g) = &mut node.grid
        {
            for r in &mut g.rows {
                r.0 -= node.scroll_y;
            }
        }
    }
}

/// Where a centred child sits in the room it's left: halfway.
const CENTRE: f32 = 0.5;

#[cfg(test)]
mod tests;
