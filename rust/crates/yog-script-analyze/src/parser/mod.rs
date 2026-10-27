//! The parser core: a cursor over tokens, problems and recovery, and the
//! turn-taking — each form is parsed by its word (`words/`) or by a generic
//! form here (types, members, blocks, expressions by precedence).

mod await_using;
mod block;
mod decl;
pub(crate) mod expr;
mod header;
mod lambda;
mod member;
mod params;
pub(crate) mod patch;
pub(crate) mod pattern;
mod postfix;
mod primary;
mod query;
mod types;

use yog_ids::script_problem;

use crate::source::token::{Punct, Span, Tok, Token};
use crate::words::{Starts, Word};
use yog_analysis::Report;

/// A form that didn't parse: its problem is reported already.
#[derive(Debug)]
pub struct Fail;

pub type PResult<T> = Result<T, Fail>;

pub struct Parser<'t, 'r, 'a> {
    toks: &'t [Token],
    pos: usize,
    pub(crate) report: &'r mut Report<'a>,
    /// Patches are allowed (a `.yns` source).
    pub(crate) nanites: bool,
    /// A `{` here ends the expression (a system's period, a guard) instead
    /// of starting a trailing lambda.
    no_brace: bool,
}

impl<'t, 'r, 'a> Parser<'t, 'r, 'a> {
    /// Over `toks`, which end with `Eof`.
    pub fn new(toks: &'t [Token], report: &'r mut Report<'a>, nanites: bool) -> Self {
        Parser {
            toks,
            pos: 0,
            report,
            nanites,
            no_brace: false,
        }
    }

    /// How many tokens are read.
    pub(crate) fn consumed(&self) -> usize {
        self.pos
    }

