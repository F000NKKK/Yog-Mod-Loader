//! The problems found in one source, as Diagnostics placed in it.

use std::fmt::Display;

use yog_data::{Diagnostic, LineCol, Span};

/// Collects one source's problems, each at its line and column.
pub struct Report<'a> {
    src: &'a str,
    file: &'a str,
    pub out: Vec<Diagnostic>,
}

impl<'a> Report<'a> {
    /// A report on `src`, the file named `file`.
    pub fn new(file: &'a str, src: &'a str) -> Self {
        Report {
            src,
            file,
            out: Vec::new(),
        }
    }

    /// An empty report on the same source: for a trial whose problems are
    /// dropped (a parse that may not fit).
    pub fn scratch(&self) -> Report<'a> {
        Report::new(self.file, self.src)
    }

    /// The source text at `span`.
    pub fn text(&self, span: Span) -> &'a str {
        self.src
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
    }

    /// The line (from 1) `span` starts on.
    pub fn line_of(&self, span: Span) -> u32 {
        LineCol::of(self.src, span.start as usize).line
    }

    /// An error of kind `code` at `span`.
    pub fn error(&mut self, code: &str, span: Span, message: impl Display) {
        self.push(Diagnostic::error(yog_data::Atom::new(code), message), span);
    }

    /// A warning of kind `code` at `span`.
    pub fn warning(&mut self, code: &str, span: Span, message: impl Display) {
        self.push(
            Diagnostic::warning(yog_data::Atom::new(code), message),
            span,
        );
    }

    /// `d`, placed at `span` of this file.
    fn push(&mut self, d: Diagnostic, span: Span) {
        let from = LineCol::of(self.src, span.start as usize);
        let to = LineCol::of(self.src, span.end as usize);
        let mut d = d.in_file(self.file).at(from);
        d.end_line = Some(to.line);
        d.end_col = Some(to.col);
        self.out.push(d);
    }
}

#[cfg(test)]
mod tests;
