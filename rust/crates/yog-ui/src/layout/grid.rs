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
    Depth, GridLines, LayoutNode, Rect, SCROLLBAR_W, Shrink, UNBOUNDED, bounded, less, measure,
    measure_aligned, new_node, place,
};
use crate::widget::{GridLength, GridSpec, HAlign, Track, VAlign, Widget};

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

    // Auto tracks: single spans, then wider ones only add what's missing.
    items.sort_by_key(|&(s, e, _)| e - s);
    for &(s, e, want) in items.iter() {
        if e - s == 1 {
            if auto_like(&tracks[s], flex) {
                size[s] = clamp(&tracks[s], size[s].max(want));
            }
            continue;
        }
        let mut missing = want - extent(&size, spacing, s, e);
        while missing > 0.01 {
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

    // Star tracks share what's left.
    if flex {
        let mut open: Vec<usize> = (0..tracks.len())
            .filter(|&i| is_star(&tracks[i], flex))
            .collect();
        let fixed: f32 = (0..tracks.len())
            .filter(|i| !open.contains(i))
            .map(|i| size[i])
            .sum();
        let mut rest = avail - fixed - spacing * tracks.len().saturating_sub(1) as f32;
        // Measured to content, then arranged at that size (an auto-height
        // grid in a scroll view): `*` tracks keep their content size —
        // unless it doesn't fit, so a long list in a `*` row still can't
        // push its siblings out.
        let floored: Vec<Track>;
        let tracks = if floor.len() == tracks.len()
            && open.iter().map(|&i| floor[i]).sum::<f32>() <= rest + 0.01
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
                break;
            }
            let per = rest.max(0.0) / total;
            // A track whose share breaks a limit takes the limit and drops
            // out; the others share again. Min violations first.
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
                break;
            }
            for i in fix {
                size[i] = clamp(&tracks[i], per * weight(i));
                rest -= size[i];
                open.retain(|&o| o != i);
            }
        }
    }
    size
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
    let (floor_rows, floor_cols) = match (&shrink, node.grid.take()) {
        (None, Some(g)) => (g.floor_rows, g.floor_cols),
        _ => (Vec::new(), Vec::new()),
    };
    let Rect { x, y, w: ww, h: hh } = node.rect;
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
    let rows: &[Track] = if spec.rows.is_empty() {
        &ONE_STAR
    } else {
        &spec.rows
    };
    let cols: &[Track] = if spec.cols.is_empty() {
        &ONE_STAR
    } else {
        &spec.cols
    };
    let (rsp, csp) = (spec.row_spacing, spec.col_spacing);

    // Each placed child's cells, clamped to the grid (like WPF).
    let spans: Vec<Option<Span>> = w
        .children
        .iter()
        .map(|c| {
            (c.visible && c.style.pos.is_none()).then(|| {
                let cell = c.style.cell;
                let r0 = (cell.row as usize).min(rows.len() - 1);
                let c0 = (cell.col as usize).min(cols.len() - 1);
                Span {
                    r0,
                    r1: (r0 + cell.row_span.max(1) as usize).min(rows.len()),
                    c0,
                    c1: (c0 + cell.col_span.max(1) as usize).min(cols.len()),
                }
            })
        })
        .collect();
    let sizes_auto = |tracks: &[Track], avail: f32, a: usize, b: usize| {
        let flex = bounded(avail);
        let span = &tracks[a..b];
        span.iter().any(|t| auto_like(t, flex)) && !span.iter().any(|t| is_star(t, flex))
    };

    // Columns: children measured for width only.
    let mut items = Vec::new();
    for (c, sp) in w.children.iter().zip(&spans) {
        let Some(sp) = sp else { continue };
        if !sizes_auto(cols, content_w, sp.c0, sp.c1) {
            continue;
        }
        let m = c.style.margin;
        let cap_h = less(measure_cap(rows, rsp, content_h, sp.r0, sp.r1), m[0] + m[2]);
        let mut tmp = LayoutNode::default();
        measure(c, &mut tmp, UNBOUNDED, cap_h, UNBOUNDED, UNBOUNDED);
        items.push((sp.c0, sp.c1, tmp.rect.w + m[1] + m[3]));
    }
    let col_size = size_axis(cols, content_w, csp, &mut items, &floor_cols);

    // Rows: children measured at their final column widths.
    let mut items = Vec::new();
    for (c, sp) in w.children.iter().zip(&spans) {
        let Some(sp) = sp else { continue };
        if !sizes_auto(rows, content_h, sp.r0, sp.r1) {
            continue;
        }
        let m = c.style.margin;
        let cell_w = less(extent(&col_size, csp, sp.c0, sp.c1), m[1] + m[3]);
        let mut tmp = LayoutNode::default();
        measure(c, &mut tmp, cell_w, UNBOUNDED, cell_w, UNBOUNDED);
        items.push((sp.r0, sp.r1, tmp.rect.h + m[0] + m[2]));
    }
    let row_size = size_axis(rows, content_h, rsp, &mut items, &floor_rows);

    let total_w = extent(&col_size, csp, 0, cols.len());
    let total_h = extent(&row_size, rsp, 0, rows.len());
    if let Some(fit) = &shrink {
        if fit.w {
            node.rect.w = (total_w + s.pad[1] + s.pad[3] + bar_w)
                .max(s.min_w)
                .min(fit.max_w);
        }
        if fit.h {
            node.rect.h = (total_h + s.pad[0] + s.pad[2]).max(s.min_h).min(fit.max_h);
        }
    }
    let starts = |sizes: &[f32], spacing: f32, origin: f32| -> Vec<f32> {
        let mut at = origin;
        sizes
            .iter()
            .map(|v| {
                let p = at;
                at += v + spacing;
                p
            })
            .collect()
    };
    let col_x = starts(&col_size, csp, x + s.pad[3]);
    let row_y = starts(&row_size, rsp, y + s.pad[0]);
    node.grid = Some(Box::new(GridLines {
        rows: row_y
            .iter()
            .copied()
            .zip(row_size.iter().copied())
            .collect(),
        cols: col_x
            .iter()
            .copied()
            .zip(col_size.iter().copied())
            .collect(),
        floor_rows: if bounded(content_h) {
            floor_rows
        } else {
            row_size.clone()
        },
        floor_cols: if bounded(content_w) {
            floor_cols
        } else {
            col_size.clone()
        },
    }));

    let mut child_nodes = Vec::with_capacity(w.children.len());
    for (c, sp) in w.children.iter().zip(&spans) {
        let mut cn = new_node(c);
        let m = c.style.margin;
        let Some(sp) = sp else {
            if !c.visible {
                cn.rect = Rect {
                    x: x + s.pad[3],
                    y: y + s.pad[0],
                    w: 0.0,
                    h: 0.0,
                };
            } else if let Some((px, py)) = c.style.pos {
                // Positioned: as in a panel, inside the content box.
                let (cw, ch) = (
                    bounded(content_w).then_some(content_w),
                    bounded(content_h).then_some(content_h),
                );
                let mw = less(cw.unwrap_or(total_w), px + m[1] + m[3]);
                let mh = less(ch.unwrap_or(total_h), py + m[0] + m[2]);
                measure(c, &mut cn, mw, mh, content_w, content_h);
                place(
                    c,
                    &mut cn,
                    x + s.pad[3] + px + m[3],
                    y + s.pad[0] + py + m[0],
                    depth,
                );
            }
            child_nodes.push(cn);
            continue;
        };
        let cell_w = extent(&col_size, csp, sp.c0, sp.c1);
        let cell_h = extent(&row_size, rsp, sp.r0, sp.r1);
        let inner_w = (cell_w - m[1] - m[3]).max(0.0);
        let inner_h = (cell_h - m[0] - m[2]).max(0.0);
        let halign = c.style.halign.unwrap_or(HAlign::Stretch);
        if halign == HAlign::Stretch {
            measure(c, &mut cn, inner_w, inner_h, inner_w, inner_h);
        } else {
            measure_aligned(c, &mut cn, inner_w, inner_h, inner_w, inner_h);
        }
        let valign = c.style.valign.unwrap_or(VAlign::Stretch);
        // Stretch fills the cell unless the size is explicit — then it's
        // centred, like WPF.
        let fx = match halign {
            HAlign::Stretch if c.style.w.is_auto() => {
                cn.rect.w = inner_w;
                0.0
            }
            HAlign::Stretch | HAlign::Center => 0.5,
            HAlign::Left => 0.0,
            HAlign::Right => 1.0,
        };
        let fy = match valign {
            VAlign::Stretch if c.style.h.is_auto() => {
                cn.rect.h = inner_h;
                0.0
            }
            VAlign::Stretch | VAlign::Center => 0.5,
            VAlign::Top => 0.0,
            VAlign::Bottom => 1.0,
        };
        cn.rect.w = cn.rect.w.min(inner_w);
        cn.rect.h = cn.rect.h.min(inner_h);
        let cx = col_x[sp.c0] + m[3] + (inner_w - cn.rect.w) * fx;
        let cy = row_y[sp.r0] + m[0] + (inner_h - cn.rect.h) * fy;
        place(c, &mut cn, cx, cy, depth);
        child_nodes.push(cn);
    }

    // Scrolling (a grid with `scroll_y`): the tracks or the children,
    // whichever reach further down.
    let below = w
        .children
        .iter()
        .zip(&child_nodes)
        .filter(|(c, _)| c.visible)
        .map(|(c, n)| n.rect.y - y + n.rect.h + c.style.margin[2])
        .fold(s.pad[0] + total_h, f32::max);
    node.content_h = below + s.pad[2];
    node.scroll_y = s.scroll_y.clamp(0.0, node.max_scroll());
    if node.scroll_y > 0.0 {
        for c in &mut child_nodes {
            c.translate_y(-node.scroll_y);
        }
        if let Some(g) = &mut node.grid {
            for r in &mut g.rows {
                r.0 -= node.scroll_y;
            }
        }
    }
    node.children = child_nodes;
}

