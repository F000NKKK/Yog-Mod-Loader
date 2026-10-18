//! A `YogGfxApi` table whose `draw2d_*` functions record into a display
//! list instead of drawing — the game's side of the ABI, played by the IDE,
//! so yog-ui's own `render` (the code that runs in game) produces the
//! designer's picture. The GPU half of the table is inert.

use std::cell::RefCell;

use yog_abi::{YogGfxApi, YogStr};
use yog_gfx::GfxContext;

use super::raster::DrawOp;

thread_local! {
    static OPS: RefCell<Option<Vec<DrawOp>>> = const { RefCell::new(None) };
}

fn push(op: DrawOp) {
    OPS.with(|o| {
        if let Some(ops) = o.borrow_mut().as_mut() {
            ops.push(op);
        }
    });
}

/// # Safety
/// `s` must point to `len` readable bytes (or be null).
unsafe fn string(s: YogStr) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(s.ptr, s.len as usize) })
        .into_owned()
}

unsafe extern "C" fn rect(x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
    push(DrawOp::Rect {
        x1,
        y1,
        x2,
        y2,
        color,
    });
}
unsafe extern "C" fn gradient(x1: f32, y1: f32, x2: f32, y2: f32, top: u32, bottom: u32) {
    push(DrawOp::Gradient {
        x1,
        y1,
        x2,
        y2,
        top,
        bottom,
    });
}
unsafe extern "C" fn text(text: YogStr, x: f32, y: f32, color: u32, shadow: bool) {
    let text = unsafe { string(text) };
    push(DrawOp::Text {
        text,
        font: yog_ids::game::DEFAULT_FONT.into(),
        x,
        y,
        color,
        shadow,
    });
}
unsafe extern "C" fn text_font(
    text: YogStr,
    font: YogStr,
    x: f32,
    y: f32,
    color: u32,
    shadow: bool,
) {
    let (text, font) = unsafe { (string(text), string(font)) };
    let font = if font.trim().is_empty() {
        yog_ids::game::DEFAULT_FONT.to_string()
    } else {
        font
    };
    push(DrawOp::Text {
        text,
        font,
        x,
        y,
        color,
        shadow,
    });
}
/// Measured with the same glyphs the rasterizer draws (the document's fonts).
unsafe extern "C" fn text_width(text: YogStr, font: YogStr) -> f32 {
    let (text, font) = unsafe { (string(text), string(font)) };
    super::font::Font::load(&super::images::resources(), &font).width(&text)
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn image(
    source: YogStr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u: f32,
    v: f32,
    uw: f32,
    vh: f32,
    tint: u32,
) {
    let source = unsafe { string(source) };
    push(DrawOp::Image {
        source,
        x,
        y,
        w,
        h,
        region: [u, v, uw, vh],
        tint,
    });
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn text_scaled(text: YogStr, font: YogStr, x: f32, y: f32, scale: f32, color: u32, shadow: bool) {
    let (text, font) = unsafe { (string(text), string(font)) };
    let font = if font.is_empty() { yog_ids::game::DEFAULT_FONT.to_string() } else { font };
    push(DrawOp::TextScaled { text, font, x, y, scale, color, shadow });
}
unsafe extern "C" fn blocks(list: YogStr, x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32) {
    let list = unsafe { string(list) };
    push(DrawOp::Blocks { list, x, y, w, h, yaw, pitch });
}
unsafe extern "C" fn svg(data: YogStr, x: f32, y: f32, w: f32, h: f32) {
    let data = unsafe { string(data) };
    push(DrawOp::Svg { data, x, y, w, h });
}
unsafe extern "C" fn entity(entity: YogStr, x: f32, y: f32, w: f32, h: f32, _yaw: f32, _pitch: f32) {
    let entity = unsafe { string(entity) };
    push(DrawOp::Entity { entity, x, y, w, h });
}
unsafe extern "C" fn image_size(source: YogStr, w: *mut f32, h: *mut f32) -> bool {
    let source = unsafe { string(source) };
    match super::images::source_size(&source) {
        Some((sw, sh)) if !w.is_null() && !h.is_null() => {
            // SAFETY: non-null out-pointers from the caller (`Draw2D::image_size`).
            unsafe {
                *w = sw;
                *h = sh;
            }
            true
        }
        _ => false,
    }
}
unsafe extern "C" fn item(id: YogStr, x: f32, y: f32, size: f32) {
    let id = unsafe { string(id) };
    push(DrawOp::Item { id, x, y, size });
}
unsafe extern "C" fn scissor(x: i32, y: i32, w: i32, h: i32) {
    push(DrawOp::Scissor { x, y, w, h });
}
unsafe extern "C" fn clear_scissor() {
    push(DrawOp::ClearScissor);
}

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
unsafe extern "C" fn set_blend(_: bool, _: u32, _: u32) {}
unsafe extern "C" fn set_depth(_: bool, _: bool) {}
unsafe extern "C" fn set_viewport(_: i32, _: i32, _: i32, _: i32) {}

/// The recording table for a `screen_w × screen_h` GUI.
pub fn api(screen_w: i32, screen_h: i32, scale: f32) -> YogGfxApi {
    YogGfxApi {
        screen_w,
        screen_h,
        delta_tick: 0.0,
        scale_factor: scale,
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
        set_blend,
        set_depth,
        set_scissor: scissor,
        clear_scissor,
        set_viewport,
        draw2d_rect: rect,
        draw2d_gradient: gradient,
        draw2d_text: text,
        draw2d_item: item,
        draw2d_text_font: text_font,
        text_width,
        draw2d_image: image,
        image_size,
        draw2d_svg: svg,
        draw2d_entity: entity,
        draw2d_text_scaled: text_scaled,
        draw2d_blocks: blocks,
    }
}

/// Runs `draw` against a recording context and returns what it drew.
pub fn record(screen_w: i32, screen_h: i32, draw: impl FnOnce(&GfxContext)) -> Vec<DrawOp> {
    let table = api(screen_w, screen_h, 1.0);
    // SAFETY: `table` outlives `ctx`, and every function in it is sound to call.
    let ctx = unsafe { GfxContext::from_raw(&table) };
    OPS.with(|o| *o.borrow_mut() = Some(Vec::new()));
    draw(&ctx);
    OPS.with(|o| o.borrow_mut().take().unwrap_or_default())
}
