//! Java, as `yog-codegen` writes it: the hosts' code.

mod keywords;

use yog_codegen::quote::{quoted, utf16_units};
use yog_codegen::{Blocks, Comments, Identifiers, Literals};

/// The Java language.
pub struct Java;

/// What a string literal is between.
const QUOTE: char = '"';
/// What starts a UTF-16 unit's escape.
const UNIT_ESCAPE: &str = "\\u";
/// What makes a reserved word an identifier, after it.
const KEYWORD_SUFFIX: &str = "_";
/// The printable ASCII a literal carries as itself.
const PRINTABLE: std::ops::RangeInclusive<char> = ' '..='~';

impl Comments for Java {
    fn comment(text: &str) -> String {
        format!("// {text}")
    }

    fn doc(text: &str) -> String {
        format!("/** {text} */")
    }
}

impl Literals for Java {
    /// Control and non-ASCII characters as `\uXXXX`, so the source reads
    /// the same in any encoding.
    fn string(text: &str) -> String {
        quoted(text, QUOTE, |c| match c {
            '"' => Some("\\\"".into()),
            '\\' => Some("\\\\".into()),
            '\n' => Some("\\n".into()),
            '\r' => Some("\\r".into()),
            '\t' => Some("\\t".into()),
            c if PRINTABLE.contains(&c) => None,
            c => Some(utf16_units(c, UNIT_ESCAPE)),
        })
    }
}

impl Identifiers for Java {
    fn is_keyword(name: &str) -> bool {
        keywords::ALL.contains(&name)
    }

    fn escape_keyword(keyword: &str) -> String {
        format!("{keyword}{KEYWORD_SUFFIX}")
    }
}

impl Blocks for Java {
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
