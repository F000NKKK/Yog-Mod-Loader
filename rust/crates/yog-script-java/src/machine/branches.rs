//! A value only some paths reach an `await` in, in the machine: `a && await
//! b`, `a || await b`, `a ?? await b`, `c ? await a : b`. Its value is a
//! field, each path writing it; the path not taken jumped past, as C#'s
//! machine does — what's past it is a state of its own.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Expr};
use yog_script_ids::java::{IF, NULL};
use yog_script_ids::jvm::OBJECT;

use super::awaits;
use crate::Gen;

impl Gen<'_> {
    /// `left op right` (`e`) whose `right` awaits: `None` when it doesn't,
    /// or the code isn't a machine's.
    pub(crate) fn short_circuit(
        &mut self,
        e: &Expr,
        op: BinOp,
        (left, right): (&Expr, &Expr),
    ) -> Option<String> {
        if self.machine.is_none() || !awaits(right) {
            return None;
        }
        let taken_when = match op {
            BinOp::And => "!",
            BinOp::Or => "",
            BinOp::Coalesce => "",
            _ => return None,
        };
        let value = self.branch_field(e);
        let past = self.new_state();
        let l = self.expr(left);
        self.code.line(format!("{value} = {l};"));
        let skip = match op {
            BinOp::Coalesce => format!("{value} != {NULL}"),
            _ => format!("{taken_when}{value}"),
        };
        self.code
            .line(format!("{IF} ({skip}) {}", Self::jump(past)));
        let r = self.expr(right);
        self.code.line(format!("{value} = {r};"));
        self.state_starts(past);
        Some(value)
    }

    /// `cond ? then : otherwise` (`e`) whose branches await: `None` when
    /// they don't, or the code isn't a machine's.
    pub(crate) fn split_conditional(
        &mut self,
        e: &Expr,
        (cond, then, otherwise): (&Expr, &Expr, &Expr),
    ) -> Option<String> {
        if self.machine.is_none() || !(awaits(then) || awaits(otherwise)) {
            return None;
        }
        let value = self.branch_field(e);
        let (other, past) = (self.new_state(), self.new_state());
        let c = self.expr(cond);
        self.code
            .line(format!("{IF} (!({c})) {}", Self::jump(other)));
        let a = self.handed_expr(then);
        self.code.line(format!("{value} = {a};"));
        self.code.line(Self::jump(past));
        self.state_starts(other);
        let b = self.handed_expr(otherwise);
        self.code.line(format!("{value} = {b};"));
        self.state_starts(past);
        Some(value)
    }

    /// The field value `e` is kept in, of its type.
    pub(crate) fn branch_field(&mut self, e: &Expr) -> String {
        let t = self.types.of(e).cloned().unwrap_or(Type::Unknown);
        // Boxed: `a ?? b`'s `a` may be `null`.
        let ty = if t.known() {
            self.java(&t, true)
        } else {
            OBJECT.to_string()
        };
        let field = self.temp();
        self.declare_field(&ty, &field);
        field
    }
}
