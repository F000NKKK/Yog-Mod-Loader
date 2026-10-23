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
