//! A type's operators as Java writes them: static methods named as the
//! CLR names them (`op_Addition`, `op_Implicit`), so `a + b` on a `Vec` is
//! `Vec.op_Addition(a, b)`, `(i32)e` `Energy.op_Explicit(e)`, and a value
//! handed on through an implicit conversion `Energy.op_Implicit(5)`; a
//! step (`++c`) and an `op=` give the place the operator's value.

use yog_script_analyze::ast::{Expr, ExprKind, UnOp};
use yog_script_analyze::{OperatorUse, Type};

use super::Gen;

impl Gen<'_> {
    /// `e` as the operator its type declares it to be.
    pub(crate) fn operator_call(&mut self, e: &Expr, used: &OperatorUse) -> String {
        if let ExprKind::Unary { op, .. } = &e.kind
            && op.steps()
        {
            return self.declared_step(e, used, false);
        }
        let operands: Vec<&Expr> = match &e.kind {
            ExprKind::Binary { left, right, .. } => vec![left, right],
            ExprKind::Unary { expr, .. } | ExprKind::Cast { expr, .. } => vec![expr],
            _ => Vec::new(),
        };
        let args: Vec<String> = operands.into_iter().map(|o| self.handed_expr(o)).collect();
        self.applied(used, &args)
    }

    /// `++x` / `x--` on a type declaring `operator ++`: its place given the
    /// operator's value; `alone`: a statement (a postfix's old value unused).
    pub(crate) fn declared_step(&mut self, e: &Expr, used: &OperatorUse, alone: bool) -> String {
        let ExprKind::Unary { op, expr } = &e.kind else {
            return String::new();
        };
        if !alone && matches!(op, UnOp::PostIncrement | UnOp::PostDecrement) {
            self.not_yet(
                "the old value of a type's own `x++` (`++x` gives the new one)",
                e.span,
            );
            return String::new();
        }
        let place = self.expr(expr);
        let set = format!("{place} = {}", self.applied(used, std::slice::from_ref(&place)));
        if alone { set } else { format!("({set})") }
    }

    /// Value `v` through the implicit conversion `used`.
    pub(crate) fn converted(&self, used: &OperatorUse, v: &str) -> String {
        self.applied(used, &[v.to_string()])
    }

    /// Operator `used` called on `args`.
    pub(crate) fn applied(&self, used: &OperatorUse, args: &[String]) -> String {
        format!(
            "{}.{}({})",
            self.owner(&used.owner),
            used.method,
            args.join(", ")
        )
    }

    /// The class an operator is called on: its type's, without arguments
    /// (a static method's are inferred).
    fn owner(&self, t: &Type) -> String {
        match t.non_null() {
            Type::Own(name, args) => self.index.place(name, args.len()),
            other => self.java(other, false),
        }
    }
}
