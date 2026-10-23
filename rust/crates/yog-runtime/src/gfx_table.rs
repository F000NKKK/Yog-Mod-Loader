//! 2D drawing through the game, and the GFX function table.

use crate::fonts::Named;
use crate::gpu::gfx_buf_create;
use crate::gpu::gfx_buf_delete;
use crate::gpu::gfx_prog_create;
use crate::gpu::gfx_prog_delete;
use crate::gpu::gfx_tex_create;
use crate::gpu::gfx_tex_delete;
use crate::gpu::gfx_tex_from_mc;
use crate::gpu::gfx_vao_attrib;
use crate::gpu::gfx_vao_create;
use crate::gpu::gfx_vao_delete;
use crate::gpu::gfx_vao_set_ebo;
use crate::gpu_record::{
    rec_buf_data, rec_buf_subdata, rec_clear_scissor, rec_draw_arrays, rec_draw_elements,
    rec_set_blend, rec_set_depth, rec_set_scissor, rec_set_viewport, rec_tex_bind, rec_uniform_1f,
    rec_uniform_1i, rec_uniform_2f, rec_uniform_3f, rec_uniform_4f, rec_uniform_mat4,
};
use crate::images;
use crate::svg;
use yog_abi::YogGfxApi;
use yog_abi::YogStr;

// ── 2D convenience (JNI — uses MC's DrawContext / text renderer) ──────────────

pub(super) unsafe extern "C" fn gfx_draw2d_rect(x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
    crate::jni_util::guard("gfx_draw2d_rect", || unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawRect",
            [F(x1), F(y1), F(x2), F(y2), I(color as i32)]
        );
    });
}

pub(super) unsafe extern "C" fn gfx_draw2d_gradient(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    top: u32,
    bottom: u32,
) {
    crate::jni_util::guard("gfx_draw2d_gradient", || unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawGradientRect",
            [F(x1), F(y1), F(x2), F(y2), I(top as i32), I(bottom as i32)]
        );
    });
}

pub(super) unsafe extern "C" fn gfx_draw2d_text(
    text: YogStr,
    x: f32,
    y: f32,
    color: u32,
    shadow: bool,
) {
    crate::jni_util::guard("gfx_draw2d_text", || unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawText",
            [S(text), F(x), F(y), I(color as i32), Z(shadow)]
        );
    });
}

pub(super) unsafe extern "C" fn gfx_draw2d_text_font(
    text: YogStr,
    font: YogStr,
    x: f32,
    y: f32,
    color: u32,
    shadow: bool,
) {
    crate::jni_util::guard("gfx_draw2d_text_font", || {
        let id = unsafe { font.as_str() };
        match crate::fonts::named(id) {
            Named::Yog(f) => {
                crate::fonts::draw(id, &f, unsafe { text.as_str() }, [x, y, 1.0], color, shadow)
            }
            Named::Game(game) => unsafe {
                let _: Option<()> = host!(
                    "dev/yog/NativeDraw",
                    "drawTextFont",
                    [
                        S(text),
                        S(YogStr::from_str(&game)),
                        F(x),
                        F(y),
                        I(color as i32),
                        Z(shadow)
                    ]
                );
            },
        }
    });
}

pub(super) unsafe extern "C" fn gfx_draw2d_text_scaled(
    text: YogStr,
    font: YogStr,
    x: f32,
    y: f32,
    scale: f32,
    color: u32,
    shadow: bool,
) {
    crate::jni_util::guard("gfx_draw2d_text_scaled", || {
        let id = unsafe { font.as_str() };
        match crate::fonts::named(id) {
            Named::Yog(f) => crate::fonts::draw(
                id,
                &f,
                unsafe { text.as_str() },
                [x, y, scale],
                color,
                shadow,
            ),
            Named::Game(game) => unsafe {
                let _: Option<()> = host!(
                    "dev/yog/NativeDraw",
                    "drawTextScaled",
                    [
                        S(text),
                        S(YogStr::from_str(&game)),
                        F(x),
                        F(y),
                        F(scale),
                        I(color as i32),
                        Z(shadow)
                    ]
                );
            },
        }
    });
}

