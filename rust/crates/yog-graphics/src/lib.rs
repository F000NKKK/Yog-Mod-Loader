//! yog-graphics — the graphics primitives every Yog crate shares, so a
//! colour, a rectangle or a picture is one type everywhere: [`Color`],
//! [`Vec2`]/[`Vec3`]/[`Mat4`], [`Rect`], vector [`Seg`] paths, RGBA
//! [`Image`]s, drawings ([`Op`]: paths filled and stroked, images) and a
//! [`Canvas`] that paints them to pixels.

pub mod color;
pub mod draw;
pub mod image;
pub mod math;
pub mod path;
pub mod raster;
pub mod rect;

pub use color::Color;
pub use draw::{Fill, ImageOp, Op, Stroke};
pub use image::{Image, PlacedImage};
pub use math::{Mat4, Vec2, Vec3, rad};
pub use path::Seg;
pub use raster::Canvas;
pub use rect::Rect;
