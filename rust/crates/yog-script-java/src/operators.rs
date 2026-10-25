//! A type's operators as Java writes them: static methods named as the
//! CLR names them (`op_Addition`, `op_Implicit`), so `a + b` on a `Vec` is
//! `Vec.op_Addition(a, b)`, `(i32)e` `Energy.op_Explicit(e)`, and a value
//! handed on through an implicit conversion `Energy.op_Implicit(5)`.

use yog_script_analyze::ast::{Expr, ExprKind};
use yog_script_analyze::{OperatorUse, Type};

use super::Gen;

impl Gen<'_> {
    /// `e` as the operator its type declares it to be.
    pub(crate) fn operator_call(&mut self, e: &Expr, used: &OperatorUse) -> String {
        let operands: Vec<&Expr> = match &e.kind {
            ExprKind::Binary { left, right, .. } => vec![left, right],
            ExprKind::Unary { expr, .. } | ExprKind::Cast { expr, .. } => vec![expr],
            _ => Vec::new(),
        };
        let args: Vec<String> = operands.into_iter().map(|o| self.handed_expr(o)).collect();
        format!(
            "{}.{}({})",
            self.owner(&used.owner),
            used.method,
            args.join(", ")
        )
    }

    /// Value `v` through the implicit conversion `used`.
    pub(crate) fn converted(&self, used: &OperatorUse, v: &str) -> String {
        format!("{}.{}({v})", self.owner(&used.owner), used.method)
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
