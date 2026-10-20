//! Worlds generated.

/// A chunk column to fill, keyed by its generator type: the generator's
/// handler places blocks in it (`set_block`), the host sets them.
pub use yog_dimensions::GeneratedChunk as Chunk;
