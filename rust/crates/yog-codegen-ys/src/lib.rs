//! Yog Script, as `yog-codegen` writes it: the bindings a mod's code
//! reaches its generated Rust by (content ids, `.yui` code-behind).
//! Spelled as its reader reads it (`yog_script_analyze::spelling`).

use std::fmt::Write as _;

use yog_codegen::{Blocks, Comments, Identifiers, Literals};
use yog_script_analyze::spelling::{
    CLOSE, ESCAPE, ESCAPES, OPEN, QUOTE, TEXT_PREFIX, UNICODE_OPEN, VERBATIM, is_keyword,
};
use yog_script_analyze::words::{class, public, r#const, r#static, string};

/// The Yog Script language.
pub struct Ys;

impl Ys {
    /// `public static class Name`: a class holding constants, as a head
    /// [`Blocks::open`] opens.
    pub fn static_class(name: &str) -> String {
        format!(
            "{} {} {} {}",
            public::WORD.text,
            r#static::WORD.text,
            class::WORD.text,
            Self::ident(name)
        )
    }

    /// `public const string Name = "value";`.
    pub fn const_string(name: &str, value: &str) -> String {
        Self::const_value(string::WORD.text, name, &Self::string(value))
    }

    /// `public const ty Name = literal;`: `ty` a built-in type's word.
    pub fn const_value(ty: &str, name: &str, literal: &str) -> String {
        format!(
            "{} {} {ty} {} = {literal};",
            public::WORD.text,
            r#const::WORD.text,
            Self::ident(name)
        )
    }

    /// `public static string Name => t"key";`: a translatable text, by
    /// its key.
    pub fn text_getter(name: &str, key: &str) -> String {
        let quoted = Self::string(key);
        let unquoted = quoted.strip_prefix(QUOTE).unwrap_or(&quoted);
        format!(
            "{} {} {} {} => {TEXT_PREFIX}{unquoted};",
            public::WORD.text,
            r#static::WORD.text,
            string::WORD.text,
            Self::ident(name)
        )
    }
}

impl Comments for Ys {
    fn comment(text: &str) -> String {
        format!("// {text}")
    }

    fn doc(text: &str) -> String {
        format!("/// {text}")
    }
}

impl Literals for Ys {
    /// What the reader takes for itself only escaped: the quote, `\`, an
    /// interpolation's braces, the escapes' characters; other control
    /// characters by their code.
    fn string(text: &str) -> String {
        let mut out = String::from(QUOTE);
        for c in text.chars() {
            if let Some((letter, _)) = ESCAPES.iter().find(|(_, e)| *e == c) {
                out.push(ESCAPE);
                out.push(*letter);
            } else if matches!(c, QUOTE | OPEN | CLOSE) || c == ESCAPE {
                out.push(ESCAPE);
                out.push(c);
            } else if c.is_control() {
                let _ = write!(out, "{ESCAPE}{UNICODE_OPEN}{:x}{CLOSE}", u32::from(c));
            } else {
                out.push(c);
            }
        }
        out.push(QUOTE);
        out
    }
}

impl Identifiers for Ys {
    fn is_keyword(name: &str) -> bool {
        is_keyword(name)
    }

    fn escape_keyword(keyword: &str) -> String {
        format!("{VERBATIM}{keyword}")
    }
}

impl Blocks for Ys {
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
