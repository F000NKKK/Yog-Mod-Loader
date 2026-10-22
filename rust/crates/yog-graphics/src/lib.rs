//! yog-graphics — the graphics primitives every Yog crate shares, so a
//! color, a rectangle or a picture is one type everywhere: [`Color`],
//! [`Vec2`]/[`Vec3`]/[`Mat4`], [`Rect`], vector [`Seg`] paths, RGBA
//! [`Image`]s, drawings ([`Op`]: paths filled and stroked, images) and a
//! [`Canvas`] that paints them to pixels.

pub mod color;
pub mod draw;
pub mod image;
pub mod mask;
pub mod math;
pub mod path;
pub mod raster;
pub mod rect;

pub use color::Color;
pub use draw::{Fill, ImageOp, MaskOp, Op, Stroke};
pub use image::{BYTES_PER_PIXEL, Image, PlacedImage, Rgba8};
pub use mask::Mask;
pub use math::{Mat4, Vec2, Vec3, rad};
pub use path::Seg;
pub use raster::Canvas;
pub use rect::Rect;
