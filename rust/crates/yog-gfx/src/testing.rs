//! A graphics table whose every function does nothing — for tests and
//! benchmarks of code that draws (the UI, books) without a game.

use yog_abi::{YogGfxApi, YogStr};

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
unsafe extern "C" fn draw2d_svg(_: YogStr, _: f32, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn draw2d_text_scaled(
    _: YogStr,
    _: YogStr,
    _: f32,
    _: f32,
    _: f32,
    _: u32,
    _: bool,
) {
}
unsafe extern "C" fn draw2d_blocks(_: YogStr, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn draw2d_entity(_: YogStr, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn image_size(_: YogStr, _: *mut f32, _: *mut f32) -> bool {
    false
}

/// A graphics table whose calls do nothing — rendering costs only the
/// UI's own work (styles, text wrapping, clipping).
/// A `screen_w`×`screen_h` screen that draws nothing.
pub fn null_api(screen_w: f32, screen_h: f32) -> YogGfxApi {
    YogGfxApi {
        screen_w: screen_w as i32,
        screen_h: screen_h as i32,
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
        draw2d_svg,
        draw2d_entity,
        draw2d_text_scaled,
        draw2d_blocks,
    }
}