    pub(crate) fn tok(&self) -> &'t Tok {
        self.nth(0)
    }

    /// The token `n` ahead (the last, `Eof`, past the end).
    pub(crate) fn nth(&self, n: usize) -> &'t Tok {
        let last = self.toks.len().saturating_sub(1);
        &self.toks[(self.pos + n).min(last)].tok
    }

    pub(crate) fn span(&self) -> Span {
        self.span_at(0)
    }

    /// Where the token `n` ahead is.
    pub(crate) fn span_at(&self, n: usize) -> Span {
        self.toks[(self.pos + n).min(self.toks.len() - 1)].span
    }

    /// Where the token before this one ends.
    pub(crate) fn prev(&self) -> Span {
        self.pos
            .checked_sub(1)
            .map_or_else(|| self.span(), |i| self.toks[i].span)
    }

    pub(crate) fn bump(&mut self) -> &'t Token {
        let t = &self.toks[self.pos.min(self.toks.len() - 1)];
        if t.tok != Tok::Eof {
            self.pos += 1;
        }
        t
    }

    pub(crate) fn at(&self, p: Punct) -> bool {
        *self.tok() == Tok::Punct(p)
    }

    /// Whether `p` comes next, right against the token here (`>>` is two
    /// `>` with nothing between).
    pub(crate) fn joined(&self, p: Punct) -> bool {
        self.nth_is(1, p) && self.span_at(0).end == self.span_at(1).start
    }

    pub(crate) fn nth_is(&self, n: usize, p: Punct) -> bool {
        *self.nth(n) == Tok::Punct(p)
    }

    pub(crate) fn eat(&mut self, p: Punct) -> bool {
        let here = self.at(p);
        if here {
            self.bump();
        }
        here
    }

    pub(crate) fn need(&mut self, p: Punct) -> PResult<Span> {
        if self.at(p) {
            return Ok(self.bump().span);
        }
        Err(self.expected(&format!("`{}`", p.text())))
    }

    /// The `;` ending a statement or a member.
    pub(crate) fn end(&mut self) -> PResult<()> {
        self.need(Punct::Semi).map(drop)
    }

    /// Whether `tok` is `word` (reserved or written as a name).
    pub(crate) fn is_word(tok: &Tok, word: &Word) -> bool {
        match tok {
            Tok::Keyword(w) => *w == word,
            Tok::Ident(text) => !word.reserved && text == word.text,
            _ => false,
        }
    }

    pub(crate) fn at_word(&self, word: &Word) -> bool {
        Self::is_word(self.tok(), word)
    }

    pub(crate) fn nth_word(&self, n: usize, word: &Word) -> bool {
        Self::is_word(self.nth(n), word)
    }

    pub(crate) fn eat_word(&mut self, word: &Word) -> bool {
        let here = self.at_word(word);
        if here {
            self.bump();
        }
        here
    }

    pub(crate) fn need_word(&mut self, word: &Word) -> PResult<Span> {
        if self.at_word(word) {
            return Ok(self.bump().span);
        }
        Err(self.expected(&format!("`{}`", word.text)))
    }

    /// Whether a form of `word` starts here.
    pub(crate) fn starts(&self, word: &Word, starts: Starts) -> bool {
        starts.map_or_else(|| self.at_word(word), |f| f(self))
    }

    /// `what` was expected here: reported.
    pub(crate) fn expected(&mut self, what: &str) -> Fail {
        let found = describe(self.tok());
        self.error(
            script_problem::EXPECTED,
            self.span(),
            format!("expected {what}, found {found}"),
        )
    }

    pub(crate) fn error(
        &mut self,
        code: &str,
        span: Span,
        message: impl std::fmt::Display,
    ) -> Fail {
        self.report.error(code, span, message);
        Fail
    }

    /// Past a form that failed: through its `;`, or through the `{ … }`
    /// that ended it, or up to a `}` closing what it's in.
    pub(crate) fn recover(&mut self) {
        let start = self.pos;
        let mut depth = 0usize;
        loop {
            match self.tok() {
                Tok::Eof => return,
                Tok::Punct(Punct::Semi) if depth == 0 => {
                    self.bump();
                    return;
                }
                Tok::Punct(Punct::LBrace | Punct::LParen | Punct::LBracket) => depth += 1,
                Tok::Punct(Punct::RBrace | Punct::RParen | Punct::RBracket) => {
                    if depth == 0 {
                        // A `)` or `]` nothing opened, met first: skipped, or
                        // what failed on it would fail on it again. A `}`
                        // closes the block around.
                        if self.pos == start && !self.at(Punct::RBrace) {
                            self.bump();
                        }
                        return;
                    }
                    depth -= 1;
                    if depth == 0 && self.at(Punct::RBrace) {
                        self.bump();
                        return;
                    }
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// `f` with `{` ending expressions (a system's period, a guard).
    pub(crate) fn no_brace<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let was = std::mem::replace(&mut self.no_brace, true);
        let out = f(self);
        self.no_brace = was;
        out
    }

    /// `f` with `{` allowed again (inside brackets and blocks).
    pub(crate) fn braces<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let was = std::mem::replace(&mut self.no_brace, false);
        let out = f(self);
        self.no_brace = was;
        out
    }
}

/// A token as a problem names it.
fn describe(tok: &Tok) -> String {
    match tok {
        Tok::Ident(name) => format!("`{name}`"),
        Tok::Keyword(w) => format!("`{}`", w.text),
        Tok::Punct(p) => format!("`{}`", p.text()),
        Tok::Int(_) | Tok::Float(_) | Tok::Unit(..) => "a number".into(),
        Tok::Version(v) => format!("the version `{v}`"),
        Tok::Str(_) | Tok::RawStr(_) | Tok::Text(_) => "a string".into(),
        Tok::Char(_) => "a character".into(),
        Tok::Lifetime(name) => format!("the lifetime `'{name}`"),
        Tok::Content(kind, id) => format!("`@{kind}:{id}`"),
        Tok::Tag(id) => format!("`#{id}`"),
        Tok::Doc(_) => "a doc comment".into(),
        Tok::Eof => "the end of the file".into(),
    }
}
