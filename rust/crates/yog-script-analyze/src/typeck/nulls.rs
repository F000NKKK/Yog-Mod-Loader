//! Null state (C#'s nullable analysis, for names): a local, a parameter or
//! a member by its name alone, of a nullable type, is known not null where a condition proves it — the
//! branch of `x != null`, `x is not null`, `x is T`, the other of
//! `x == null`, past `&&`/`||`, in `?:` — and after an `if` whose branch
//! proving the opposite always leaves (`if (x == null) return;`), until
//! something nullable is assigned to it.

use super::Checker;
use super::model::Type;
use crate::ast::{BinOp, Expr, ExprKind, Pattern, Stmt, UnOp};
use crate::macros::builtin::{Gives, named};

impl Checker<'_, '_, '_> {
    /// `names` known not null from here to the current scope's end.
    pub(super) fn narrow(&mut self, names: Vec<String>) {
        let depth = self.scopes.len();
        self.non_null.extend(names.into_iter().map(|n| (depth, n)));
    }

    /// Whether the local `name` bound in scope `slot` is known not null here.
    pub(super) fn is_narrowed(&self, name: &str, slot: usize) -> bool {
        self.non_null
            .iter()
            .any(|(depth, n)| n == name && slot < *depth)
    }

    /// Something of type `t` assigned to `name`: not null when `t` isn't
    /// nullable, else no longer known.
    pub(super) fn assigned_to(&mut self, name: &str, t: &Type) {
        self.non_null.retain(|(_, n)| n != name);
        if t.known() && !t.is_nullable() && *t != Type::Null {
            self.narrow(vec![name.to_string()]);
        }
    }

    /// The narrowings of scopes gone (the scopes are `depth` deep now).
    pub(super) fn scopes_left(&mut self, depth: usize) {
        self.non_null.retain(|(d, _)| *d <= depth);
    }

    /// `s`, checked with `names` known not null in it.
    pub(super) fn branch_with(&mut self, names: Vec<String>, s: &Stmt) {
        self.scoped(|c| {
            c.narrow(names);
            c.stmt(s);
        });
    }

    /// `e`'s type, with `names` known not null in it.
    pub(super) fn expr_with(&mut self, names: Vec<String>, e: &Expr) -> Type {
        self.scoped(|c| {
            c.narrow(names);
            c.expr(e)
        })
    }
}

/// The names `cond` proves not null when it's `when`.
pub(super) fn proven(cond: &Expr, when: bool) -> Vec<String> {
    match &cond.kind {
        ExprKind::Binary { op, left, right } => match (op, when) {
            (BinOp::NotEq, true) | (BinOp::Eq, false) => compared_to_null(left, right),
            (BinOp::And, true) | (BinOp::Or, false) => {
                [proven(left, when), proven(right, when)].concat()
            }
            _ => Vec::new(),
        },
        ExprKind::Unary {
            op: UnOp::Not,
            expr,
        } => proven(expr, !when),
        ExprKind::Is { expr, pattern } => match (&expr.kind, matched_not_null(pattern)) {
            (ExprKind::Ident(n), Some(m)) if m == when => vec![n.clone()],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// `x` of `x op null` or `null op x`.
fn compared_to_null(left: &Expr, right: &Expr) -> Vec<String> {
    match (&left.kind, &right.kind) {
        (ExprKind::Ident(n), ExprKind::Null) | (ExprKind::Null, ExprKind::Ident(n)) => {
            vec![n.clone()]
        }
        _ => Vec::new(),
    }
}

/// When matching `p` proves a value not null: `Some(true)` when it
/// matches (`T`, `not null`), `Some(false)` when it doesn't (`null`).
fn matched_not_null(p: &Pattern) -> Option<bool> {
    match p {
        Pattern::Type { .. } | Pattern::Property { .. } => Some(true),
        Pattern::Value(Expr {
            kind: ExprKind::Null,
            ..
        }) => Some(false),
        Pattern::Not(inner) => matched_not_null(inner).map(|m| !m),
        _ => None,
    }
}

/// Whether `s` always leaves what it's in (`return`, `break`, `continue`,
/// `yield break`, a panic), so what follows runs only when it doesn't run.
pub(super) fn leaves(s: &Stmt) -> bool {
    match s {
        Stmt::Return(..) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Skip(..) => true,
        Stmt::Yield(None, _) => true,
        Stmt::Block(b) | Stmt::Unsafe(b, _) => b.stmts.last().is_some_and(leaves),
        Stmt::If {
            then,
            otherwise: Some(o),
            ..
        } => leaves(then) && leaves(o),
        Stmt::Expr(Expr {
            kind: ExprKind::Macro { name, .. },
            ..
        }) => named(&name.text).is_some_and(|b| matches!(b.gives, Gives::Never)),
        _ => false,
    }
}
