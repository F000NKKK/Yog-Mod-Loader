//! Numbers: integers (decimal, `0x`, `0b`), floats, versions (`1.20.1`),
//! and numbers with a unit (`20t`, `100Energy`).

use yog_ids::script_problem;

use super::{HEX, Lexer, is_ident_char, is_ident_start};
use crate::source::token::{Span, Tok};

const HEX_PREFIX: &str = "0x";
const BIN_PREFIX: &str = "0b";
const BIN: u32 = 2;
const DIGIT_SEP: char = '_';
/// What makes a number a `float` or a `double` (`2.0f`, `1d`), as in C#.
const FLOAT_SUFFIXES: [&str; 2] = ["f", "d"];

impl Lexer<'_, '_, '_> {
    pub(super) fn number(&mut self, start: usize) {
        for (prefix, radix) in [(HEX_PREFIX, HEX), (BIN_PREFIX, BIN)] {
            if self.rest().starts_with(prefix) {
                self.pos += prefix.len();
                return self.radix(start, radix);
            }
        }
        self.digits();
        let mut dots = 0;
        while self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
            self.digits();
            dots += 1;
        }
        if dots > 1 {
            let text = self.src[start..self.pos].to_string();
            return self.push(Tok::Version(text), start);
        }
        if dots == 1 && self.exponent_follows() {
            self.bump();
            if matches!(self.peek(), Some('+' | '-')) {
                self.bump();
            }
            self.digits();
        }
        let text = self.src[start..self.pos].replace(DIGIT_SEP, "");
        if self.peek().is_some_and(is_ident_start) {
            let unit = self.word().to_string();
            if FLOAT_SUFFIXES.contains(&unit.as_str()) {
                let tok = text.parse().ok().map(Tok::Float);
                return self.number_or_problem(tok, start);
            }
            return self.push(Tok::Unit(text, unit), start);
        }
        let tok = if dots == 0 {
            text.parse().ok().map(Tok::Int)
        } else {
            text.parse().ok().map(Tok::Float)
        };
        self.number_or_problem(tok, start);
    }

    fn radix(&mut self, start: usize, radix: u32) {
        let from = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_digit(radix) || c == DIGIT_SEP)
        {
            self.bump();
        }
        let digits = self.src[from..self.pos].replace(DIGIT_SEP, "");
        let tok = i64::from_str_radix(&digits, radix).ok().map(Tok::Int);
        if self.peek().is_some_and(is_ident_char) {
            self.word();
            return self.number_or_problem(None, start);
        }
        self.number_or_problem(tok, start);
    }

    fn number_or_problem(&mut self, tok: Option<Tok>, start: usize) {
        match tok {
            Some(tok) => self.push(tok, start),
            None => {
                let span = Span::new(start, self.pos);
                let text = &self.src[start..self.pos];
                self.report.error(
                    script_problem::BAD_NUMBER,
                    span,
                    format!("`{text}` isn't a number"),
                );
                self.push(Tok::Int(0), start);
            }
        }
    }

    fn digits(&mut self) {
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || c == DIGIT_SEP)
        {
            self.bump();
        }
    }

    /// `e`/`E`, an optional sign, then a digit: an exponent, not a unit.
    fn exponent_follows(&self) -> bool {
        let signed = matches!(self.peek_at(1), Some('+' | '-'));
        let digit_at = if signed { 2 } else { 1 };
        matches!(self.peek(), Some('e' | 'E'))
            && self.peek_at(digit_at).is_some_and(|c| c.is_ascii_digit())
    }
}
