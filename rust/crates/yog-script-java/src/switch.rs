//! `switch` and `is`: a switch's value computed before its statement, in a
//! labeled block — each arm an `if` on its pattern, its names bound, its
//! guard, its value; none taking the value panics.

use yog_script_analyze::ast::{Arm, Expr, ExprKind};

use super::Gen;
use crate::types::RUNTIME;
use yog_script_ids::java::{BREAK, IF, THROW, VAR};
use yog_script_ids::jvm::OBJECT;

impl Gen<'_> {
    pub(crate) fn switch(&mut self, e: &Expr, on: &Expr, arms: &[Arm]) -> String {
        let t = self
            .types
            .of(e)
            .map_or_else(|| OBJECT.to_string(), |t| self.java(t, false));
        let (result, label, value) = (self.temp(), self.temp(), self.temp());
        let v = self.expr(on);
        self.code.line(format!("{t} {result};"));
        self.code.open(format!("{label}:"));
        self.code.line(format!("{VAR} {value} = {v};"));
        let matched = self.types.of(on).cloned();
        for arm in arms {
            let depth = self.pattern_open(&arm.pattern, (&value, matched.as_ref()));
            if let Some(g) = &arm.guard {
                let g = self.expr(g);
                self.code.open(format!("{IF} ({g})"));
            }
            let out = self.handed_expr(&arm.value);
            self.code.line(format!("{result} = {out};"));
            self.code.line(format!("{BREAK} {label};"));
            if arm.guard.is_some() {
                self.code.close("");
            }
            for _ in 0..depth {
                self.code.close("");
            }
            // An arm taking anything (`_ =>`, `let x =>`): nothing after it is reached.
            if depth == 0 && arm.guard.is_none() {
                self.code.close("");
                return result;
            }
        }
        self.code.line(format!(
            "{THROW} {RUNTIME}.Ys.noArm({value}, {});",
            self.at(e.span)
        ));
        self.code.close("");
        result
    }
}

/// Whether `e` computes a value before its statement (a `switch`, a
/// `try`, a `with`): it can't stand where Java evaluates it apart.
pub(crate) fn hoists(e: &Expr) -> bool {
    let mut found = matches!(
        e.kind,
        ExprKind::Switch { .. }
            | ExprKind::With { .. }
            | ExprKind::Unary {
                op: yog_script_analyze::ast::UnOp::Try,
                ..
            }
    );
    e.each_child(|c| found |= hoists(c));
    found
}
