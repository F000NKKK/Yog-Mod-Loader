//! `switch` and `is`: a switch's value computed before its statement, in a
//! labeled block — each arm an `if` on its pattern, its names bound, its
//! guard, its value; none taking the value panics.

use yog_script_analyze::ast::{Arm, Expr, ExprKind, Pattern, TypeKind};

use super::Gen;
use crate::index::Kind;
use crate::types::{RUNTIME, java};

impl Gen<'_> {
    pub(crate) fn switch(&mut self, e: &Expr, on: &Expr, arms: &[Arm]) -> String {
        let t = self
            .types
            .of(e)
            .map_or_else(|| "Object".to_string(), |t| java(t, false));
        let (result, label, value) = (self.temp(), self.temp(), self.temp());
        let v = self.expr(on);
        self.code.line(format!("{t} {result};"));
        self.code.open(format!("{label}:"));
        self.code.line(format!("var {value} = {v};"));
        for arm in arms {
            let depth = self.pattern_open(&arm.pattern, &value);
            if let Some(g) = &arm.guard {
                let g = self.expr(g);
                self.code.open(format!("if ({g})"));
            }
            let out = self.handed_expr(&arm.value);
            self.code.line(format!("{result} = {out};"));
            self.code.line(format!("break {label};"));
            if arm.guard.is_some() {
                self.code.close("");
            }
            for _ in 0..depth {
                self.code.close("");
            }
        }
        self.code.line(format!(
            "throw {RUNTIME}.Ys.noArm({value}, {});",
            self.at(e.span)
        ));
        self.code.close("");
        result
    }

    /// `x is pattern` in a condition.
    pub(crate) fn is(&mut self, on: &Expr, p: &Pattern) -> String {
        let v = self.expr(on);
        match p {
            Pattern::Type { ty, bind } => {
                let t = crate::types::written(ty, true);
                match bind {
                    Some(n) => format!("({v} instanceof {t} {})", Self::ident(&n.text)),
                    None => format!("({v} instanceof {t})"),
                }
            }
            Pattern::Value(x) if matches!(x.kind, ExprKind::Null) => format!("({v} == null)"),
            Pattern::Not(inner) if matches!(&**inner, Pattern::Value(x) if matches!(x.kind, ExprKind::Null)) =>
            {
                format!("({v} != null)")
            }
            _ => {
                self.not_yet("this pattern in `is`", on.span);
                String::new()
            }
        }
    }

    /// The `if`s that pattern `p` on `v` opens (its names bound inside):
    /// how many to close.
    fn pattern_open(&mut self, p: &Pattern, v: &str) -> usize {
        match p {
            Pattern::Wild(_) => 0,
            Pattern::Let(n) => {
                self.code
                    .line(format!("var {} = {v};", Self::ident(&n.text)));
                0
            }
            Pattern::Value(x) => {
                let x = self.expr(x);
                self.code
                    .open(format!("if (java.util.Objects.equals({v}, {x}))"));
                1
            }
            Pattern::Type { ty, bind } => {
                self.case_or_type(ty, bind.as_ref().map(|n| n.text.as_str()), v)
            }
            Pattern::Variant { path, args } => {
                let (of, case) = self.case_path(path);
                let c = self.temp();
                self.code.open(format!(
                    "if ({v} instanceof {} {c})",
                    qualified(of.as_deref(), &case)
                ));
                let fields = self.index.case_fields(of.as_deref(), &case);
                let mut depth = 1;
                for (arg, field) in args.iter().zip(fields) {
                    let inner = format!("{c}.{}()", Self::ident(&field));
                    depth += self.pattern_open(arg, &inner);
                }
                depth
            }
            Pattern::Relational { op, value } => {
                let x = self.expr(value);
                self.code
                    .open(format!("if ({v} {} {x})", crate::exprs::op_text(*op)));
                1
            }
            Pattern::Range { from, to } => {
                let (a, b) = (self.expr(from), self.expr(to));
                self.code.open(format!("if ({a} <= {v} && {v} <= {b})"));
                1
            }
            _ => {
                self.not_yet("this pattern", yog_script_analyze::token::Span::default());
                0
            }
        }
    }

    /// `Mode.Idle` (a case) or `Monster m` (a type) as a pattern.
    fn case_or_type(
        &mut self,
        ty: &yog_script_analyze::ast::Type,
        bind: Option<&str>,
        v: &str,
    ) -> usize {
        let TypeKind::Path { path, .. } = &ty.kind else {
            return 0;
        };
        let of = path.len().checked_sub(2).map(|k| path[k].text.clone());
        let written = path
            .iter()
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let cond = match of.as_deref().and_then(|o| self.index.kind(o)) {
            Some(Kind::Enum) => format!("{v} == {written}"),
            _ => match bind {
                Some(b) => format!("{v} instanceof {written} {}", Self::ident(b)),
                None => format!("{v} instanceof {written}"),
            },
        };
        self.code.open(format!("if ({cond})"));
        1
    }

    /// A case's enum (its Java name: the runtime's for `Result`) and name.
    fn case_path(&self, path: &[yog_script_analyze::ast::Name]) -> (Option<String>, String) {
        let case = path.last().map(|n| n.text.clone()).unwrap_or_default();
        let of = match path.len().checked_sub(2) {
            Some(k) => Some(path[k].text.clone()),
            None => self.index.enum_of(&case),
        };
        (of, case)
    }
}

/// Whether `e` computes a value before its statement (a `switch`, a
/// `try`): it can't stand where Java evaluates it apart.
pub(crate) fn hoists(e: &Expr) -> bool {
    let mut found = matches!(
        e.kind,
        ExprKind::Switch { .. }
            | ExprKind::Unary {
                op: yog_script_analyze::ast::UnOp::Try,
                ..
            }
    );
    e.each_child(|c| found |= hoists(c));
    found
}

/// A case's Java type: `Mode.Charging`, the runtime's `Result.Ok`.
fn qualified(of: Option<&str>, case: &str) -> String {
    match of {
        Some(o) => format!("{}.{case}", crate::types::own(o)),
        None => case.to_string(),
    }
}
