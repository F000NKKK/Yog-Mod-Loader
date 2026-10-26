//! `decimal` on the JVM: Java's `BigDecimal`, whose arithmetic is methods —
//! `a + b` is `a.add(b)`, a comparison `compareTo`, a number of another
//! type made one first (`Decimal.op_Implicit(n)`, its implicit conversion).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Expr};
use yog_script_ids::clr::{DECIMAL, OP_IMPLICIT};

use super::Gen;

/// The precision a `decimal` division keeps (.NET's 28-29 digits).
const DIVISION: &str = "java.math.MathContext.DECIMAL128";

impl Gen<'_> {
    /// Whether `left op right` is a `decimal`'s (`e` its whole).
    pub(crate) fn is_decimal(&self, e: &Expr, left: &Expr, right: &Expr) -> bool {
        [e, left, right]
            .iter()
            .any(|x| self.types.of(x).map(Type::non_null) == Some(&Type::Decimal))
    }

    /// `l op r`, a `decimal`'s, its operands (`left`, `right`) written.
    pub(crate) fn decimal_binary(
        &self,
        op: BinOp,
        (left, l): (&Expr, String),
        (right, r): (&Expr, String),
    ) -> Option<String> {
        let (l, r) = (self.as_decimal(left, l), self.as_decimal(right, r));
        let method = match op {
            BinOp::Add => "add",
            BinOp::Sub => "subtract",
            BinOp::Mul => "multiply",
            BinOp::Rem => "remainder",
            BinOp::Div => return Some(format!("{l}.divide({r}, {DIVISION})")),
            BinOp::Eq => return Some(format!("({l}.compareTo({r}) == 0)")),
            BinOp::NotEq => return Some(format!("({l}.compareTo({r}) != 0)")),
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                return Some(format!(
                    "({l}.compareTo({r}) {} 0)",
                    crate::exprs::op_text(op)
                ));
            }
            _ => return None,
        };
        Some(format!("{l}.{method}({r})"))
    }

    /// `-d` of a `decimal`.
    pub(crate) fn decimal_negated(v: &str) -> String {
        format!("{v}.negate()")
    }

    /// Operand `v` (`e`'s) as a `decimal`: one already, or made one.
    fn as_decimal(&self, e: &Expr, v: String) -> String {
        match self.types.of(e).map(Type::non_null) {
            Some(Type::Decimal) | None => v,
            Some(_) => format!("{}.{OP_IMPLICIT}({v})", self.index.place(DECIMAL, 0)),
        }
    }
}
