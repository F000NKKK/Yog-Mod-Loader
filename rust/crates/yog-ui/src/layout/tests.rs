use super::*;
use crate::widget::{self, Dock, Units};

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
    // A list screen: header, a flex row holding a 100%-high column
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

#[test]
fn a_screen_bigger_than_the_window_keeps_its_size_and_is_cut() {
    let content = widget::panel(FlexDir::Column)
        .w(Units::Px(300.0))
        .h(Units::Px(200.0))
        .child(widget::spacer().w(Units::Percent(100.0)).h(Units::Px(40.0)))
        .at(-50.0, -25.0);
    let screen = widget::panel(FlexDir::Column)
        .w(Units::Px(200.0))
        .h(Units::Px(150.0))
        .child(content);
    let l = super::compute(&screen, 200.0, 150.0);
    let c = &l.children[0];
    assert_eq!(
        (c.rect.x, c.rect.y, c.rect.w, c.rect.h),
        (-50.0, -25.0, 300.0, 200.0)
    );
    assert_eq!(
        c.children[0].rect.w, 300.0,
        "percent of its own size, not the window's"
    );
}