#[cfg(test)]
mod tests {
    use crate::layout::{FlexDir, LayoutNode, compute};
    use crate::widget::{self, GridLength, Track, Units};

    fn r(n: &LayoutNode) -> (f32, f32, f32, f32) {
        (n.rect.x, n.rect.y, n.rect.w, n.rect.h)
    }

    fn block(w: f32, h: f32) -> widget::Widget {
        widget::spacer().min_w(w).min_h(h)
    }

    #[test]
    fn pixel_auto_and_star_rows() {
        let g = widget::grid()
            .w(100.0)
            .h(200.0)
            .rows([Track::auto(), Track::star(1.0), Track::px(26.0)])
            .child(widget::panel(FlexDir::Column).h(30.0).cell(0, 0))
            .child(widget::panel(FlexDir::Column).cell(1, 0))
            .child(widget::panel(FlexDir::Column).cell(2, 0));
        let n = compute(&g, 400.0, 400.0);
        assert_eq!(r(&n), (0.0, 0.0, 100.0, 200.0));
        assert_eq!(r(&n.children[0]), (0.0, 0.0, 100.0, 30.0));
        assert_eq!(r(&n.children[1]), (0.0, 30.0, 100.0, 144.0));
        assert_eq!(r(&n.children[2]), (0.0, 174.0, 100.0, 26.0));
    }

