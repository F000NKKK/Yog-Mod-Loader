//! What a tool finds wrong — reading a document, building a mod, serving a
//! request — each as one record: what it is (a code), how bad, what's
//! wrong and where (a file, its line and column, a value's path in it).
//! The one kind of problem every Yog tool says and every Yog tool reads.

use std::fmt;

use crate::{Atom, LineCol, Path, Schema, SyntaxError};

/// How bad it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Schema)]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Note,
}

/// One thing found.
#[derive(Debug, Clone, PartialEq, Eq, Default, Schema)]
#[data(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: Severity,
    /// What it is: one of `yog_ids`' problem codes (`YD0001`, `YB0002`), or
    /// the tool's own (`E0308`, a lint's name); none until it's known.
    pub code: Option<Atom>,
    pub message: String,
    /// The file it's in.
    pub file: Option<Atom>,
    /// The value it's about, in its document (`entries[2].icon`).
    #[data(default)]
    pub path: Path,
    /// About the key at `path` rather than its value (an unknown field).
    #[data(default)]
    pub on_key: bool,
    /// 1-based.
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub end_line: Option<u32>,
    pub end_col: Option<u32>,
    /// Who found it, when not Yog itself (`rustc`, `cargo`).
    pub tool: Option<Atom>,
}

impl Diagnostic {
    /// An error of kind `code`.
    pub fn error(code: &str, message: impl fmt::Display) -> Self {
        Self::of(Severity::Error, code, message)
    }

    /// A warning of kind `code`.
    pub fn warning(code: &str, message: impl fmt::Display) -> Self {
        Self::of(Severity::Warning, code, message)
    }

    pub fn of(severity: Severity, code: &str, message: impl fmt::Display) -> Self {
        Diagnostic {
            severity,
            code: Some(Atom::new(code)),
            message: message.to_string(),
            ..Self::default()
        }
    }

    /// Text that doesn't parse, where it stops.
    pub fn syntax(e: SyntaxError) -> Self {
        Self::error(yog_ids::data_problem::SYNTAX, e.message).at(e.at)
    }

    /// In `file`.
    #[must_use]
    pub fn in_file(mut self, file: impl fmt::Display) -> Self {
        self.file = Some(Atom::new(&file.to_string()));
        self
    }

    /// At `line` of its file.
    #[must_use]
    pub fn at_line(mut self, line: usize) -> Self {
        self.line = u32::try_from(line).ok();
        self
    }

    /// At `at` (a line and column) of its file.
    #[must_use]
    pub fn at(mut self, at: LineCol) -> Self {
        self.line = Some(at.line);
        self.col = Some(at.col);
        self
    }

    /// About the value at `path`.
    #[must_use]
    pub fn at_path(mut self, path: Path) -> Self {
        self.path = path;
        self
    }

    /// Found by `tool`.
    #[must_use]
    pub fn by(mut self, tool: &str) -> Self {
        self.tool = Some(Atom::new(tool));
        self
    }

    /// Of kind `code` when its kind isn't known yet (it was met as plain text).
    #[must_use]
    pub fn or_code(mut self, code: &str) -> Self {
        self.code.get_or_insert_with(|| Atom::new(code));
        self
    }

    /// Its message `wrap`ped (`<Slot> {e}`), the rest kept.
    #[must_use]
    pub fn map(mut self, wrap: impl FnOnce(String) -> String) -> Self {
        self.message = wrap(self.message);
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// As text: `file:line:col: path: message`, each part when it's known.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.file, self.line, self.col) {
            (Some(file), Some(line), Some(col)) => write!(f, "{file}:{line}:{col}: ")?,
            (Some(file), Some(line), None) => write!(f, "{file}:{line}: ")?,
            (Some(file), None, _) => write!(f, "{file}: ")?,
            (None, Some(line), Some(col)) => write!(f, "{line}:{col}: ")?,
            (None, Some(line), None) => write!(f, "line {line}: ")?,
            (None, None, _) => {}
        }
        if !self.path.is_root() {
            write!(f, "{}: ", self.path)?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for Diagnostic {}

/// An error met as plain text: its kind not known yet ([`Diagnostic::or_code`]).
impl From<String> for Diagnostic {
    fn from(message: String) -> Self {
        Diagnostic {
            message,
            ..Self::default()
        }
    }
}

impl From<&str> for Diagnostic {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

/// As text, for what says errors as text.
impl From<Diagnostic> for String {
    fn from(d: Diagnostic) -> Self {
        d.to_string()
    }
}

/// Every problem found reading a document, in its order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errors(pub Vec<Diagnostic>);

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Errors {}

impl From<SyntaxError> for Errors {
    fn from(e: SyntaxError) -> Self {
        Errors(vec![Diagnostic::syntax(e)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn said_with_what_is_known_and_a_frame_reads_back() {
        let d = Diagnostic::error("YB0002", "no model")
            .in_file("content/ruby.yblock")
            .at_line(3)
            .at_path(Path::root().key("model"));
        assert_eq!(d.to_string(), "content/ruby.yblock:3: model: no model");
        let mut bytes = Vec::new();
        crate::io::write_frame(&mut bytes, &d).unwrap();
        let back: Diagnostic = crate::io::read_frame(&mut bytes.as_slice())
            .unwrap()
            .unwrap();
        assert_eq!(back, d);
        assert_eq!(
            Diagnostic::from("bad").or_code("YB0004").code.as_deref(),
            Some("YB0004")
        );
    }
}
