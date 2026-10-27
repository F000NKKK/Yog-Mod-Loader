//! `goto`: a stop (`Stop::Goto`) the statements of its label's block
//! catch, going on after the label; `goto case`/`goto default`, its
//! `switch` (`switch.rs`).

use yog_script_analyze::ast::{Goto, Stmt};

use super::{Interp, Run, Stop};
use crate::value::Value;

/// Where a `goto` goes.
pub(crate) enum Jump {
    Label(String),
    Case(Value),
    Default,
}

impl Interp<'_> {
    /// `stmts` run in order; a `goto` to one of their labels goes on after it.
    pub(super) fn run_stmts(&mut self, stmts: &[Stmt]) -> Run<()> {
        let mut at = 0;
        while let Some(s) = stmts.get(at) {
            match self.stmt(s) {
                Ok(()) => at += 1,
                Err(Stop::Goto(Jump::Label(l))) => match label_at(stmts, &l) {
                    Some(k) => at = k + 1,
                    None => return Err(Stop::Goto(Jump::Label(l))),
                },
                Err(other) => return Err(other),
            }
        }
        Ok(())
    }

    /// Where `goto to` goes (`goto case`'s value computed).
    pub(super) fn jump(&mut self, to: &Goto) -> Run<Jump> {
        Ok(match to {
            Goto::Label(n) => Jump::Label(n.text.clone()),
            Goto::Case(v) => Jump::Case(self.eval(v)?),
            Goto::Default => Jump::Default,
        })
    }
}

/// Where label `name` is among `stmts`.
fn label_at(stmts: &[Stmt], name: &str) -> Option<usize> {
    stmts
        .iter()
        .position(|s| matches!(s, Stmt::Label(n) if n.text == name))
}