    #[test]
    fn star_weights_and_spacing() {
        let g = widget::grid()
            .w(310.0)
            .h(50.0)
            .cols([Track::star(1.0), Track::star(2.0)])
            .col_spacing(10.0)
            .child(block(1.0, 1.0).cell(0, 0))
            .child(block(1.0, 1.0).cell(0, 1));
        let n = compute(&g, 400.0, 400.0);
        assert_eq!(r(&n.children[0]), (0.0, 0.0, 100.0, 50.0));
        assert_eq!(r(&n.children[1]), (110.0, 0.0, 200.0, 50.0));
    }

    #[test]
    fn star_min_and_max() {
        let g = widget::grid()
            .w(300.0)
            .h(10.0)
            .cols([
                Track::star(1.0).max(50.0),
                Track::star(1.0),
                Track::star(1.0).min(200.0),
            ])
            .child(block(1.0, 1.0).cell(0, 0))
            .child(block(1.0, 1.0).cell(0, 1))
            .child(block(1.0, 1.0).cell(0, 2));
        let n = compute(&g, 400.0, 400.0);
        // The third takes its min (200), the first its max (50), the middle the rest.
        assert_eq!(n.children[0].rect.w, 50.0);
        assert_eq!(n.children[1].rect.w, 50.0);
        assert_eq!(n.children[2].rect.w, 200.0);
        assert_eq!(n.children[2].rect.x, 100.0);
    }

    #[test]
    fn auto_track_limits() {
        let g = widget::grid()
            .w(300.0)
            .h(10.0)
            .cols([
                Track::auto().max(40.0),
                Track::auto().min(30.0),
                GridLength::STAR.into(),
            ])
            .child(block(80.0, 5.0).cell(0, 0))
            .child(block(10.0, 5.0).cell(0, 1));
        let n = compute(&g, 400.0, 400.0);
        assert_eq!(n.children[0].rect.w, 40.0);
        assert_eq!(n.children[1].rect.x, 40.0);
        assert_eq!(n.children[1].rect.w, 30.0);
    }

