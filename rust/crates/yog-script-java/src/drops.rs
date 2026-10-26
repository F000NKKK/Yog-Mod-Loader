//! Dropping, as Rust's (`ys.md`, "Dropping"): a local holding an
//! `IDisposable` is disposed when its scope ends, however it ends (C#'s
//! `using`, implicit) — the rest of its block in `try`, `Dispose()` in
//! `finally`. One moved on (`move x`) or returned is the new owner's to
//! drop, and isn't disposed here.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Binding, Expr, ExprKind, Stmt};
use yog_script_ids::java::{FINALLY, IF, NULL, TRY};
use yog_script_ids::system::DISPOSABLE;

use crate::Gen;

impl Gen<'_> {
    /// The local `s` declares, when it's dropped at its scope's end: one
    /// holding an `IDisposable` that `rest` neither moves nor returns.
    pub(crate) fn dropped_local<'s>(&self, s: &'s Stmt, rest: &[Stmt]) -> Option<&'s str> {
        let Stmt::Local {
            binding: Binding::One(n),
            value: Some(v),
            ..
        } = s
        else {
            return None;
        };
        let t = self.types.of(v)?;
        if !self.is_disposable(t) || rest.iter().any(|r| hands_on(r, &n.text)) {
            return None;
        }
        Some(&n.text)
    }

    /// `rest` (the statements after local `name`) in `try`, `name`
    /// disposed in `finally` (when it's there: it may be `null`).
    pub(crate) fn dropping(&mut self, name: &str, rest: impl FnOnce(&mut Self)) {
        let local = Self::ident(name);
        self.code.open(TRY);
        rest(self);
        self.code.close("");
        self.code.open(FINALLY);
        self.code
            .line(format!("{IF} ({local} != {NULL}) {local}.{DISPOSE}();"));
        self.code.close("");
    }

    /// Whether a value of `t` is an `IDisposable`: its type, or one it
    /// inherits, implements it.
    fn is_disposable(&self, t: &Type) -> bool {
        match t.non_null() {
            Type::Own(name, args) => self
                .index
                .implements(&(name.clone(), args.len()), DISPOSABLE),
            _ => false,
        }
    }
}

/// What disposes: `IDisposable`'s method.
const DISPOSE: &str = yog_script_ids::members::DISPOSE;

/// Whether `s` hands local `name` on: `move name` anywhere in it, or
/// `return name`.
fn hands_on(s: &Stmt, name: &str) -> bool {
    let mut found = false;
    each_expr(s, &mut |e| found |= moves(e, name));
    if let Stmt::Return(Some(e), _) = s
        && matches!(&e.kind, ExprKind::Ident(n) if n == name)
    {
        found = true;
    }
    found
}

/// Whether `e` moves local `name` somewhere in it.
fn moves(e: &Expr, name: &str) -> bool {
    if let ExprKind::Move(inner) = &e.kind
        && matches!(&inner.kind, ExprKind::Ident(n) if n == name)
    {
        return true;
    }
    let mut found = false;
    e.each_child(|c| found |= moves(c, name));
    found
}

/// Each expression statement `s` holds, its inner statements' too.
fn each_expr(s: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match s {
        Stmt::Local { value, .. } => value.iter().for_each(&mut *f),
        Stmt::Assign { place, value, .. } => {
            f(place);
            f(value);
        }
        Stmt::If {
            cond,
            then,
            otherwise,
        } => {
            f(cond);
            each_expr(then, f);
            if let Some(o) = otherwise {
                each_expr(o, f);
            }
        }
        Stmt::Foreach { iter, body, .. } => {
            f(iter);
            each_expr(body, f);
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => {
            if let Some(i) = init {
                each_expr(i, f);
            }
            cond.iter().for_each(&mut *f);
            if let Some(st) = step {
                each_expr(st, f);
            }
            each_expr(body, f);
        }
        Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
            f(cond);
            each_expr(body, f);
        }
        Stmt::Block(b) | Stmt::Unsafe(b, _) => b.stmts.iter().for_each(|s| each_expr(s, f)),
        Stmt::Return(v, _) | Stmt::Skip(v, _) | Stmt::Yield(v, _) => v.iter().for_each(&mut *f),
        Stmt::Lock { target, body, .. } => {
            f(target);
            each_expr(body, f);
        }
        Stmt::Expr(e) | Stmt::Value(e) => f(e),
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}
