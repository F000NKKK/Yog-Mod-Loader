//! Graphics: the frame's GFX table and 2D drawing through the game, raw GL
//! and the GPU pipeline, images, SVGs, the textures the runtime makes,
//! Yog's fonts.

pub(crate) mod fonts;
pub(crate) mod frame;
pub(crate) mod gl_context;
pub(crate) mod gpu;
pub(crate) mod gpu_record;
pub(crate) mod images;
pub(crate) mod svg;
pub(crate) mod table;
pub(crate) mod textures;
