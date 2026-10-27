//! `await t` in the machine: `t` kept in a field, the state after it
//! made, the machine leaving when `t` isn't done (it goes on there once it
//! is), then what `t` finished with. What its statement reads before it
//! (an operand, an argument) is kept in a field first, as C# reads it
//! before it waits; one only some paths reach is `branches.rs`'. An
//! `await` in a `switch`'s arm, after `?.`, or beside a value computed
//! before its statement, isn't compiled yet.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, ExprKind, Stmt, UnOp};
use yog_script_ids::java::{IF, RETURN};
use yog_script_ids::jvm::tasks::{CHECK, RESULT, STATE, SUSPEND};

use crate::Gen;

impl Gen<'_> {
    /// `await inner`: its value (a `Task`'s nothing: its failure thrown).
    pub(crate) fn await_value(&mut self, e: &Expr, inner: &Expr) -> String {
        if self.machine.is_none() {
            self.not_yet("`await` outside an `async` method", e.span);
            return String::new();
        }
        let t = self.types.of(inner).cloned().unwrap_or(Type::Unknown);
        let task = self.expr(inner);
        let field = self.temp();
        self.declare_field(&self.java(&t, false), &field);
        let state = self.new_state();
        self.code.line(format!("{field} = {task};"));
        self.code.line(format!("{STATE} = {state};"));
        self.code
            .line(format!("{IF} ({SUSPEND}({field})) {RETURN};"));
        self.state_starts(state);
        match &t {
            Type::Own(_, args) if args.len() == 1 => format!("{field}.{RESULT}()"),
            _ => {
                self.code.line(format!("{field}.{CHECK}();"));
                String::new()
            }
        }
    }

    /// Value `v` (of `e`) where an `await` comes after it in its statement:
    /// kept in a field first, as C# reads it before it waits; else as it is.
    pub(crate) fn kept_before(&mut self, v: String, e: &Expr, later_awaits: bool) -> String {
        let constant = matches!(
            e.kind,
            ExprKind::Int(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::Str(_)
                | ExprKind::Null
        );
        if self.machine.is_none() || !later_awaits || constant || v.is_empty() {
            return v;
        }
        let t = self.types.of(e).cloned().unwrap_or(Type::Unknown);
        let field = self.temp();
        self.declare_field(&self.java(&t, false), &field);
        self.code.line(format!("{field} = {v};"));
        field
    }

    /// Whether statement `s`'s values hold an `await` this can't write yet:
    /// reported when they do.
    pub(crate) fn await_unwritable(&mut self, e: &Expr) -> bool {
        let hazard = hazard(e);
        if let Some(why) = hazard {
            self.not_yet(why, e.span);
        }
        hazard.is_some()
    }
}

/// Whether `e` awaits (a lambda's code aside: it runs later, its own).
pub(crate) fn awaits(e: &Expr) -> bool {
    if matches!(e.kind, ExprKind::Lambda { .. }) {
        return false;
    }
    let mut found = matches!(
        e.kind,
        ExprKind::Unary {
            op: UnOp::Await,
            ..
        }
    );
    e.each_child(|c| found |= awaits(c));
    found
}

/// Whether statement `s` leaves its machine anywhere in it: awaits, or
/// (an iterator's) yields.
pub(crate) fn stmt_awaits(s: &Stmt) -> bool {
    let all = |stmts: &[Stmt]| stmts.iter().any(stmt_awaits);
    match s {
        Stmt::Local { value, .. } => value.as_ref().is_some_and(awaits),
        Stmt::Goto(yog_script_analyze::ast::Goto::Case(v), _) => awaits(v),
        Stmt::Goto(..) | Stmt::Label(_) => false,
        Stmt::Assign { place, value, .. } => awaits(place) || awaits(value),
        Stmt::If {
            cond,
            then,
            otherwise,
        } => awaits(cond) || stmt_awaits(then) || otherwise.as_deref().is_some_and(stmt_awaits),
        Stmt::Foreach {
            iter,
            body,
            awaits: each,
            ..
        } => *each || awaits(iter) || stmt_awaits(body),
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => {
            init.as_deref().is_some_and(stmt_awaits)
                || cond.as_ref().is_some_and(awaits)
                || step.as_deref().is_some_and(stmt_awaits)
                || stmt_awaits(body)
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            awaits(cond) || stmt_awaits(body)
        }
        Stmt::Block(b) | Stmt::Unsafe(b, _) => all(&b.stmts),
        Stmt::Return(v, _) | Stmt::Skip(v, _) => v.as_ref().is_some_and(awaits),
        // An iterator's `yield` leaves its machine as an `await` does.
        Stmt::Yield(..) => true,
        // Nothing waits inside a `lock` (the checker says so).
        Stmt::Lock { .. } => false,
        Stmt::Expr(e) | Stmt::Value(e) => awaits(e),
        Stmt::Break(_) | Stmt::Continue(_) => false,
        // A local function's awaits are its own code's.
        Stmt::Function(_) => false,
        Stmt::Switch { expr, sections } => {
            awaits(expr)
                || sections.iter().any(|s| {
                    s.labels
                        .iter()
                        .flatten()
                        .any(|(_, g)| g.as_ref().is_some_and(awaits))
                        || all(&s.body)
                })
        }
    }
}

/// Why `e`'s `await` can't be written yet, when it can't.
fn hazard(e: &Expr) -> Option<&'static str> {
    if !awaits(e) {
        return None;
    }
    let mut found = None;
    e.each_child(|c| found = found.or_else(|| hazard(c)));
    found
}
