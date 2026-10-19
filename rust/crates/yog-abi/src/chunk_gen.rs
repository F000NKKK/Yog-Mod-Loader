//! The chunk generator's callback table.

use super::*;

// ── Chunk generator callback (ABI minor 29) ───────────────────────────────────

/// Per-callback context handed to a registered chunk generator (see
/// `YogApi::register_chunk_generator`) — mirrors `YogGfxApi`'s pattern of
/// bundling per-call context with the function pointer(s) needed to act on
/// it. Valid only for the duration of the generator callback.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogChunkWriterApi {
    /// Chunk X coordinate (in chunks, not blocks).
    pub chunk_x: i32,
    /// Chunk Z coordinate (in chunks, not blocks).
    pub chunk_z: i32,
    /// Minimum build height (world Y) for this dimension.
    pub min_y: i32,
    /// Total world height for this dimension.
    pub height: i32,
    /// Opaque host-side handle for the chunk currently being generated.
    pub ctx: *mut c_void,
    /// Set a block within this chunk column. `x`/`z` are local (0..16)
    /// chunk-relative coordinates; `y` is world-absolute.
    pub set_block:
        unsafe extern "C" fn(ctx: *mut c_void, x: i32, y: i32, z: i32, block_id: YogStr) -> bool,
}

unsafe impl Send for YogChunkWriterApi {}
unsafe impl Sync for YogChunkWriterApi {}

/// Called by the host once per chunk column that needs generating in the
/// registered dimension. `writer` is valid only for the duration of the call.
pub type YogChunkGeneratorFn =
    unsafe extern "C" fn(ud: *mut c_void, writer: *const YogChunkWriterApi);
