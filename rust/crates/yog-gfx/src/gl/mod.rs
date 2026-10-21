//! Low-level GPU resource wrappers: [`Buffer`], [`VertexArray`], [`ShaderProgram`], [`Texture`].
//!
//! All types are plain handles (`u32`).  Create them via [`GfxContext`] methods;
//! delete them explicitly with `ctx.delete_*` when no longer needed.

use yog_abi::YogStr;

use crate::GfxContext;
use crate::core::DataType;

// ── Buffer ────────────────────────────────────────────────────────────────────

/// A GPU buffer (VBO or EBO).
///
/// Created by [`GfxContext::create_buffer`]. Must be deleted with
/// [`GfxContext::delete_buffer`] when done.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    pub handle: u32,
}

impl Buffer {
    /// Upload bytes to this buffer.
    /// `dynamic`: hint that the buffer will be updated frequently.
    pub fn upload_bytes(&self, ctx: &GfxContext, data: &[u8], dynamic: bool) {
        unsafe { (ctx.api().buf_data)(self.handle, data.as_ptr(), data.len() as u32, dynamic) }
    }

    /// Upload typed data to this buffer.  `T` must be a plain-old-data type
    /// (the bytes are used as-is, in the layout `T` has in memory).
    ///
    /// # Safety
    /// The byte representation of `T` must be well-defined (no padding traps,
    /// no floats encoding NaN, etc.). Use `#[repr(C)]` structs or primitive types.
    pub unsafe fn upload<T: Sized>(&self, ctx: &GfxContext, data: &[T], dynamic: bool) {
        unsafe {
            let bytes =
                std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data));
            self.upload_bytes(ctx, bytes, dynamic);
        }
    }

    /// Overwrite a sub-range of this buffer starting at `offset` bytes.
    pub fn subdata_bytes(&self, ctx: &GfxContext, offset: u32, data: &[u8]) {
        unsafe { (ctx.api().buf_subdata)(self.handle, offset, data.as_ptr(), data.len() as u32) }
    }
}

// ── VertexArray ───────────────────────────────────────────────────────────────

/// A vertex array object (VAO).
///
/// Created by [`GfxContext::create_vao`]. Must be deleted with
/// [`GfxContext::delete_vao`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VertexArray {
    pub handle: u32,
}

/// One vertex attribute: where the shader reads it and how it lies in the buffer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Attrib {
    /// The attribute's location in the shader (`layout(location = N)`).
    pub index: u32,
    /// How many components (1–4).
    pub components: u8,
    pub dtype: DataType,
    /// A `U8` component mapped to 0–1.
    pub normalized: bool,
    /// Bytes from one vertex to the next.
    pub stride: u32,
    /// Bytes from a vertex's start to this attribute.
    pub offset: u32,
}

impl VertexArray {
    /// Declares vertex attribute `a` of this VAO, sourced from `vbo`.
    pub fn attrib(&self, ctx: &GfxContext, vbo: &Buffer, a: Attrib) {
        unsafe {
            (ctx.api().vao_attrib)(
                self.handle,
                vbo.handle,
                a.index,
                a.components,
                a.dtype as u8,
                a.normalized,
                a.stride,
                a.offset,
            )
        }
    }

    /// Bind an index buffer (EBO) to this VAO.
    pub fn set_ebo(&self, ctx: &GfxContext, ebo: &Buffer) {
        unsafe { (ctx.api().vao_set_ebo)(self.handle, ebo.handle) }
    }
}

// ── ShaderProgram ─────────────────────────────────────────────────────────────

/// A compiled and linked GLSL shader program.
///
/// Created by [`GfxContext::create_shader`]. Must be deleted with
/// [`GfxContext::delete_shader`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ShaderProgram {
    pub handle: u32,
}

impl ShaderProgram {
    pub fn uniform_1i(&self, ctx: &GfxContext, name: &str, v: i32) {
        unsafe { (ctx.api().prog_uniform_1i)(self.handle, YogStr::from_str(name), v) }
    }
    pub fn uniform_1f(&self, ctx: &GfxContext, name: &str, v: f32) {
        unsafe { (ctx.api().prog_uniform_1f)(self.handle, YogStr::from_str(name), v) }
    }
    pub fn uniform_2f(&self, ctx: &GfxContext, name: &str, [x, y]: [f32; 2]) {
        unsafe { (ctx.api().prog_uniform_2f)(self.handle, YogStr::from_str(name), x, y) }
    }
    pub fn uniform_3f(&self, ctx: &GfxContext, name: &str, [x, y, z]: [f32; 3]) {
        unsafe { (ctx.api().prog_uniform_3f)(self.handle, YogStr::from_str(name), x, y, z) }
    }
    pub fn uniform_4f(&self, ctx: &GfxContext, name: &str, [x, y, z, w]: [f32; 4]) {
        unsafe { (ctx.api().prog_uniform_4f)(self.handle, YogStr::from_str(name), x, y, z, w) }
    }
    /// Upload a column-major 4×4 matrix (16 contiguous `f32` values).
    pub fn uniform_mat4(&self, ctx: &GfxContext, name: &str, col_major: &[f32; 16]) {
        unsafe {
            (ctx.api().prog_uniform_mat4)(self.handle, YogStr::from_str(name), col_major.as_ptr())
        }
    }
}

// ── Texture ───────────────────────────────────────────────────────────────────

/// A 2-D GPU texture.
///
/// Created by [`GfxContext::create_texture_rgba`] or [`GfxContext::texture_from_mc`].
/// Must be deleted with [`GfxContext::delete_texture`] when you own it
/// (do **not** delete handles from `texture_from_mc` — Minecraft owns those).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub handle: u32,
}
