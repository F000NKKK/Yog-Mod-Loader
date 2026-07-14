//! Yog graphics — low-level GPU pipeline access for mods.
//!
//! Provides ergonomic wrappers around `YogGfxApi`, the stable C ABI table for
//! GPU operations (VAO/VBO, GLSL shaders, textures, draw calls, render state).
//!
//! # Render contexts
//!
//! Mods receive a [`GfxContext`] in two places:
//!
//! - `on_hud_render(|ctx| { ... })` — called every frame after the HUD is drawn.
//!   `ctx.view_proj()` is zeroed; `ctx.draw2d()` works.
//! - `on_world_render(|ctx| { ... })` — called after world geometry.
//!   `ctx.view_proj()` holds the camera view-projection matrix in camera-relative
//!   space; `ctx.camera_pos()` is the world-space camera position.
//!
//! # GPU resource lifetime
//!
//! GPU handles (`u32`) must be created and destroyed on the render thread.
//! Store handles between frames; pass `ctx` on every render call.
//!
//! ```ignore
//! use yog_gfx::{GfxContext, gl::{Buffer, ShaderProgram, VertexArray}};
//!
//! struct MyRenderer {
//!     vbo: Option<Buffer>,
//!     vao: Option<VertexArray>,
//!     prog: Option<ShaderProgram>,
//! }
//!
//! impl MyRenderer {
//!     fn ensure_init(&mut self, ctx: &GfxContext) {
//!         if self.vbo.is_some() { return; }
//!         let vbo = ctx.create_buffer();
//!         vbo.upload(ctx, &MY_VERTICES, false);
//!         let prog = ctx.create_shader(VERT_SRC, FRAG_SRC).unwrap();
//!         let vao = ctx.create_vao();
//!         vao.attrib(ctx, &vbo, 0, 3, yog_gfx::core::DataType::F32, false, 24, 0);
//!         self.vbo = Some(vbo);
//!         self.vao = Some(vao);
//!         self.prog = Some(prog);
//!     }
//!
//!     fn render(&mut self, ctx: &GfxContext) {
//!         self.ensure_init(ctx);
//!         let vao = self.vao.as_ref().unwrap();
//!         let prog = self.prog.as_ref().unwrap();
//!         prog.uniform_mat4(ctx, "uViewProj", &ctx.view_proj());
//!         ctx.draw_arrays(vao, prog, yog_gfx::core::DrawMode::Triangles, 0, 3);
//!     }
//! }
//! ```

pub mod core;
pub mod draw2d;
pub mod gl;

use yog_abi::YogGfxApi;

/// Handle to the GPU and draw capabilities for a single render frame.
///
/// Valid only within an `on_hud_render` or `on_world_render` callback.
/// Do **not** store across frames — store GPU resource handles (`u32`) instead.
#[derive(Copy, Clone)]
pub struct GfxContext(*const YogGfxApi);

unsafe impl Send for GfxContext {}
unsafe impl Sync for GfxContext {}

impl GfxContext {
    #[doc(hidden)]
    pub unsafe fn from_raw(raw: *const YogGfxApi) -> Self {
        Self(raw)
    }

    #[inline]
    fn api(&self) -> &YogGfxApi {
        unsafe { &*self.0 }
    }

    // ── Frame info ────────────────────────────────────────────────────────────

    /// GUI pixel dimensions of the screen for this frame.
    pub fn screen_size(&self) -> (i32, i32) {
        let a = self.api();
        (a.screen_w, a.screen_h)
    }

    /// Partial-tick interpolation factor (0.0–1.0).
    pub fn delta_tick(&self) -> f32 {
        self.api().delta_tick
    }

    /// View-projection matrix in camera-relative space (column-major, 16 × f32).
    /// Zeros during `on_hud_render`; filled during `on_world_render`.
    pub fn view_proj(&self) -> [f32; 16] {
        self.api().view_proj
    }

    /// Camera world-space position.  All zeros during `on_hud_render`.
    pub fn camera_pos(&self) -> [f32; 3] {
        self.api().camera_pos
    }

    /// Local player world-space position (eye height).  All zeros during `on_hud_render`.
    /// Use this to anchor geometry to the player; differs from `camera_pos` in third-person.
    pub fn player_pos(&self) -> [f32; 3] {
        self.api().player_pos
    }

    // ── GPU buffer ───────────────────────────────────────────────────────────

    /// Allocate a new GPU buffer (VBO or EBO). Returns handle 0 on failure.
    pub fn create_buffer(&self) -> gl::Buffer {
        gl::Buffer {
            handle: unsafe { (self.api().buf_create)() },
        }
    }

    /// Delete a buffer allocated by `create_buffer`.
    pub fn delete_buffer(&self, buf: gl::Buffer) {
        unsafe { (self.api().buf_delete)(buf.handle) }
    }

    // ── Vertex array ─────────────────────────────────────────────────────────

