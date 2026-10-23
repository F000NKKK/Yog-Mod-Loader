//! TOML, as the build writes it (`Cargo.toml`, `rust-toolchain.toml`): comments.

use yog_codegen::Comments;

/// The TOML language.
pub struct Toml;

impl Comments for Toml {
    fn comment(text: &str) -> String {
        format!("# {text}")
    }

    /// TOML has no doc comments: a comment it is.
    fn doc(text: &str) -> String {
        Self::comment(text)
    }
}
