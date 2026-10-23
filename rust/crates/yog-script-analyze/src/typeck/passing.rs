//! How each argument is passed against how its parameter takes it
//! (`ys.md` §3, Memory): written at both ends, so they must agree.

use yog_ids::script_problem;

use super::Checker;
use crate::ast::{Arg, Pass};
use crate::words::{self, Word};

impl Checker<'_, '_, '_> {
    /// Each of `args` against the parameter it lands on.
    pub(super) fn check_passing(&mut self, takes: &[Option<Pass>], args: &[Arg]) {
        for (a, &param) in args.iter().zip(takes) {
            if agrees(a.pass, param) {
                continue;
            }
            let wanted = param.map_or_else(
                || "by value (or `move`)".to_string(),
                |p| format!("with `{}`", word(p).text),
            );
            self.report.error(
                script_problem::PASS,
                a.value.span,
                format!("this parameter is passed {wanted}"),
            );
        }
    }
}

/// Whether an argument passed `given` fits a parameter taking `param`: an
/// `in` may be lent without the word, a value may be `move`d.
fn agrees(given: Option<Pass>, param: Option<Pass>) -> bool {
    matches!(
        (given, param),
        (None | Some(Pass::In), Some(Pass::In))
            | (Some(Pass::Ref), Some(Pass::Ref))
            | (Some(Pass::Out), Some(Pass::Out))
            | (Some(Pass::Move), Some(Pass::Move))
            | (None | Some(Pass::Move), None)
    )
}

/// The word that passes so.
fn word(p: Pass) -> &'static Word {
    match p {
        Pass::In => &words::r#in::WORD,
        Pass::Ref => &words::r#ref::WORD,
        Pass::Out => &words::out::WORD,
        Pass::Move => &words::r#move::WORD,
    }
}
