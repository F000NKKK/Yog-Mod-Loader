//! Shared bench fixtures: realistic widget trees and a no-op graphics table.

#![allow(dead_code)] // each bench uses a subset

use yog_abi::YogGfxApi;
use yog_ui::widget::{self, Track};
use yog_ui::{Align, FlexDir, Units, Widget};

/// Screen size every bench lays out at (a typical 1080p GUI at scale 3).
pub const SCREEN_W: f32 = 640.0;
pub const SCREEN_H: f32 = 360.0;

/// Node counts the size-parametrised benches run at.
pub const SIZES: [usize; 3] = [10, 100, 1000];

const ROW_NODES: usize = 6;

/// A list row: icon, name + wrapped description, a
/// growing spacer and a button — 6 nodes.
pub fn list_row(i: usize) -> Widget {
    widget::panel(FlexDir::Row)
        .align(Align::Center)
        .padding(2.0, 2.0, 2.0, 2.0)
        .child(widget::item_slot("minecraft:apple"))
        .child(
            widget::panel(FlexDir::Column)
                .w(Units::Percent(60.0))
                .child(widget::label(format!("Entry number {i}")).color(0xFF_FFFFFF))
                .child(widget::label(
                    "A longer description line that has to wrap inside the column width",
                )),
        )
        .child(widget::spacer().flex(1.0))
        .child(widget::button("Open").on_click(format!("open:{i}")))
}

/// A scroll view of list rows totalling about `nodes` widgets.
pub fn list(nodes: usize) -> Widget {
    widget::scroll_view()
        .w(Units::Percent(100.0))
        .flex(1.0)
        .children((0..nodes.div_ceil(ROW_NODES)).map(list_row))
}

/// A full screen: header, a list of about `nodes` widgets beside a details
/// column, and a footer — panels nested 4-6 deep like real screens.
pub fn screen(nodes: usize) -> Widget {
    widget::panel(FlexDir::Column)
        .w(Units::Percent(100.0))
        .h(Units::Percent(100.0))
        .padding(4.0, 4.0, 4.0, 4.0)
        .child(
            widget::label("Title")
                .align(Align::Center)
                .w(Units::Percent(100.0)),
        )
        .child(
            widget::panel(FlexDir::Row)
                .flex(1.0)
                .w(Units::Percent(100.0))
                .child(
                    widget::panel(FlexDir::Column)
                        .w(Units::Percent(60.0))
                        .h(Units::Percent(100.0))
                        .child(widget::text_box("search").placeholder("Search..."))
                        .child(list(nodes)),
                )
                .child(
                    widget::panel(FlexDir::Column)
                        .flex(1.0)
                        .child(widget::label("Details").no_wrap())
                        .child(widget::label(
                            "Some details text that wraps a few times over",
                        )),
                ),
        )
        .child(
            widget::panel(FlexDir::Row)
                .child(widget::spacer().flex(1.0))
                .child(widget::button("Done").on_click("done")),
        )
}

/// Panels nested `depth` levels deep, alternating Row/Column, each with two
/// children; labels at the leaves — `2^(depth+1) - 1` nodes.
pub fn nested(depth: u32) -> Widget {
    fn level(d: u32, row: bool) -> Widget {
        if d == 0 {
            return widget::label("leaf").on_click("leaf");
        }
        let dir = if row { FlexDir::Row } else { FlexDir::Column };
        widget::panel(dir)
            .padding(1.0, 1.0, 1.0, 1.0)
            .child(level(d - 1, !row))
            .child(level(d - 1, !row).flex(1.0))
    }
    level(depth, false)
}

/// A grid of about `nodes` cells: auto / `*` / auto / pixel columns.
pub fn grid(nodes: usize) -> Widget {
    let rows = nodes.div_ceil(4);
    let mut g = widget::grid()
        .w(Units::Percent(100.0))
        .cols([
            Track::auto(),
            Track::star(1.0),
            Track::auto(),
            Track::px(40.0),
        ])
        .rows((0..rows).map(|_| Track::auto()))
        .row_spacing(1.0)
        .col_spacing(2.0);
    for r in 0..rows as u16 {
        g = g
            .child(widget::item_slot("minecraft:stone").cell(r, 0))
            .child(widget::label("Name that wraps in the star column").cell(r, 1))
            .child(widget::label("x64").cell(r, 2))
            .child(widget::button("Go").cell(r, 3));
    }
    widget::scroll_view().h(Units::Percent(100.0)).child(g)
}

// ── No-op graphics table ─────────────────────────────────────────────────────

pub fn null_gfx() -> YogGfxApi {
    yog_gfx::testing::null_api(SCREEN_W, SCREEN_H)
}
