//! yog-media — media as the game takes it, whatever it was made as: the
//! build and the IDE read a mod's sources through here, so both see the
//! same picture. Pictures: any format [`image::EXTENSIONS`] lists, decoded
//! to RGBA, written as PNG; SVG documents rasterized. Video: AV1 packets
//! decoded into RGBA frames.

pub mod image;
pub mod svg;
pub mod video;
pub mod yimage;
