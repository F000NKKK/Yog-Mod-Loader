//! Strings: `"…"` with escapes and `{expr}` interpolation, raw `r"…"`,
//! translatable `t"key"`.

use yog_ids::script_problem;

use super::{HEX, Lexer};
use crate::source::token::{Span, StrPart, Tok, Token};

const QUOTE: char = '"';
const ESCAPE: char = '\\';
const OPEN: char = '{';
const CLOSE: char = '}';
const UNICODE_OPEN: &str = "u{";

impl Lexer<'_, '_, '_> {
    pub(super) fn string(&mut self, start: usize) {
        self.bump();
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            let Some(c) = self.bump() else {
                return self.unterminated(start, Tok::Str(parts));
            };
            match c {
                QUOTE => break,
                ESCAPE => self.escape(&mut text),
                OPEN | CLOSE if self.peek() == Some(c) => {
                    self.bump();
                    text.push(c);
                }
                OPEN => {
                    if !text.is_empty() {
                        parts.push(StrPart::Text(std::mem::take(&mut text)));
                    }
                    parts.push(StrPart::Expr(self.interpolation()));
                    if self.bump() != Some(CLOSE) {
                        return self.unterminated(start, Tok::Str(parts));
                    }
                }
                _ => text.push(c),
            }
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(StrPart::Text(text));
        }
        self.push(Tok::Str(parts), start);
    }

    /// The tokens of a `{expr}`, up to its `}`.
    fn interpolation(&mut self) -> Vec<Token> {
        let mut inner = Lexer::new(self.src, self.pos, &mut *self.report);
        inner.run(true);
        let (tokens, pos) = inner.into_tokens();
        self.pos = pos;
        tokens
    }

    fn escape(&mut self, text: &mut String) {
        let at = self.pos;
        let c = match self.bump() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('r') => '\r',
            Some('0') => '\0',
            Some(c @ (ESCAPE | QUOTE | OPEN | CLOSE)) => c,
            Some('u') if self.src[at..].starts_with(UNICODE_OPEN) => match self.unicode() {
                Some(c) => c,
                None => return self.bad_escape(at),
            },
            _ => return self.bad_escape(at),
        };
        text.push(c);
    }

    /// `{XXXX}` after `\u`.
    fn unicode(&mut self) -> Option<char> {
        self.bump();
        let from = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.bump();
        }
        let code = u32::from_str_radix(&self.src[from..self.pos], HEX).ok();
        if self.bump() != Some(CLOSE) {
            return None;
        }
        char::from_u32(code?)
    }

    fn bad_escape(&mut self, at: usize) {
        let span = Span::new(at.saturating_sub(1), self.pos);
        self.report.error(
            script_problem::EXPECTED,
            span,
            "an escape is one of \\n \\t \\r \\0 \\\\ \\\" \\{ \\} \\u{…}",
        );
    }

    /// `r"…"` or `t"…"`: the text up to the next `"`, as written.
    pub(super) fn raw(&mut self, start: usize, prefix: usize, tok: fn(String) -> Tok) {
        self.pos += prefix;
        let Some(len) = self.rest().find(QUOTE) else {
            self.pos = self.src.len();
            return self.unterminated(start, tok(String::new()));
        };
        let text = self.rest()[..len].to_string();
        self.pos += len + QUOTE.len_utf8();
        self.push(tok(text), start);
    }

    fn unterminated(&mut self, start: usize, tok: Tok) {
        let span = Span::new(start, self.pos);
        self.report.error(
            script_problem::UNTERMINATED,
            span,
            "this string never ends (`\"`)",
        );
        self.push(tok, start);
    }
}
