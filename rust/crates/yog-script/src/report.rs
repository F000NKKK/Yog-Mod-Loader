//! The problems found in one source, as Diagnostics placed in it.

use yog_data::{Diagnostic, LineCol};

use crate::token::Span;

/// Collects one source's problems, each at its line and column.
pub struct Report<'a> {
    src: &'a str,
    file: &'a str,
    pub out: Vec<Diagnostic>,
}

impl<'a> Report<'a> {
    /// The source text at `span`.
    pub fn text(&self, span: Span) -> &'a str {
        self.src
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
    }

    pub fn new(file: &'a str, src: &'a str) -> Self {
        Report {
            src,
            file,
            out: Vec::new(),
        }
    }

    /// The line (from 1) `span` starts on.
    pub fn line_of(&self, span: Span) -> u32 {
        LineCol::of(self.src, span.start as usize).line
    }

    /// An error of kind `code` at `span`.
    pub fn error(&mut self, code: &str, span: Span, message: impl std::fmt::Display) {
        let from = LineCol::of(self.src, span.start as usize);
        let to = LineCol::of(self.src, span.end as usize);
        let mut d = Diagnostic::error(code, message).in_file(self.file).at(from);
        d.end_line = Some(to.line);
        d.end_col = Some(to.col);
        self.out.push(d);
    }
}
