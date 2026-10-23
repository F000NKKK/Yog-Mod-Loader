//! The languages generated code is written in, and what differs between them.

use crate::consts::{ESCAPE_SUFFIX, INDENT_JAVA, INDENT_RUST, INDENT_TS, RAW_IDENT};
use crate::keywords;
use crate::literal::{java_str, rust_str, ts_str};

/// A language code is generated in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Java,
    TypeScript,
}

impl Lang {
    /// One level of indentation, as its code is formatted.
    pub const fn indent(self) -> &'static str {
        match self {
            Lang::Rust => INDENT_RUST,
            Lang::Java => INDENT_JAVA,
            Lang::TypeScript => INDENT_TS,
        }
    }

    /// A string literal reading back as `text`.
    pub fn str(self, text: &str) -> String {
        match self {
            Lang::Rust => rust_str(text),
            Lang::Java => java_str(text),
            Lang::TypeScript => ts_str(text),
        }
    }

    /// The language's reserved words.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Rust => keywords::RUST,
            Lang::Java => keywords::JAVA,
            Lang::TypeScript => keywords::TYPESCRIPT,
        }
    }

    /// `name` as an identifier: a reserved word escaped (`r#type` in Rust,
    /// `interface_` in Java and TypeScript).
    pub fn ident(self, name: &str) -> String {
        if !self.keywords().contains(&name) {
            return name.to_string();
        }
        match self {
            Lang::Rust => format!("{RAW_IDENT}{name}"),
            Lang::Java | Lang::TypeScript => format!("{name}{ESCAPE_SUFFIX}"),
        }
    }
}
