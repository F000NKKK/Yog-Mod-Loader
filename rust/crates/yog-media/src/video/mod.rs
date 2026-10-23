//! Video as the game plays it: AV1 packets (as the build writes them, see
//! `docs/design/video.md`) decoded into RGBA frames.

mod decoder;
pub mod yuv;

pub use decoder::{DecodeError, Decoder};
