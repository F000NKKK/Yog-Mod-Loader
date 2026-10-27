//! `goto` (C#'s): a label is reached from its block and the blocks inside
//! it, never from outside (nor out of a lambda or a local function); its
//! name once in a method. `goto case`/`goto default` go in a `switch`.

use yog_ids::script_problem;

use super::Resolver;
use crate::ast::{Goto, Stmt};
use crate::source::token::Span;

impl Resolver<'_, '_> {
    /// `f` run with the labels of `stmts` (a block's) in reach.
    pub(super) fn labeled(&mut self, stmts: &[Stmt], f: impl FnOnce(&mut Self)) {
        let mut here = Vec::new();
        for s in stmts {
            let Stmt::Label(n) = s else {
                continue;
            };
            let twice = here.contains(&n.text) || self.labels.iter().flatten().any(|l| *l == n.text);
            if twice {
                self.report.error(
                    script_problem::GOTO,
                    n.span,
                    format!("a label `{}` is declared already around it", n.text),
                );
            }
            here.push(n.text.clone());
        }
        self.labels.push(here);
        f(self);
        self.labels.pop();
    }

    /// `f` (a method's or a lambda's body) with labels of its own only.
    pub(super) fn own_labels(&mut self, f: impl FnOnce(&mut Self)) {
        let outer = std::mem::take(&mut self.labels);
        f(self);
        self.labels = outer;
    }

    /// `goto to;` at `at`.
    pub(super) fn goto_stmt(&mut self, to: &Goto, at: Span) {
        match to {
            Goto::Label(n) => {
                if !self.labels.iter().flatten().any(|l| *l == n.text) {
                    self.report.error(
                        script_problem::GOTO,
                        n.span,
                        format!("no label `{}` in this block nor one around it", n.text),
                    );
                }
            }
            Goto::Case(_) | Goto::Default if self.cx.switches == 0 => {
                self.report.error(
                    script_problem::GOTO,
                    at,
                    "`goto case` and `goto default` go in a `switch`",
                );
            }
            Goto::Case(value) => self.expr(value),
            Goto::Default => {}
        }
    }
}
