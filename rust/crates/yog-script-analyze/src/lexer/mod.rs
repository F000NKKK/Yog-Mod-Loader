//! Source text into tokens (`docs/design/ys-grammar.md` §1).

mod number;
pub(crate) mod string;

use yog_ids::script_problem;

use crate::source::token::{Punct, Span, Tok, Token};
use crate::words;
use yog_analysis::Report;

/// `src` cut into tokens, ending with `Eof`; what doesn't lex is reported.
pub fn lex(src: &str, report: &mut Report) -> Vec<Token> {
    let mut lexer = Lexer::new(src, 0, report);
    lexer.run(false);
    let end = Span::new(src.len(), src.len());
    let mut out = lexer.out;
    out.push(Token {
        tok: Tok::Eof,
        span: end,
    });
    out
}

pub(crate) struct Lexer<'s, 'r, 'a> {
    src: &'s str,
    pos: usize,
    out: Vec<Token>,
    report: &'r mut Report<'a>,
    /// The brackets open now: a `}` closing none ends an interpolation.
    open: Vec<Punct>,
}

const LINE_COMMENT: &str = "//";
const DOC_COMMENT: &str = "///";
const BLOCK_OPEN: &str = "/*";
const BLOCK_CLOSE: &str = "*/";
const RAW_QUOTES: &str = "\"\"\"";
/// Before a translatable text's key (`t"charge.title"`).
pub const TEXT_PREFIX: &str = "t\"";
use crate::source::token::QUOTE as CHAR_QUOTE;
/// Before a name: the name even when it's a keyword (`@this`).
pub const VERBATIM: char = '@';
/// Hexadecimal: `0x…` numbers, `\u{…}` escapes.
const HEX: u32 = 16;

impl<'s, 'r, 'a> Lexer<'s, 'r, 'a> {
    pub(crate) fn new(src: &'s str, pos: usize, report: &'r mut Report<'a>) -> Self {
        Lexer {
            src,
            pos,
            out: Vec::new(),
            report,
            open: Vec::new(),
        }
    }

