//! An indenting writer for the brace-block languages (Rust, Java, TypeScript).

use crate::lang::Lang;

/// Generated code, written a line at a time at the current depth.
pub struct Code {
    lang: Lang,
    out: String,
    depth: usize,
}

impl Code {
    /// An empty file of `lang`.
    pub fn new(lang: Lang) -> Self {
        Self::starting(lang, String::new())
    }

    /// A file of `lang` starting with `text` (its header, say) as written.
    pub fn starting(lang: Lang, text: impl Into<String>) -> Self {
        Self {
            lang,
            out: text.into(),
            depth: 0,
        }
    }

    /// A string literal of the file's language.
    pub fn str(&self, text: &str) -> String {
        self.lang.str(text)
    }

    /// One line at the current depth.
    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        for _ in 0..self.depth {
            self.out.push_str(self.lang.indent());
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

    /// A `/** … */` doc comment, on one line.
    pub fn doc(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.line(format!("/** {} */", text.as_ref()))
    }

    /// `head {`, and what follows one level deeper.
    pub fn open(&mut self, head: impl AsRef<str>) -> &mut Self {
        self.line(format!("{} {{", head.as_ref()));
        self.depth += 1;
        self
    }

    /// `}` closing the innermost block, then `tail` (`;`, `)`, nothing).
    pub fn close(&mut self, tail: &str) -> &mut Self {
        self.depth = self.depth.saturating_sub(1);
        self.line(format!("}}{tail}"))
    }

    /// The code written.
    pub fn finish(self) -> String {
        self.out
    }
}

#[cfg(test)]
mod tests;
