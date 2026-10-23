//! The text forms: Yog Data's syntax, JSON and TOML read into it, text
//! edited in place, source files as they're kept.

#[cfg(feature = "json")]
pub mod json;
pub mod patch;
pub mod source;
pub mod text;
#[cfg(feature = "toml")]
pub mod toml;