    #[test]
    fn spans_grow_auto_tracks_only_by_whats_missing() {
        let g = widget::grid()
            .cols([Track::auto(), Track::auto(), Track::px(10.0)])
            .rows([Track::auto(), Track::auto()])
            .child(block(20.0, 5.0).cell(0, 0))
            .child(block(60.0, 5.0).cell(1, 0).span(1, 3));
        let n = compute(&g, 400.0, 400.0);
        // 60 needed over 20 + 0 + 10: the two auto columns share the 30 missing.
        assert_eq!(n.children[0].rect.w, 35.0);
        assert_eq!(r(&n.children[1]), (0.0, 5.0, 60.0, 5.0));
        assert_eq!(n.rect.w, 60.0);
    }

    #[test]
    fn alignment_and_margin_in_a_cell() {
        let g = widget::grid()
            .w(100.0)
            .h(100.0)
            .child(
                block(10.0, 10.0)
                    .halign(widget::HAlign::Right)
                    .valign(widget::VAlign::Center),
            )
            .child(block(10.0, 10.0).margin(5.0, 6.0, 7.0, 8.0))
            .child(block(10.0, 10.0).w(20.0).h(20.0));
        let n = compute(&g, 400.0, 400.0);
        assert_eq!(r(&n.children[0]), (90.0, 45.0, 10.0, 10.0));
        assert_eq!(r(&n.children[1]), (8.0, 5.0, 86.0, 88.0));
        // An explicit size under Stretch is centred.
        assert_eq!(r(&n.children[2]), (40.0, 40.0, 20.0, 20.0));
    }

    #[test]
    fn unbounded_axis_treats_stars_as_auto() {
        // An auto-height grid in a scroll view: its `*` rows size to content.
        let sv = widget::scroll_view().w(100.0).h(50.0).child(
            widget::grid()
                .rows([Track::star(1.0), Track::star(1.0)])
                .child(block(10.0, 40.0).cell(0, 0))
                .child(block(10.0, 30.0).cell(1, 0)),
        );
        let n = compute(&sv, 400.0, 400.0);
        let g = &n.children[0];
        assert_eq!(g.rect.h, 70.0);
        assert_eq!(g.children[1].rect.y, 40.0);
        assert_eq!(n.max_scroll(), 20.0);
    }

    #[test]
    fn auto_width_grid_in_a_row_sizes_to_content() {
        let row = widget::panel(FlexDir::Row).gap(0.0).w(300.0).h(20.0).child(
            widget::grid()
                .cols([Track::star(1.0), Track::px(10.0)])
                .child(block(25.0, 5.0)),
        );
        let n = compute(&row, 400.0, 400.0);
        assert_eq!(n.children[0].rect.w, 35.0);
    }

    #[test]
    fn long_list_in_a_star_row_does_not_push_siblings() {
        let mut list = widget::scroll_view();
        for i in 0..100 {
            list = list.child(widget::label(format!("Item {i}")).no_wrap());
        }
        let g = widget::grid()
            .w(Units::Percent(100.0))
            .h(Units::Percent(100.0))
            .rows([Track::auto(), Track::star(1.0), Track::px(20.0)])
            .cols([Track::star(1.0), Track::star(1.0)])
            .child(widget::label("Header").no_wrap().cell(0, 0).span(1, 2))
            .child(
                widget::panel(FlexDir::Column)
                    .cell(1, 0)
                    .child(list.flex(1.0)),
            )
            .child(widget::label("Details").cell(1, 1))
            .child(widget::button("Done").cell(2, 1));
        let n = compute(&g, 200.0, 150.0);
        let header_h = n.children[0].rect.h;
        let left = &n.children[1];
        assert_eq!(r(left), (0.0, header_h, 100.0, 150.0 - 20.0 - header_h));
        // The list fills the cell and scrolls; the footer stays in place.
        let sv = &left.children[0];
        assert_eq!(sv.rect.h, left.rect.h);
        assert!(sv.max_scroll() > 0.0);
        assert_eq!(r(&n.children[3]), (100.0, 130.0, 100.0, 20.0));
    }

