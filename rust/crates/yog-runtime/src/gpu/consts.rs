//! Where compiled shaders are kept: the user's cache, by their sources' hash.

pub(super) const CACHE: &str = yog_ids::ns::YOG;
/// Compiled shaders, by their sources' hash.
pub(super) const SHADERS: &str = "shaders";
