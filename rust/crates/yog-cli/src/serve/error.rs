//! What a request got wrong, typed: what to say, and each problem with
//! where it is — a parameter's path, a file's line and column — so a tool
//! shows it without reading it out of a string.

use yog_data::Schema;

/// The error a request is answered with.
#[derive(Debug, Clone, Default, PartialEq, Schema)]
pub struct ServiceError {
    pub message: String,
    #[data(default)]
    pub problems: Vec<Problem>,
}

/// One problem: what's wrong, and where.
#[derive(Debug, Clone, Default, PartialEq, Schema)]
pub struct Problem {
    pub message: String,
    /// The value it's about: a parameter, a field of one (`args.nodes[2].icon`).
    #[data(default)]
    pub path: String,
    /// The line and column of the text it's in, when it's about text.
    pub line: Option<u32>,
    pub col: Option<u32>,
}

impl ServiceError {
    /// `message`, about the text at `line`:`col`.
    pub fn at(message: impl Into<String>, line: u32, col: Option<u32>) -> Self {
        let message = message.into();
        ServiceError {
            problems: vec![Problem {
                message: message.clone(),
                line: Some(line),
                col,
                ..Problem::default()
            }],
            message,
        }
    }

    /// Text that doesn't parse.
    pub fn syntax(e: &yog_data::SyntaxError) -> Self {
        Self::at(e.message.clone(), e.at.line, Some(e.at.col))
    }

    /// Parameter `name` that doesn't read: each of its problems, at its path.
    pub fn param(name: &str, problems: Vec<yog_data::schema::Problem>) -> Self {
        let problems: Vec<Problem> = problems
            .into_iter()
            .map(|p| Problem {
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
        ServiceError { message, problems }
    }
}

impl From<String> for ServiceError {
    fn from(message: String) -> Self {
        ServiceError {
            message,
            problems: Vec::new(),
        }
    }
}

impl From<&str> for ServiceError {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}
