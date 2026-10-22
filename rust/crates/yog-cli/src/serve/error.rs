//! What a request got wrong, typed: what to say, and each problem with
//! where it is — a parameter's path, a file's line and column — so a tool
//! shows it without reading it out of a string.

use yog_data::{Diagnostic, Path, Schema};

/// The error a request is answered with.
#[derive(Debug, Clone, Default, PartialEq, Schema)]
pub struct ServiceError {
    pub message: String,
    /// Every problem, each with its code; at least one.
    #[data(default)]
    pub problems: Vec<Diagnostic>,
    /// The file the request is about, when it's about one.
    pub file: Option<String>,
}

impl ServiceError {
    /// One problem of kind `code`.
    pub fn of(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        ServiceError {
            problems: vec![Diagnostic::error(code, &message)],
            message,
            file: None,
        }
    }

    /// `message` of kind `code`, about the text at `line`:`col`.
    pub fn at(code: &str, message: impl Into<String>, line: u32, col: Option<u32>) -> Self {
        let mut e = Self::of(code, message);
        e.problems[0].line = Some(line);
        e.problems[0].col = col;
        e
    }

    /// Text that doesn't parse.
    pub fn syntax(e: &yog_data::SyntaxError) -> Self {
        Self::at(
            yog_ids::data_problem::SYNTAX,
            e.message.clone(),
            e.at.line,
            Some(e.at.col),
        )
    }

    /// What a request names isn't there.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::of(yog_ids::request_problem::NOT_FOUND, message)
    }

    /// Parameter `name` that doesn't read: each of its problems, at its path.
    pub fn param(name: &str, problems: Vec<Diagnostic>) -> Self {
        let param = Path::root().key(name);
        let problems: Vec<Diagnostic> = problems
            .into_iter()
            .map(|p| {
                let path = param.then(&p.path);
                p.at_path(path)
            })
            .collect();
        let message = match problems.as_slice() {
            [one] => format!("`{}`: {}", one.path, one.message),
            _ => format!("`{name}` has {} problems", problems.len()),
        };
        ServiceError {
            message,
            problems,
            file: None,
        }
    }
}

impl From<String> for ServiceError {
    fn from(message: String) -> Self {
        Self::of(yog_ids::request_problem::FAILED, message)
    }
}

impl From<&str> for ServiceError {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}

/// Yog Data that didn't read: each problem with its code, field and place.
impl From<yog_data::Errors> for ServiceError {
    fn from(found: yog_data::Errors) -> Self {
        ServiceError {
            message: found.to_string(),
            problems: found.0,
            file: None,
        }
    }
}
