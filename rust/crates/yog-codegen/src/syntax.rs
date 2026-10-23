//! What a language generated code is written in has to say for itself:
//! each concrete language (its own crate) implements the parts it has.

/// How the language writes comments.
pub trait Comments {
    /// `text` as a one-line comment (no line break).
    fn comment(text: &str) -> String;
    /// `text` as a one-line doc comment on what follows.
    fn doc(text: &str) -> String;
}

/// How the language writes string literals.
pub trait Literals {
    /// A string literal reading back as `text`.
    fn string(text: &str) -> String;
}

/// The language's names: its reserved words, and how a name spelled like
/// one is made an identifier.
pub trait Identifiers {
    /// The reserved words.
    const KEYWORDS: &'static [&'static str];

    /// `keyword` made an identifier.
    fn escape_keyword(keyword: &str) -> String;

    /// `name` as an identifier: escaped when it's a reserved word.
    fn ident(name: &str) -> String {
        if Self::KEYWORDS.contains(&name) {
            Self::escape_keyword(name)
        } else {
            name.to_string()
        }
    }
}

/// How the language nests: its blocks and their indentation.
pub trait Blocks {
    /// One level of indentation.
    const INDENT: &'static str;

    /// The line opening a block after `head`.
    fn open(head: &str) -> String;
    /// The line closing a block, then `tail`.
    fn close(tail: &str) -> String;
}