    /// Allocate a new vertex array object. Returns handle 0 on failure.
    pub fn create_vao(&self) -> gl::VertexArray {
        gl::VertexArray {
            handle: unsafe { (self.api().vao_create)() },
        }
    }

    /// Delete a vertex array allocated by `create_vao`.
    pub fn delete_vao(&self, vao: gl::VertexArray) {
        unsafe { (self.api().vao_delete)(vao.handle) }
    }

    // ── Shader program ────────────────────────────────────────────────────────

    /// Compile and link a GLSL shader program.
    /// Returns `Err(())` and logs on compile/link failure.
    pub fn create_shader(&self, vert_src: &str, frag_src: &str) -> Result<gl::ShaderProgram, ()> {
        use yog_abi::YogStr;
        let mut handle = 0u32;
        let ok = unsafe {
            (self.api().prog_create)(
                YogStr::from_str(vert_src),
                YogStr::from_str(frag_src),
                &mut handle,
            )
        };
        if ok && handle != 0 {
            Ok(gl::ShaderProgram { handle })
        } else {
            Err(())
        }
    }

    /// Delete a shader program.
    pub fn delete_shader(&self, prog: gl::ShaderProgram) {
        unsafe { (self.api().prog_delete)(prog.handle) }
    }

    // ── Texture ───────────────────────────────────────────────────────────────

    /// Upload RGBA8 pixel data as a new GPU texture.
    /// `linear`: `true` = bilinear filter, `false` = nearest.
    pub fn create_texture_rgba(&self, w: u32, h: u32, pixels: &[u8], linear: bool) -> gl::Texture {
        gl::Texture {
            handle: unsafe { (self.api().tex_create)(w, h, pixels.as_ptr(), linear) },
        }
    }

    /// Get the GL texture handle that Minecraft uses for an identifier
    /// (e.g. `"minecraft:textures/gui/icons.png"`).  Returns handle 0 if not found.
    pub fn texture_from_mc(&self, id: &str) -> gl::Texture {
        use yog_abi::YogStr;
        gl::Texture {
            handle: unsafe { (self.api().tex_from_mc)(YogStr::from_str(id)) },
        }
    }

    /// Delete a texture.
    pub fn delete_texture(&self, tex: gl::Texture) {
        unsafe { (self.api().tex_delete)(tex.handle) }
    }

    /// Bind a texture to the given sampler unit (0–7).
    pub fn bind_texture(&self, unit: u32, tex: &gl::Texture) {
        unsafe { (self.api().tex_bind)(unit, tex.handle) }
    }

    // ── Draw calls ────────────────────────────────────────────────────────────

    /// Draw primitives using a vertex array (no index buffer).
    pub fn draw_arrays(
        &self,
        vao: &gl::VertexArray,
        prog: &gl::ShaderProgram,
        mode: core::DrawMode,
        first: u32,
        count: u32,
    ) {
        unsafe { (self.api().draw_arrays)(vao.handle, prog.handle, mode as u8, first, count) }
    }

    /// Draw primitives via an index buffer.
    /// `u32_indices`: `true` = `u32` indices, `false` = `u16` indices.
    pub fn draw_elements(
        &self,
        vao: &gl::VertexArray,
        ebo: &gl::Buffer,
        prog: &gl::ShaderProgram,
        mode: core::DrawMode,
        count: u32,
        u32_indices: bool,
    ) {
        unsafe {
            (self.api().draw_elements)(
                vao.handle,
                ebo.handle,
                prog.handle,
                mode as u8,
                count,
                u32_indices,
            )
        }
    }

    // ── Render state ──────────────────────────────────────────────────────────

    /// Enable or disable alpha blending.
    /// `src`/`dst`: GL blend factor constants from [`core::blend`].
    pub fn set_blend(&self, enabled: bool, src: u32, dst: u32) {
        unsafe { (self.api().set_blend)(enabled, src, dst) }
    }

    /// Enable or disable depth testing and writing.
    pub fn set_depth(&self, test: bool, write: bool) {
        unsafe { (self.api().set_depth)(test, write) }
    }

    /// Enable scissor clipping to a GUI-pixel rectangle.
    pub fn set_scissor(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe { (self.api().set_scissor)(x, y, w, h) }
    }

    /// Disable scissor clipping.
    pub fn clear_scissor(&self) {
        unsafe { (self.api().clear_scissor)() }
    }

    /// Set the GL viewport in physical pixels (x, y, width, height).
    pub fn set_viewport(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe { (self.api().set_viewport)(x, y, w, h) }
    }

    // ── 2D convenience ────────────────────────────────────────────────────────

    /// Access the 2D drawing helpers (text, rectangles, MC textures).
    /// These only work during `on_hud_render`.
    pub fn draw2d(&self) -> draw2d::Draw2D<'_> {
        draw2d::Draw2D::new(self)
    }
}
