//! Iterators, as C#'s: a method whose body `yield`s gives an
//! `IEnumerable<T>`, an `IEnumerator<T>` (or, `async`, an
//! `IAsyncEnumerable<T>`); each `yield return` a `T`, its end `yield
//! break` — never `return` with a value.

use yog_ids::script_problem;
use yog_script_ids::system::{ASYNC_ENUMERABLE, ENUMERABLE, ENUMERATOR};

use super::Checker;
use super::model::Type;
use crate::ast::{Expr, MethodBody, Stmt};
use crate::source::token::Span;

/// What an iterator giving `t` gives an item of: `IEnumerable<T>`'s `T`.
pub fn item_of(t: &Type) -> Option<&Type> {
    match t.non_null() {
        Type::Own(name, args)
            if args.len() == 1
                && [ENUMERABLE, ENUMERATOR, ASYNC_ENUMERABLE].contains(&name.as_str()) =>
        {
            args.first()
        }
        _ => None,
    }
}

/// Whether `body` yields: it's an iterator's.
pub fn body_yields(body: Option<&MethodBody>) -> bool {
    match body {
        Some(MethodBody::Block(b)) => b.stmts.iter().any(stmt_yields),
        _ => false,
    }
}

/// Whether `s` yields anywhere in it (a lambda's own code aside: lambdas
/// aren't iterators).
pub fn stmt_yields(s: &Stmt) -> bool {
    match s {
        Stmt::Yield(..) => true,
        Stmt::If {
            then, otherwise, ..
        } => stmt_yields(then) || otherwise.as_deref().is_some_and(stmt_yields),
        Stmt::Foreach { body, .. } | Stmt::While { body, .. } | Stmt::DoWhile { body, .. } => {
            stmt_yields(body)
        }
        Stmt::For {
            init, step, body, ..
        } => {
            init.as_deref().is_some_and(stmt_yields)
                || step.as_deref().is_some_and(stmt_yields)
                || stmt_yields(body)
        }
        Stmt::Block(b) => b.stmts.iter().any(stmt_yields),
        _ => false,
    }
}

impl Checker<'_, '_, '_> {
    /// `yield return v` (`v`) or `yield break` at `at`.
    pub(super) fn yielded(&mut self, v: Option<&Expr>, at: Span) {
        let item = self.yields.clone();
        match (item, v) {
            (Some(t), Some(v)) => self.fits(v, &t),
            (Some(_), None) => {}
            (None, v) => {
                if let Some(v) = v {
                    self.expr(v);
                }
                self.report.error(
                    script_problem::ITERATOR,
                    at,
                    "`yield` is an iterator's: a method giving an `IEnumerable<T>`, an `IEnumerator<T>` or an `IAsyncEnumerable<T>`",
                );
            }
        }
    }

    /// `return v` in an iterator: an error, its end is `yield break`.
    pub(super) fn returned_from_iterator(&mut self, at: Span) {
        self.report.error(
            script_problem::ITERATOR,
            at,
            "an iterator gives its items by `yield return` and ends by `yield break`, not `return` with a value",
        );
    }
}
