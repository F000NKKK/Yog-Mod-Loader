//! What the lexer cuts a source into: tokens, each with where it is.

use crate::words::Word;

/// Where something is in its source: byte offsets, `start..end`.
pub use yog_data::Span;

/// What a character literal is between and a lifetime starts with
/// (`'R'`, `'a`).
pub const QUOTE: char = '\'';

/// A part of a string literal: text as written, or `{expr}` cut into tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    /// `{expr,alignment:format}`: the expression's tokens, then what pads
    /// and formats its value, as written (C#'s composite format).
    Expr {
        tokens: Vec<Token>,
        alignment: Option<String>,
        format: Option<String>,
    },
}

/// What a token is.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    /// A reserved word (`words/`).
    Keyword(&'static Word),
    Int(i64),
    Float(f64),
    /// A number with a unit (`20t`, `3b`, `100Energy`): the number as
    /// written, the unit's name.
    Unit(String, String),
    /// A version (`1.20.1`), as written.
    Version(String),
    Str(Vec<StrPart>),
    RawStr(String),
    Char(char),
    /// `'a`: a lifetime, by its name.
    Lifetime(String),
    /// A translatable text by key: `t"charge.tooltip"`.
    Text(String),
    /// `@kind:namespace:path`: the kind, the id.
    Content(String, String),
    /// `#namespace:path`.
    Tag(String),
    /// `/// …`: a declaration's documentation line.
    Doc(String),
    Punct(Punct),
    Eof,
}

/// A token and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

macro_rules! puncts {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $text:literal,)* }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            /// How it's written.
            #[must_use]
            pub const fn text(self) -> &'static str {
                match self { $($name::$variant => $text),* }
            }
        }
    };
}

puncts! {
    /// Punctuation and operators, longest first where one starts another.
    Punct {
        RangeExcl = "..<", NullAssign = "??=", ShlEq = "<<=",
        FatArrow = "=>", SafeDot = "?.", Coalesce = "??",
        EqEq = "==", NotEq = "!=", Shl = "<<", LtEq = "<=", GtEq = ">=", AndAnd = "&&", OrOr = "||",
        PlusEq = "+=", MinusEq = "-=", StarEq = "*=", SlashEq = "/=", PercentEq = "%=",
        AmpEq = "&=", PipeEq = "|=", CaretEq = "^=", PlusPlus = "++", MinusMinus = "--",
        Range = "..",
        LBrace = "{", RBrace = "}", LParen = "(", RParen = ")", LBracket = "[", RBracket = "]",
        Comma = ",", Dot = ".", Colon = ":", Semi = ";", Question = "?",
        Eq = "=", Lt = "<", Gt = ">", Plus = "+", Minus = "-", Star = "*", Slash = "/",
        Percent = "%", Bang = "!", Amp = "&", Hash = "#", Dollar = "$",
        Pipe = "|", Caret = "^", Tilde = "~",
    }
}
