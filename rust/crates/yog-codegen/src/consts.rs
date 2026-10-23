//! The characters names are cut at and joined with.

/// What separates words in a name: `snake_case`, `kebab-case`, paths, dots, spaces.
pub(crate) const WORD_SEPARATORS: [char; 5] = ['_', '-', '/', '.', ' '];
/// Joins the words of `snake_case` and `SCREAMING_CASE`.
pub(crate) const SNAKE_JOIN: &str = "_";