    fn rest(&self) -> &'s str {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.rest().chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn push(&mut self, tok: Tok, start: usize) {
        self.out.push(Token {
            tok,
            span: Span::new(start, self.pos),
        });
    }

    /// Tokens until the end, or (`interpolation`) until a `}` closing
    /// nothing opened here; that `}` is left for the caller.
    pub(crate) fn run(&mut self, interpolation: bool) {
        while let Some(c) = self.peek() {
            // A hole's expression ends at its `}`, its alignment's `,` or its format's `:`.
            if interpolation && matches!(c, '}' | ',' | ':') && self.open.is_empty() {
                return;
            }
            self.token(c);
        }
    }

    fn token(&mut self, c: char) {
        let start = self.pos;
        match c {
            c if c.is_whitespace() => {
                self.bump();
            }
            '/' if self.rest().starts_with(LINE_COMMENT) => self.line_comment(start),
            '/' if self.rest().starts_with(BLOCK_OPEN) => self.block_comment(start),
            '0'..='9' => self.number(start),
            '"' if self.rest().starts_with(RAW_QUOTES) => self.raw_block(start),
            '"' => self.string(start, false),
            // `@"C:\mods\{name}"`: C#'s verbatim string (`$@`, as every string interpolates).
            VERBATIM if self.peek_at(1) == Some('"') => {
                self.bump();
                self.string(start, true);
            }
            CHAR_QUOTE if self.lifetime_follows() => self.lifetime(start),
            CHAR_QUOTE => self.char_literal(start),
            't' if self.rest().starts_with(TEXT_PREFIX) => {
                self.raw(start, TEXT_PREFIX.len(), Tok::Text)
            }
            c if is_ident_start(c) => self.ident(start),
            VERBATIM if self.verbatim_name_follows() => self.verbatim(start),
            VERBATIM => self.content(start),
            '#' if self.peek_at(1).is_some_and(is_ident_start) => self.tag(start),
            _ => self.punct(start, c),
        }
    }

    fn line_comment(&mut self, start: usize) {
        let doc = self.rest().starts_with(DOC_COMMENT);
        let end = self
            .rest()
            .find('\n')
            .map_or(self.src.len(), |n| self.pos + n);
        let text = &self.src[self.pos..end];
        self.pos = end;
        if doc {
            let line = text[DOC_COMMENT.len()..].trim().to_string();
            self.push(Tok::Doc(line), start);
        }
    }

    fn block_comment(&mut self, start: usize) {
        let mut depth = 0usize;
        while !self.rest().is_empty() {
            if self.rest().starts_with(BLOCK_OPEN) {
                depth += 1;
                self.pos += BLOCK_OPEN.len();
            } else if self.rest().starts_with(BLOCK_CLOSE) {
                depth -= 1;
                self.pos += BLOCK_CLOSE.len();
                if depth == 0 {
                    return;
                }
            } else {
                self.bump();
            }
        }
        let span = Span::new(start, self.pos);
        self.report.error(
            script_problem::UNTERMINATED,
            span,
            "this comment never ends (`*/`)",
        );
    }

    /// `@name` without a `:` after it: a name, keyword or not.
    fn verbatim_name_follows(&self) -> bool {
        let rest = &self.rest()[VERBATIM.len_utf8()..];
        let name_len = rest.find(|c: char| !is_ident_char(c)).unwrap_or(rest.len());
        name_len > 0 && !rest[name_len..].starts_with(':')
    }

    fn verbatim(&mut self, start: usize) {
        self.bump();
        let text = self.word().to_string();
        self.push(Tok::Ident(text), start);
    }

    fn ident(&mut self, start: usize) {
        let text = self.word();
        let tok = words::reserved(text).map_or_else(|| Tok::Ident(text.to_string()), Tok::Keyword);
        self.push(tok, start);
    }

    /// An identifier's characters from here.
    fn word(&mut self) -> &'s str {
        let from = self.pos;
        while self.peek().is_some_and(is_ident_char) {
            self.bump();
        }
        &self.src[from..self.pos]
    }

    /// A resource id's characters from here (`minecraft:logs`, `a/b.c`).
    fn res_id(&mut self) -> &'s str {
        let from = self.pos;
        while self.peek().is_some_and(is_res_char) {
            self.bump();
        }
        // A trailing `.` or `:` is what follows the id (`@item:a:b.count`).
        while self.pos > from && self.src[..self.pos].ends_with(['.', ':', '-', '/']) {
            self.pos -= 1;
        }
        &self.src[from..self.pos]
    }

    /// `@kind:id`, Yog Script's old content reference: said to be a
    /// typed id now (`new ItemId("minecraft:stick")`), read as its text.
    fn content(&mut self, start: usize) {
        self.bump();
        let kind = self.word();
        let id = if !kind.is_empty() && self.peek() == Some(':') {
            self.bump();
            self.res_id()
        } else {
            ""
        };
        let typed = format!("{}{}", yog_codegen::pascal(kind), yog_script_ids::system::CONTENT_ID_SUFFIX);
        let span = Span::new(start, self.pos);
        self.report.error(
            script_problem::BAD_REFERENCE,
            span,
            format!("content is named by a typed id: `new {typed}(\"{id}\")`; `@` starts a verbatim string or name"),
        );
        self.push(Tok::Str(vec![crate::source::token::StrPart::Text(id.to_string())]), start);
    }

    fn tag(&mut self, start: usize) {
        self.bump();
        let id = self.res_id().to_string();
        self.push(Tok::Tag(id), start);
    }

    fn punct(&mut self, start: usize, c: char) {
        let Some(p) = Punct::ALL
            .iter()
            .copied()
            .find(|p| self.rest().starts_with(p.text()))
        else {
            self.bump();
            let span = Span::new(start, self.pos);
            self.report.error(
                script_problem::UNEXPECTED_CHAR,
                span,
                format!("`{c}` doesn't start anything here"),
            );
            return;
        };
        self.pos += p.text().len();
        match p {
            Punct::LParen | Punct::LBracket | Punct::LBrace => self.open.push(p),
            Punct::RParen | Punct::RBracket | Punct::RBrace => {
                self.open.pop();
            }
            _ => {}
        }
        self.push(Tok::Punct(p), start);
    }

    pub(crate) fn into_tokens(self) -> (Vec<Token>, usize) {
        (self.out, self.pos)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_res_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-' | '/' | ':')
}