pub(super) unsafe extern "C" fn gfx_draw2d_blocks(
    blocks: YogStr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    yaw: f32,
    pitch: f32,
) {
    crate::jni_util::guard("gfx_draw2d_blocks", || {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        unsafe {
            let _: Option<()> = host!(
                "dev/yog/NativeDraw",
                "drawBlocks",
                [S(blocks), F(x), F(y), F(w), F(h), F(yaw), F(pitch)]
            );
        }
    });
}

pub(super) unsafe extern "C" fn gfx_text_width(text: YogStr, font: YogStr) -> f32 {
    crate::jni_util::guarded("gfx_text_width", 0.0, || {
        match crate::fonts::named(unsafe { font.as_str() }) {
            Named::Yog(f) => crate::fonts::width(&f, unsafe { text.as_str() }, 1.0),
            Named::Game(game) => unsafe {
                host!(
                    "dev/yog/NativeDraw",
                    "textWidth",
                    [S(text), S(YogStr::from_str(&game))]
                )
                .unwrap_or(0.0)
            },
        }
    })
}

pub(super) unsafe extern "C" fn gfx_draw2d_item(id: YogStr, x: f32, y: f32, size: f32) {
    crate::jni_util::guard("gfx_draw2d_item", || unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawItem",
            [S(id), F(x), F(y), F(size)]
        );
    });
}

// ── Static GFX function table (function pointers only — per-frame data filled at call time) ──

pub(super) static GFX_FN_TABLE: YogGfxApi = YogGfxApi {
    // Per-frame fields zeroed in the static; actual values are set on the stack per render call.
    screen_w: 0,
    screen_h: 0,
    delta_tick: 0.0,
    scale_factor: 1.0,
    view_proj: [0.0; 16],
    camera_pos: [0.0; 3],
    player_pos: [0.0; 3],
    _pad1: 0.0,
    buf_create: gfx_buf_create,
    buf_delete: gfx_buf_delete,
    buf_data: rec_buf_data,
    buf_subdata: rec_buf_subdata,
    vao_create: gfx_vao_create,
    vao_delete: gfx_vao_delete,
    vao_attrib: gfx_vao_attrib,
    vao_set_ebo: gfx_vao_set_ebo,
    prog_create: gfx_prog_create,
    prog_delete: gfx_prog_delete,
    prog_uniform_1i: rec_uniform_1i,
    prog_uniform_1f: rec_uniform_1f,
    prog_uniform_2f: rec_uniform_2f,
    prog_uniform_3f: rec_uniform_3f,
    prog_uniform_4f: rec_uniform_4f,
    prog_uniform_mat4: rec_uniform_mat4,
    tex_create: gfx_tex_create,
    tex_delete: gfx_tex_delete,
    tex_bind: rec_tex_bind,
    tex_from_mc: gfx_tex_from_mc,
    draw_arrays: rec_draw_arrays,
    draw_elements: rec_draw_elements,
    set_blend: rec_set_blend,
    set_depth: rec_set_depth,
    set_scissor: rec_set_scissor,
    clear_scissor: rec_clear_scissor,
    set_viewport: rec_set_viewport,
    draw2d_rect: gfx_draw2d_rect,
    draw2d_gradient: gfx_draw2d_gradient,
    draw2d_text: gfx_draw2d_text,
    draw2d_item: gfx_draw2d_item,
    draw2d_text_font: gfx_draw2d_text_font,
    text_width: gfx_text_width,
    draw2d_image: images::gfx_draw2d_image,
    image_size: images::gfx_image_size,
    draw2d_svg: svg::gfx_draw2d_svg,
    draw2d_entity: svg::gfx_draw2d_entity,
    draw2d_text_scaled: gfx_draw2d_text_scaled,
    draw2d_blocks: gfx_draw2d_blocks,
};