    #[test]
    fn nested_grids() {
        let inner = widget::grid()
            .cols([Track::px(10.0), Track::star(1.0)])
            .cell(0, 1)
            .child(block(1.0, 1.0))
            .child(block(1.0, 1.0).cell(0, 1));
        let g = widget::grid()
            .w(100.0)
            .h(40.0)
            .cols([Track::px(30.0), Track::star(1.0)])
            .child(inner);
        let n = compute(&g, 400.0, 400.0);
        let i = &n.children[0];
        assert_eq!(r(i), (30.0, 0.0, 70.0, 40.0));
        assert_eq!(r(&i.children[1]), (40.0, 0.0, 60.0, 40.0));
    }

    #[test]
    fn out_of_range_cells_clamp_and_collapsed_children_vanish() {
        let g = widget::grid()
            .w(100.0)
            .h(100.0)
            .rows([Track::px(10.0), Track::star(1.0)])
            .child(block(1.0, 1.0).cell(9, 9).span(4, 4))
            .child(block(1.0, 1.0).cell(1, 0).visible(false));
        let n = compute(&g, 400.0, 400.0);
        assert_eq!(r(&n.children[0]), (0.0, 10.0, 100.0, 90.0));
        assert_eq!(r(&n.children[1]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn empty_grid_takes_its_pixel_tracks() {
        let col = widget::panel(FlexDir::Column).w(200.0).h(200.0).child(
            widget::grid()
                .rows([Track::px(12.0), Track::px(8.0)])
                .cols([Track::px(30.0)])
                .row_spacing(2.0),
        );
        let n = compute(&col, 400.0, 400.0);
        assert_eq!((n.children[0].rect.w, n.children[0].rect.h), (30.0, 22.0));
    }

    #[test]
    fn wrapped_text_in_an_auto_row_measures_at_the_column_width() {
        let g = widget::grid()
            .w(40.0)
            .h(100.0)
            .rows([Track::auto(), Track::star(1.0)])
            .child(widget::label("aaaa bbbb cccc dddd"));
        let n = compute(&g, 400.0, 400.0);
        assert!(n.children[0].rect.h > 9.0, "{:?}", n.children[0].rect);
        assert_eq!(n.children[0].rect.w, 40.0);
    }

    #[test]
    fn grown_grid_in_a_column_keeps_a_long_star_row_inside() {
        let mut list = widget::scroll_view();
        for i in 0..100 {
            list = list.child(widget::label(format!("Item {i}")).no_wrap());
        }
        let col = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(100.0)
            .gap(0.0)
            .child(block(10.0, 10.0))
            .child(
                widget::grid()
                    .flex(1.0)
                    .rows([Track::star(1.0), Track::px(10.0)])
                    .child(list)
                    .child(block(5.0, 5.0).cell(1, 0)),
            );
        let n = compute(&col, 400.0, 400.0);
        let g = &n.children[1];
        assert_eq!(r(g), (0.0, 10.0, 100.0, 90.0));
        assert_eq!(g.children[0].rect.h, 80.0);
        assert_eq!(g.children[1].rect.y, 90.0);
        let lines = g.grid.as_ref().unwrap();
        assert_eq!(lines.rows, vec![(10.0, 80.0), (90.0, 10.0)]);
    }

    #[test]
    fn margins_and_self_alignment_in_flex_panels() {
        let col = widget::panel(FlexDir::Column)
            .w(100.0)
            .h(100.0)
            .gap(0.0)
            .padding(1.0, 1.0, 1.0, 1.0)
            .child(block(10.0, 10.0).margin(2.0, 3.0, 4.0, 5.0))
            .child(block(10.0, 10.0).halign(widget::HAlign::Right))
            .child(
                block(10.0, 10.0)
                    .halign(widget::HAlign::Stretch)
                    .margin(0.0, 4.0, 0.0, 4.0),
            );
        let n = compute(&col, 400.0, 400.0);
        assert_eq!(r(&n.children[0]), (6.0, 3.0, 10.0, 10.0));
        assert_eq!(r(&n.children[1]), (89.0, 17.0, 10.0, 10.0));
        assert_eq!(r(&n.children[2]), (5.0, 27.0, 90.0, 10.0));
        // Auto size counts the margins.
        let row = widget::panel(FlexDir::Row)
            .gap(0.0)
            .child(block(10.0, 10.0).margin(1.0, 2.0, 3.0, 4.0));
        let n = compute(&row, 400.0, 400.0);
        assert_eq!((n.rect.w, n.rect.h), (16.0, 14.0));
    }
}
