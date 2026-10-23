//! The characters the cases and literals are made of.

/// What separates words in a name: `snake_case`, `kebab-case`, paths, dots, spaces.
pub(crate) const WORD_SEPARATORS: [char; 5] = ['_', '-', '/', '.', ' '];
/// Joins the words of `snake_case` and `SCREAMING_CASE`.
pub(crate) const SNAKE_JOIN: &str = "_";
/// One level of indentation: rustfmt's, the hosts' Java, the IDE's TypeScript.
pub(crate) const INDENT_RUST: &str = "    ";
pub(crate) const INDENT_JAVA: &str = "    ";
pub(crate) const INDENT_TS: &str = "  ";
/// The last character a literal may carry as itself; above, an escape.
pub(crate) const LAST_ASCII: char = '\u{7e}';
/// The first printable character; below, an escape.
pub(crate) const FIRST_PRINTABLE: char = ' ';
/// What makes a Rust keyword an identifier.
pub(crate) const RAW_IDENT: &str = "r#";
/// What makes a Java or TypeScript keyword an identifier, after it.
pub(crate) const ESCAPE_SUFFIX: &str = "_";
