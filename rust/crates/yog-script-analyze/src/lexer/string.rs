//! Strings: `"…"` with escapes and `{expr}` interpolation, raw `"""…"""`,
//! translatable `t"key"`, characters `'c'`.

use yog_ids::script_problem;

use super::{HEX, Lexer};
use crate::source::token::{Span, StrPart, Tok, Token};

pub const QUOTE: char = '"';
const RAW_QUOTES: &str = "\"\"\"";
pub use crate::source::token::QUOTE as CHAR_QUOTE;
pub const ESCAPE: char = '\\';
/// A string's interpolation's ends (`{name}`); doubled, the character.
pub const OPEN: char = '{';
pub const CLOSE: char = '}';
/// What starts a hole's alignment (`{n,5}`) and its format (`{x:F2}`).
pub const ALIGNMENT: char = ',';
pub const FORMAT: char = ':';
/// A character by its code, after `\\`: `u{e9}`.
pub const UNICODE_OPEN: &str = "u{";
/// The escapes after `\\` of a character each: the letter, the character.
pub const ESCAPES: [(char, char); 4] = [('n', '\n'), ('t', '\t'), ('r', '\r'), ('0', '\0')];

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
                    let tokens = self.interpolation();
                    let alignment = self.hole_part(ALIGNMENT, &[FORMAT, CLOSE]);
                    let format = self.hole_part(FORMAT, &[CLOSE]);
                    parts.push(StrPart::Expr {
                        tokens,
                        alignment,
                        format,
                    });
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

    /// A hole's part after `lead` (`,5`, `:F2`), as written, up to one of
    /// `ends`; none when it hasn't one.
    fn hole_part(&mut self, lead: char, ends: &[char]) -> Option<String> {
        if self.peek() != Some(lead) {
            return None;
        }
        self.bump();
        let from = self.pos;
        while self.peek().is_some_and(|c| !ends.contains(&c)) {
            self.bump();
        }
        Some(self.src[from..self.pos].trim().to_string())
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
        if let Some(c) = self.escaped() {
            text.push(c);
        }
    }

    /// The character an escape after `\\` stands for; a bad one reported.
    fn escaped(&mut self) -> Option<char> {
        let at = self.pos;
        let c = match self.bump() {
            Some(c @ (ESCAPE | QUOTE | CHAR_QUOTE | OPEN | CLOSE)) => c,
            Some('u') if self.src[at..].starts_with(UNICODE_OPEN) => match self.unicode() {
                Some(c) => c,
                None => {
                    self.bad_escape(at);
                    return None;
                }
            },
            letter => match ESCAPES.iter().find(|(e, _)| Some(*e) == letter) {
                Some((_, c)) => *c,
                None => {
                    self.bad_escape(at);
                    return None;
                }
            },
        };
        Some(c)
    }

    /// `'c'` or `'\\n'`.
    pub(super) fn char_literal(&mut self, start: usize) {
        self.bump();
        let c = match self.bump() {
            Some(ESCAPE) => self.escaped(),
            Some(c) if c != CHAR_QUOTE => Some(c),
            _ => None,
        };
        if self.peek() != Some(CHAR_QUOTE) {
            let span = Span::new(start, self.pos);
            self.report.error(
                script_problem::UNTERMINATED,
                span,
                "a character is one, between `'`s: `'R'`",
            );
            return self.push(Tok::Char(c.unwrap_or_default()), start);
        }
        self.bump();
        self.push(Tok::Char(c.unwrap_or_default()), start);
    }

    /// `'a` (not `'a'`): a name after the quote, no quote after the name.
    pub(super) fn lifetime_follows(&self) -> bool {
        if !self.peek_at(1).is_some_and(super::is_ident_start) {
            return false;
        }
        let name = self.rest()[1..]
            .chars()
            .take_while(|c| super::is_ident_char(*c))
            .count();
        self.peek_at(name + 1) != Some(CHAR_QUOTE)
    }

    pub(super) fn lifetime(&mut self, start: usize) {
        self.bump();
        let name = self.word().to_string();
        self.push(Tok::Lifetime(name), start);
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
            "an escape is one of \\n \\t \\r \\0 \\\\ \\\" \\' \\{ \\} \\u{…}",
        );
    }

    /// `"""…"""`: the text up to the next `"""`, as written, lines included.
    pub(super) fn raw_block(&mut self, start: usize) {
        self.pos += RAW_QUOTES.len();
        let Some(len) = self.rest().find(RAW_QUOTES) else {
            self.pos = self.src.len();
            return self.unterminated(start, Tok::RawStr(String::new()));
        };
        let text = self.rest()[..len].to_string();
        self.pos += len + RAW_QUOTES.len();
        self.push(Tok::RawStr(text), start);
    }

    /// `t"…"`: the text up to the next `"`, as written.
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
