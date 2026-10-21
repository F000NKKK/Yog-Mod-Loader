//! What a request got wrong, typed: what to say, and each problem with
//! where it is — a parameter's path, a file's line and column — so a tool
//! shows it without reading it out of a string.

use yog_data::Schema;

/// The error a request is answered with.
#[derive(Debug, Clone, Default, PartialEq, Schema)]
pub struct ServiceError {
    pub message: String,
    /// Every problem, each with its code; at least one.
    #[data(default)]
    pub problems: Vec<Problem>,
    /// The file the request is about, when it's about one.
    pub file: Option<String>,
}

/// One problem: what it is, what's wrong, and where.
#[derive(Debug, Clone, Default, PartialEq, Schema)]
pub struct Problem {
    /// What it is (`yog_ids::problem_code`).
    pub code: String,
    pub message: String,
    /// The value it's about: a parameter, a field of one (`args.nodes[2].icon`).
    #[data(default)]
    pub path: String,
    /// The line and column of the text it's in, when it's about text.
    pub line: Option<u32>,
    pub col: Option<u32>,
}

impl ServiceError {
    /// One problem of kind `code`.
    pub fn of(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        ServiceError {
            problems: vec![Problem {
                code: code.to_string(),
                message: message.clone(),
                ..Problem::default()
            }],
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
            yog_ids::problem_code::SYNTAX,
            e.message.clone(),
            e.at.line,
            Some(e.at.col),
        )
    }

    /// What a request names isn't there.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::of(yog_ids::problem_code::NOT_FOUND, message)
    }

    /// Parameter `name` that doesn't read: each of its problems, at its path.
    pub fn param(name: &str, problems: Vec<yog_data::schema::Problem>) -> Self {
        let problems: Vec<Problem> = problems
            .into_iter()
            .map(|p| Problem {
                code: p.code.to_string(),
                path: if p.path.is_root() {
                    name.to_string()
                } else {
                    format!("{name}.{}", p.path)
                },
                message: p.message,
                ..Problem::default()
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
        Self::of(yog_ids::problem_code::FAILED, message)
    }
}

impl From<&str> for ServiceError {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}
