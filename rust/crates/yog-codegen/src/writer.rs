//! An indenting writer for a language with blocks.

use std::marker::PhantomData;

use crate::syntax::{Blocks, Comments, Literals};

/// Generated code of language `L`, written a line at a time at the current depth.
pub struct Code<L> {
    out: String,
    depth: usize,
    lang: PhantomData<L>,
}

impl<L: Blocks + Comments + Literals> Code<L> {
    /// An empty file.
    pub fn new() -> Self {
        Self::starting(String::new())
    }

    /// A file starting with `text` (its header, say) as written.
    pub fn starting(text: impl Into<String>) -> Self {
        Self {
            out: text.into(),
            depth: 0,
            lang: PhantomData,
        }
    }

    /// A string literal of the language.
    pub fn str(&self, text: &str) -> String {
        L::string(text)
    }

    /// One line at the current depth.
    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        for _ in 0..self.depth {
            self.out.push_str(L::INDENT);
        }
        self.out.push_str(text.as_ref());
        self.out.push('\n');
        self
    }

    /// An empty line.
    pub fn blank(&mut self) -> &mut Self {
        self.out.push('\n');
        self
    }

    /// A doc comment on what follows.
    pub fn doc(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.line(L::doc(text.as_ref()))
    }

    /// A block after `head`, and what follows one level deeper.
    pub fn open(&mut self, head: impl AsRef<str>) -> &mut Self {
        self.line(L::open(head.as_ref()));
        self.depth += 1;
        self
    }

    /// The innermost block closed, then `tail`.
    pub fn close(&mut self, tail: &str) -> &mut Self {
        self.depth = self.depth.saturating_sub(1);
        self.line(L::close(tail))
    }

    /// The code written.
    pub fn finish(self) -> String {
        self.out
    }
}

impl<L: Blocks + Comments + Literals> Default for Code<L> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
