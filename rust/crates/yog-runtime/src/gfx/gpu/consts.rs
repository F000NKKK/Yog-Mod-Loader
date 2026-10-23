//! Where compiled shaders are kept: the user's cache, by their sources' hash.

pub(crate) const CACHE: &str = yog_ids::ns::YOG;
/// Compiled shaders, by their sources' hash.
pub(crate) const SHADERS: &str = "shaders";
