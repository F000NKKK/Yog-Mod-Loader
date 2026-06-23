//! Core graphics constants and enumerations.

/// Primitive topology for draw calls.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrawMode {
    Triangles     = 0,
    Lines         = 1,
    LineStrip     = 2,
    TriangleStrip = 3,
    TriangleFan   = 4,
}

/// Vertex attribute component data type.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataType {
    /// 32-bit float (`GL_FLOAT`).
    F32 = 0,
    /// Unsigned byte, optionally normalized to 0.0–1.0 (`GL_UNSIGNED_BYTE`).
    U8  = 1,
    /// Signed 32-bit integer (`GL_INT`).
    I32 = 2,
    /// Unsigned 32-bit integer (`GL_UNSIGNED_INT`).
    U32 = 3,
}

/// OpenGL blend factor constants for use with [`crate::GfxContext::set_blend`].
pub mod blend {
    pub const ZERO:                u32 = 0;
    pub const ONE:                 u32 = 1;
    pub const SRC_COLOR:           u32 = 0x0300;
    pub const ONE_MINUS_SRC_COLOR: u32 = 0x0301;
    pub const SRC_ALPHA:           u32 = 0x0302;
    pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
    pub const DST_ALPHA:           u32 = 0x0304;
    pub const ONE_MINUS_DST_ALPHA: u32 = 0x0305;
    pub const DST_COLOR:           u32 = 0x0306;
    pub const ONE_MINUS_DST_COLOR: u32 = 0x0307;
}
