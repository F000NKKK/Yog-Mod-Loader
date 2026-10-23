//! The low-level GPU pipeline table (ABI 14).
use crate::YogStr;

// ── ABI minor 14 — low-level GPU pipeline ────────────────────────────────────

/// Low-level GPU context passed to render handlers.
///
/// Provides direct access to the OpenGL pipeline: buffer objects, vertex arrays,
/// shader programs, textures, draw calls, and render state.
///
/// The per-frame fields (`screen_w/h`, `delta_tick`, `view_proj`, `camera_pos`)
/// are filled by the runtime before calling the handler; the function pointers
/// point to statically-allocated implementations.
///
/// Valid only for the duration of the render callback — never store the pointer.
/// GPU resource **handles** (`u32`) may be stored between frames.
///
/// Colors are `0xAARRGGBB` (Minecraft convention).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogGfxApi {
    // ── Per-frame context ─────────────────────────────────────────────────────
    /// GUI-pixel screen width.
    pub screen_w: i32,
    /// GUI-pixel screen height.
    pub screen_h: i32,
    /// Partial-tick interpolation factor (0.0–1.0).
    pub delta_tick: f32,
    /// GUI scale factor: physical pixels per GUI pixel (e.g. 2.0 for 2× GUI scale).
    /// Useful for converting GUI-pixel coordinates to physical pixels for OpenGL calls.
    pub scale_factor: f32,
    /// View-projection matrix in camera-relative space (column-major, 16 × f32).
    /// All zeros during `on_hud_render`; filled during `on_world_render`.
    pub view_proj: [f32; 16],
    /// Camera world-space position.  All zeros during `on_hud_render`.
    pub camera_pos: [f32; 3],
    /// Local player world-space position (eye height).  All zeros during `on_hud_render`.
    /// Differs from `camera_pos` in third-person view.
    pub player_pos: [f32; 3],
    pub _pad1: f32,

    // ── GPU buffers ───────────────────────────────────────────────────────────
    /// Allocate a new GPU buffer (VBO / EBO). Returns 0 on failure.
    pub buf_create: unsafe extern "C" fn() -> u32,
    /// Delete a buffer created by `buf_create`.
    pub buf_delete: unsafe extern "C" fn(handle: u32),
    /// Upload `len` bytes from `bytes` into `handle`.
    /// `dynamic`: hints frequent updates (`GL_DYNAMIC_DRAW` vs `GL_STATIC_DRAW`).
    pub buf_data: unsafe extern "C" fn(handle: u32, bytes: *const u8, len: u32, dynamic: bool),
    /// Overwrite `len` bytes at `offset` in `handle`.
    pub buf_subdata: unsafe extern "C" fn(handle: u32, offset: u32, bytes: *const u8, len: u32),

    // ── Vertex arrays ─────────────────────────────────────────────────────────
    /// Create a vertex array object. Returns 0 on failure.
    pub vao_create: unsafe extern "C" fn() -> u32,
    /// Delete a VAO created by `vao_create`.
    pub vao_delete: unsafe extern "C" fn(handle: u32),
    /// Declare one vertex attribute in `vao`, sourced from `vbo`.
    /// `dtype`: 0=f32, 1=u8, 2=i32, 3=u32.
    pub vao_attrib: unsafe extern "C" fn(
        vao: u32,
        vbo: u32,
        index: u32,
        components: u8,
        dtype: u8,
        normalized: bool,
        stride: u32,
        offset: u32,
    ),
    /// Bind an index buffer (EBO) to `vao`.
    pub vao_set_ebo: unsafe extern "C" fn(vao: u32, ebo: u32),

    // ── Shader programs ───────────────────────────────────────────────────────
    /// Compile + link `vert_src` / `frag_src` (GLSL 150 core).
    /// Writes the program handle to `*out`. Returns false and logs on error.
    pub prog_create: unsafe extern "C" fn(vert: YogStr, frag: YogStr, out: *mut u32) -> bool,
    /// Delete a shader program.
    pub prog_delete: unsafe extern "C" fn(handle: u32),
    pub prog_uniform_1i: unsafe extern "C" fn(prog: u32, name: YogStr, v: i32),
    pub prog_uniform_1f: unsafe extern "C" fn(prog: u32, name: YogStr, v: f32),
    pub prog_uniform_2f: unsafe extern "C" fn(prog: u32, name: YogStr, x: f32, y: f32),
    pub prog_uniform_3f: unsafe extern "C" fn(prog: u32, name: YogStr, x: f32, y: f32, z: f32),
    pub prog_uniform_4f:
        unsafe extern "C" fn(prog: u32, name: YogStr, x: f32, y: f32, z: f32, w: f32),
    /// Set a mat4 uniform from 16 column-major floats.
    pub prog_uniform_mat4: unsafe extern "C" fn(prog: u32, name: YogStr, col_major: *const f32),

    // ── Textures ──────────────────────────────────────────────────────────────
    /// Upload RGBA8 pixel data as a new texture.
    /// `linear`: `GL_LINEAR` if true, `GL_NEAREST` if false.
    pub tex_create: unsafe extern "C" fn(w: u32, h: u32, rgba: *const u8, linear: bool) -> u32,
    /// Delete a texture created by `tex_create`.
    pub tex_delete: unsafe extern "C" fn(handle: u32),
    /// Bind `handle` to texture unit `unit` (0–7).
    pub tex_bind: unsafe extern "C" fn(unit: u32, handle: u32),
    /// Return the GL texture handle Minecraft uses for a namespaced resource
    /// (e.g. `"minecraft:textures/gui/icons.png"`). Returns 0 if not found.
    /// Do **not** delete handles obtained this way — Minecraft owns them.
    pub tex_from_mc: unsafe extern "C" fn(id: YogStr) -> u32,

    // ── Draw calls ────────────────────────────────────────────────────────────
    /// Draw `count` primitives from `vao`, using shader `prog`.
    /// `mode`: 0=Triangles, 1=Lines, 2=LineStrip, 3=TriangleStrip, 4=TriangleFan.
    pub draw_arrays: unsafe extern "C" fn(vao: u32, prog: u32, mode: u8, first: u32, count: u32),
    /// Draw indexed primitives.  `ebo` must be bound to `vao` via `vao_set_ebo`.
    /// `u32_idx`: `true` for `u32` indices, `false` for `u16` indices.
    pub draw_elements:
        unsafe extern "C" fn(vao: u32, ebo: u32, prog: u32, mode: u8, count: u32, u32_idx: bool),

    // ── Render state ──────────────────────────────────────────────────────────
    /// Enable/disable blending. `src`/`dst` are raw GL blend factor enum values.
    pub set_blend: unsafe extern "C" fn(enabled: bool, src: u32, dst: u32),
    /// Enable/disable depth testing and depth writes.
    pub set_depth: unsafe extern "C" fn(test: bool, write: bool),
    /// Enable the GL scissor: `x, y, w, h` in framebuffer pixels, from the
    /// bottom-left corner — for the GPU drawing here, not `draw2d_*` (the
    /// game batches 2D drawing; clip it by hand, as yog-ui does).
    pub set_scissor: unsafe extern "C" fn(x: i32, y: i32, w: i32, h: i32),
    /// Disable scissor clipping.
    pub clear_scissor: unsafe extern "C" fn(),
    /// Set the GL viewport (physical pixel coordinates).
    pub set_viewport: unsafe extern "C" fn(x: i32, y: i32, w: i32, h: i32),

    // ── 2D convenience (HUD-render only — uses MC's DrawContext) ─────────────
    /// Filled rectangle. Only valid during `on_hud_render`.
    pub draw2d_rect: unsafe extern "C" fn(x1: f32, y1: f32, x2: f32, y2: f32, color: u32),
    /// Vertical-gradient rectangle. Only valid during `on_hud_render`.
    pub draw2d_gradient:
        unsafe extern "C" fn(x1: f32, y1: f32, x2: f32, y2: f32, top: u32, bottom: u32),
    /// MC text renderer string. Only valid during `on_hud_render`.
    pub draw2d_text: unsafe extern "C" fn(text: YogStr, x: f32, y: f32, color: u32, shadow: bool),

    // ── appended in ABI minor 21 ──────────────────────────────────────────────
    /// Render an item stack (3D block models included) via MC's item renderer,
    /// like Patchouli's `renderItemStack`. `id` is a registry item id
    /// ("minecraft:crafting_table"); `size` is the on-screen size in GUI px
    /// (16 = inventory size). Only valid during `on_hud_render`.
    pub draw2d_item: unsafe extern "C" fn(id: YogStr, x: f32, y: f32, size: f32),

    // ── appended in ABI minor 33 ──────────────────────────────────────────────
    /// `draw2d_text` in the font `font` — a Yog font (`"mymod:fancy"`) or a
    /// game font by Yog's alias (`"yog:font/uniform"`); empty = the default
    /// font. Only
    /// valid during a render callback.
    pub draw2d_text_font:
        unsafe extern "C" fn(text: YogStr, font: YogStr, x: f32, y: f32, color: u32, shadow: bool),
    /// Width in GUI pixels of `text` drawn in `font` (empty = default), as the
    /// host's text renderer measures it. Callable from the render thread at
    /// any time (render callbacks and UI event handlers); the pointer stays
    /// valid for the life of the process.
    pub text_width: unsafe extern "C" fn(text: YogStr, font: YogStr) -> f32,

    // ── appended in ABI minor 34 ──────────────────────────────────────────────
    /// Draw the image `source` — a Yog image source (`item:apple`,
    /// `block:stone`, `sprite:button`, `mymod:gui/logo`, ...; see
    /// `yog_gfx::image`), resolved by the host for its Minecraft version —
    /// scaled into `(x, y, w, h)` (GUI pixels, nearest-neighbour). `(u, v,
    /// uw, vh)` is the part of the image to draw, in the image's own pixels;
    /// `uw <= 0` or `vh <= 0` = the whole image (items and sprites are
    /// always whole). `tint` multiplies the colors (`0xAARRGGBB`;
    /// `0xFFFFFFFF` = none; items ignore it). Only valid during a render
    /// callback.
    pub draw2d_image: unsafe extern "C" fn(
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
    ),
    /// Natural size in GUI pixels of the image `source` (16×16 for items, a
    /// texture's pixel size, ...) into `*w`/`*h`. `false` (outputs untouched)
    /// when the source can't be resolved. Callable from the render thread at
    /// any time; the pointer stays valid for the life of the process.
    pub image_size: unsafe extern "C" fn(source: YogStr, w: *mut f32, h: *mut f32) -> bool,

    // ── ABI 1.4 ──────────────────────────────────────────────────────────────
    /// Draws the SVG document `data` scaled into `(x, y, w, h)` GUI pixels
    /// (rasterized by the host at the screen's resolution, cached).
    pub draw2d_svg: unsafe extern "C" fn(data: YogStr, x: f32, y: f32, w: f32, h: f32),
    /// Draws a living entity of type `entity` (`minecraft:zombie`) standing
    /// in `(x, y, w, h)` GUI pixels, as big as fits, turned `yaw` degrees
    /// (0 = facing the viewer) and tilted `pitch` degrees.
    pub draw2d_entity:
        unsafe extern "C" fn(entity: YogStr, x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32),

    // ── ABI 1.5 ──────────────────────────────────────────────────────────────
    /// Draws `text` in `font` (empty = default) at `(x, y)`, `scale` times
    /// its size.
    pub draw2d_text_scaled: unsafe extern "C" fn(
        text: YogStr,
        font: YogStr,
        x: f32,
        y: f32,
        scale: f32,
        color: u32,
        shadow: bool,
    ),
    /// Draws blocks in 3D, as big as fits in `(x, y, w, h)`: `blocks` is one
    /// `x y z state` line per block (`0 0 0 minecraft:furnace[facing=north]`),
    /// turned `yaw` degrees about the vertical and tilted `pitch` degrees.
    pub draw2d_blocks:
        unsafe extern "C" fn(blocks: YogStr, x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32),
}

unsafe impl Send for YogGfxApi {}
unsafe impl Sync for YogGfxApi {}
