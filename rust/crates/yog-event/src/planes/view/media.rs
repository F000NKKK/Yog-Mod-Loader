//! Media as the game takes it: a `.yog`'s yimage handed to the game as PNG
//! (`yog_media::yimage`; the runtime answers).

use yog_data::with::Raw;

/// The PNG of a yimage.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Png {
    #[data(with = "Raw")]
    pub yimage: Vec<u8>,
}

/// A PNG's bytes (none: not a yimage).
#[derive(Debug, Clone, Default, yog_data::Schema)]
pub struct PngBytes {
    #[data(with = "Raw")]
    pub png: Vec<u8>,
}
