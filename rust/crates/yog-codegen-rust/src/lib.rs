//! Rust, as `yog-codegen` writes it: mods' generated code.

mod keywords;

use yog_codegen::{Blocks, Comments, Identifiers, Literals};

/// The Rust language.
pub struct Rust;

/// What makes a keyword an identifier.
const RAW_IDENT: &str = "r#";

impl Rust {
    /// `text` as an inner doc comment: a module's own docs.
    pub fn inner_doc(text: &str) -> String {
        format!("//! {text}")
    }
}

impl Comments for Rust {
    fn comment(text: &str) -> String {
        format!("// {text}")
    }

    fn doc(text: &str) -> String {
        format!("/// {text}")
    }
}

impl Literals for Rust {
    fn string(text: &str) -> String {
        format!("{text:?}")
    }
}

impl Identifiers for Rust {
    fn is_keyword(name: &str) -> bool {
        keywords::ALL.contains(&name)
    }

    fn escape_keyword(keyword: &str) -> String {
        format!("{RAW_IDENT}{keyword}")
    }
}

impl Blocks for Rust {
    const INDENT: &'static str = "    ";

    fn open(head: &str) -> String {
        format!("{head} {{")
    }

    fn close(tail: &str) -> String {
        format!("}}{tail}")
    }
}

#[cfg(test)]
mod tests;
