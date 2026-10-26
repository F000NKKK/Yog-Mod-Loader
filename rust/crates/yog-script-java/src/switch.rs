//! `switch` and `is`: a switch's value computed before its statement, in a
//! labeled block — each arm an `if` on its pattern, its names bound, its
//! guard, its value; none taking the value panics.

use yog_script_analyze::ast::{Arm, Expr, ExprKind, Pattern, Section, Stmt};

use super::Gen;
use crate::types::RUNTIME;
use yog_script_ids::java::{BOOLEAN, BREAK, FALSE, IF, THROW, TRUE, VAR};
use yog_script_ids::jvm::{HELPERS, NO_ARM, OBJECT};

impl Gen<'_> {
    pub(crate) fn switch(&mut self, e: &Expr, on: &Expr, arms: &[Arm]) -> String {
        if let Some(v) = self.split_switch(e, on, arms) {
            return v;
        }
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
            "{THROW} {RUNTIME}.{HELPERS}.{NO_ARM}({value}, {});",
            self.at(e.span)
        ));
        self.code.close("");
        result
    }
}

impl Gen<'_> {
    /// A `switch` statement: in a labeled block, its value read once; each
    /// section an `if` on its labels' patterns (its names bound when it has
    /// one label), its statements, `break` leaving the block; `default`'s
    /// last, when no label took it.
    pub(crate) fn switch_stmt(&mut self, on: &Expr, sections: &[Section]) {
        let (label, value) = (self.temp(), self.temp());
        let v = self.expr(on);
        let matched = self.types.of(on).cloned();
        self.code.open(format!("{label}:"));
        self.code.line(format!("{VAR} {value} = {v};"));
        self.breaks.push(Some(format!("{BREAK} {label};")));
        let leave = format!("{BREAK} {label};");
        let is_default = |s: &Section| s.labels.iter().any(Option::is_none);
        for s in sections.iter().filter(|s| !is_default(s)) {
            match s.labels.as_slice() {
                [Some((pattern, guard))] => {
                    self.labelled((pattern, guard.as_ref()), (&value, matched.as_ref()), |g| {
                        g.section(&s.body, &leave);
                    });
                }
                labels => {
                    let hit = self.temp();
                    self.code.line(format!("{BOOLEAN} {hit} = {FALSE};"));
                    for (pattern, guard) in labels.iter().flatten() {
                        self.code.open(format!("{IF} (!{hit})"));
                        self.labelled((pattern, guard.as_ref()), (&value, matched.as_ref()), |g| {
                            g.code.line(format!("{hit} = {TRUE};"));
                        });
                        self.code.close("");
                    }
                    self.code.open(format!("{IF} ({hit})"));
                    self.section(&s.body, &leave);
                    self.code.close("");
                }
            }
        }
        if let Some(d) = sections.iter().find(|s| is_default(s)) {
            self.section(&d.body, &leave);
        }
        self.breaks.pop();
        self.code.close("");
    }

    /// `then` where `pattern` (and `guard`) takes `value` (of `matched`).
    fn labelled(
        &mut self,
        (pattern, guard): (&Pattern, Option<&Expr>),
        (value, matched): (&str, Option<&yog_script_analyze::Type>),
        then: impl FnOnce(&mut Self),
    ) {
        let depth = self.pattern_open(pattern, (value, matched));
        if let Some(g) = guard {
            let g = self.expr(g);
            self.code.open(format!("{IF} ({g})"));
        }
        then(self);
        if guard.is_some() {
            self.code.close("");
        }
        for _ in 0..depth {
            self.code.close("");
        }
    }

    /// A section's statements, leaving the switch after them (unless they
    /// leave already).
    fn section(&mut self, body: &[Stmt], leave: &str) {
        body.iter().for_each(|s| self.stmt(s));
        if !body.last().is_some_and(yog_script_analyze::leaves) {
            self.code.line(leave);
        }
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
