//! Tables a crate ships, read as they're needed.

use substrate_data::{Schema, from_text};

/// A table a crate ships (`include_str!` of its `.ydata`), read: what it
/// says, or — when it won't read, which its crate's tests catch — the
/// error logged (`what`: which table) and its default.
pub fn built_in<T: Schema + Default>(what: &str, text: &str) -> T {
    match from_text::<T>(text) {
        Ok(read) => read.value,
        Err(e) => {
            yog_logging::error!("{what} won't read: {e}");
            T::default()
        }
    }
}
