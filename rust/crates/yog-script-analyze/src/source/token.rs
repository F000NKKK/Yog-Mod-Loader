//! What the lexer cuts a source into: tokens, each with where it is.

use crate::words::Word;

/// Where something is in its source: byte offsets, `start..end`.
pub use yog_data::Span;

/// A part of a string literal: text as written, or `{expr}` cut into tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    Expr(Vec<Token>),
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
    /// A translatable text by key: `t"charge.tooltip"`.
    Text(String),
    /// `@kind:namespace:path`: the kind, the id.
    Content(String, String),
    /// `#namespace:path`.
    Tag(String),
    /// `/// …`: a declaration's documentation line.
    Doc(String),
    Punct(Punct),
    /// The end of a statement (a newline that isn't a continuation).
    Newline,
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
        RangeExcl = "..<", NullAssign = "??=",
        Arrow = "->", FatArrow = "=>", SafeDot = "?.", Coalesce = "??",
        EqEq = "==", NotEq = "!=", LtEq = "<=", GtEq = ">=", AndAnd = "&&", OrOr = "||",
        PlusEq = "+=", MinusEq = "-=", StarEq = "*=", SlashEq = "/=", PercentEq = "%=",
        Range = "..",
        LBrace = "{", RBrace = "}", LParen = "(", RParen = ")", LBracket = "[", RBracket = "]",
        Comma = ",", Dot = ".", Colon = ":", Semi = ";", Question = "?",
        Eq = "=", Lt = "<", Gt = ">", Plus = "+", Minus = "-", Star = "*", Slash = "/",
        Percent = "%", Bang = "!", Pipe = "|", Hash = "#",
    }
}

impl Punct {
    /// After it a newline doesn't end the statement: the line goes on.
    #[must_use]
    pub const fn continues(self) -> bool {
        !matches!(
            self,
            Punct::RBrace | Punct::RParen | Punct::RBracket | Punct::Semi | Punct::Question
        )
    }
}
