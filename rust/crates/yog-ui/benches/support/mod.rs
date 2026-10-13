//! Shared bench fixtures: realistic widget trees and a no-op graphics table.

#![allow(dead_code)] // each bench uses a subset

use yog_abi::{YogGfxApi, YogStr};
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

unsafe extern "C" fn zero() -> u32 {
    0
}
unsafe extern "C" fn nop_u32(_: u32) {}
unsafe extern "C" fn buf_data(_: u32, _: *const u8, _: u32, _: bool) {}
unsafe extern "C" fn buf_subdata(_: u32, _: u32, _: *const u8, _: u32) {}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn vao_attrib(_: u32, _: u32, _: u32, _: u8, _: u8, _: bool, _: u32, _: u32) {}
unsafe extern "C" fn vao_set_ebo(_: u32, _: u32) {}
unsafe extern "C" fn prog_create(_: YogStr, _: YogStr, _: *mut u32) -> bool {
    false
}
unsafe extern "C" fn uni_1i(_: u32, _: YogStr, _: i32) {}
unsafe extern "C" fn uni_1f(_: u32, _: YogStr, _: f32) {}
unsafe extern "C" fn uni_2f(_: u32, _: YogStr, _: f32, _: f32) {}
unsafe extern "C" fn uni_3f(_: u32, _: YogStr, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn uni_4f(_: u32, _: YogStr, _: f32, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn uni_mat4(_: u32, _: YogStr, _: *const f32) {}
unsafe extern "C" fn tex_create(_: u32, _: u32, _: *const u8, _: bool) -> u32 {
    0
}
unsafe extern "C" fn tex_bind(_: u32, _: u32) {}
unsafe extern "C" fn tex_from_mc(_: YogStr) -> u32 {
    0
}
unsafe extern "C" fn draw_arrays(_: u32, _: u32, _: u8, _: u32, _: u32) {}
unsafe extern "C" fn draw_elements(_: u32, _: u32, _: u32, _: u8, _: u32, _: bool) {}
unsafe extern "C" fn flag2(_: bool, _: bool) {}
unsafe extern "C" fn blend(_: bool, _: u32, _: u32) {}
unsafe extern "C" fn rect4(_: i32, _: i32, _: i32, _: i32) {}
unsafe extern "C" fn nop() {}
unsafe extern "C" fn rect(_: f32, _: f32, _: f32, _: f32, _: u32) {}
unsafe extern "C" fn gradient(_: f32, _: f32, _: f32, _: f32, _: u32, _: u32) {}
unsafe extern "C" fn text(_: YogStr, _: f32, _: f32, _: u32, _: bool) {}
unsafe extern "C" fn item(_: YogStr, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn text_font(_: YogStr, _: YogStr, _: f32, _: f32, _: u32, _: bool) {}
unsafe extern "C" fn text_width(t: YogStr, _: YogStr) -> f32 {
    t.len as f32 * 6.0
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn image(
    _: YogStr,
    _: f32,
    _: f32,
    _: f32,
    _: f32,
    _: f32,
    _: f32,
    _: f32,
    _: f32,
    _: u32,
) {
}
unsafe extern "C" fn image_size(_: YogStr, _: *mut f32, _: *mut f32) -> bool {
    false
}

/// A graphics table whose calls do nothing — rendering costs only the
/// UI's own work (styles, text wrapping, clipping).
pub fn null_gfx() -> YogGfxApi {
    YogGfxApi {
        screen_w: SCREEN_W as i32,
        screen_h: SCREEN_H as i32,
        delta_tick: 0.0,
        scale_factor: 1.0,
        view_proj: [0.0; 16],
        camera_pos: [0.0; 3],
        player_pos: [0.0; 3],
        _pad1: 0.0,
        buf_create: zero,
        buf_delete: nop_u32,
        buf_data,
        buf_subdata,
        vao_create: zero,
        vao_delete: nop_u32,
        vao_attrib,
        vao_set_ebo,
        prog_create,
        prog_delete: nop_u32,
        prog_uniform_1i: uni_1i,
        prog_uniform_1f: uni_1f,
        prog_uniform_2f: uni_2f,
        prog_uniform_3f: uni_3f,
        prog_uniform_4f: uni_4f,
        prog_uniform_mat4: uni_mat4,
        tex_create,
        tex_delete: nop_u32,
        tex_bind,
        tex_from_mc,
        draw_arrays,
        draw_elements,
        set_blend: blend,
        set_depth: flag2,
        set_scissor: rect4,
        clear_scissor: nop,
        set_viewport: rect4,
        draw2d_rect: rect,
        draw2d_gradient: gradient,
        draw2d_text: text,
        draw2d_item: item,
        draw2d_text_font: text_font,
        text_width,
        draw2d_image: image,
        image_size,
    }
}
