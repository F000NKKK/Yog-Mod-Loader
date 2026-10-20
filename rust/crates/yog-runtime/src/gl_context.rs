//! The OpenGL context (client side, render thread only).
use super::{OnceLock};


// ── OpenGL context (client-side, render thread only) ─────────────────────────

pub(super) struct GlCtx(pub(super) glow::Context);
unsafe impl Send for GlCtx {}
unsafe impl Sync for GlCtx {}

/// Initialized by `nativeGlInit` on the render thread.  `None` on dedicated server.
pub(super) static GL: OnceLock<GlCtx> = OnceLock::new();

// Raw GL function pointers for GL_ARB_get_program_binary (not exposed by glow 0.13).
// Captured during the glow loader callback in `nativeGlInit`.
// `None` when the extension is unavailable (very old drivers).
pub(super) static GL_GET_PROGRAM_BINARY: OnceLock<Option<usize>> = OnceLock::new();
pub(super) static GL_PROGRAM_BINARY: OnceLock<Option<usize>> = OnceLock::new();
pub(super) static GL_GET_PROGRAM_IV: OnceLock<Option<usize>> = OnceLock::new();
