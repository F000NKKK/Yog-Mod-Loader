//! A source as it's being edited, checked: what an editor shows live —
//! its syntax, then every problem its format finds, each at its line and
//! column.

use yog_data::{Diagnostic, Schema};

/// What [`check`] found.
#[derive(Debug, Default, Schema)]
pub struct Checked {
    pub problems: Vec<Diagnostic>,
}

/// The text of the file at `path` (unsaved, as edited), checked: a syntax
/// error, or what its format finds (none for a format it doesn't know).
pub fn check(path: &str, text: &str) -> Checked {
    let doc = match yog_data::text::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            return Checked {
                problems: vec![Diagnostic::syntax(e)],
            };
        }
    };
    let Some(format) = yog_ids::Format::of_path(std::path::Path::new(path)) else {
        return Checked::default();
    };
    let problems = yog_formats::problems(format, &doc.value)
        .into_iter()
        .map(|p| yog_data::placed(text, &doc, p))
        .collect();
    Checked { problems }
}
