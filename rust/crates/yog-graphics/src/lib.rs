//! yog-graphics — the graphics primitives every Yog crate shares, so a
//! color, a rectangle or a picture is one type everywhere: [`Color`],
//! [`Vec2`]/[`Vec3`]/[`Mat4`], [`Rect`], vector [`Seg`] paths, RGBA
//! [`Image`]s, drawings ([`Op`]: paths filled and stroked, images) and a
//! [`Canvas`] that paints them to pixels.

mod geometry;
mod painting;
mod pixels;

pub use geometry::{math, path, rect};
pub use painting::{draw, raster};
pub use pixels::{color, image, mask};

pub use geometry::math::{Mat4, Vec2, Vec3, rad};
pub use geometry::path::Seg;
pub use geometry::rect::Rect;
pub use painting::draw::{Fill, ImageOp, MaskOp, Op, Stroke};
pub use painting::raster::Canvas;
pub use pixels::color::Color;
pub use pixels::image::{BYTES_PER_PIXEL, Image, PlacedImage, Rgba8};
pub use pixels::mask::Mask;
