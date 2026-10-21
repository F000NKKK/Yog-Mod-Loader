//! A source as it's being edited, checked: what an editor shows live —
//! its syntax, then every problem its format finds, each at its line and
//! column.

use yog_data::Schema;
use yog_data::text::{Doc, LineCol};

use super::Problem;

/// What [`check`] found.
#[derive(Debug, Default, Schema)]
pub struct Checked {
    pub problems: Vec<Problem>,
}

/// The text of the file at `path` (unsaved, as edited), checked: a syntax
/// error, or what its format finds (none for a format it doesn't know).
pub fn check(path: &str, text: &str) -> Checked {
    let doc = match yog_data::text::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            return Checked {
                problems: vec![Problem {
                    code: yog_ids::problem_code::SYNTAX.to_string(),
                    message: e.message,
                    path: String::new(),
                    line: Some(e.at.line),
                    col: Some(e.at.col),
                }],
            };
        }
    };
    let Some(format) = yog_ids::Format::of_path(std::path::Path::new(path)) else {
        return Checked::default();
    };
    let problems = yog_formats::problems(format, &doc.value)
        .into_iter()
        .map(|p| located(&doc, text, p))
        .collect();
    Checked { problems }
}

/// `p` at its place in `text`: its key's for one about the key (an unknown field).
fn located(doc: &Doc, text: &str, p: yog_data::schema::Problem) -> Problem {
    let spans = doc.spans_at(&p.path);
    let start = match (p.at_key, spans.key) {
        (true, Some(key)) => key.start,
        _ => spans.span.start,
    };
    let at = LineCol::of(text, start as usize);
    Problem {
        code: p.code.to_string(),
        message: p.message,
        path: p.path.to_string(),
        line: Some(at.line),
        col: Some(at.col),
    }
}
