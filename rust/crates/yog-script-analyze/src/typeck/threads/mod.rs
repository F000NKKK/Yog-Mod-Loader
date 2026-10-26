//! What crosses threads (`ys-concurrency.md` §2): where a method's
//! `[Crosses("work", Crossing.…)]` parameter takes a lambda, what it
//! captures — a local handed over sendable (shareable where it runs on
//! several threads at once), `this` and statics shareable; another value
//! itself. `unsafe { … }` skips it (a warning).

mod captures;
mod safety;

pub use captures::{Capture, Captured};

use yog_ids::script_problem;
use yog_script_ids::system::threads::CONCURRENT;

use self::safety::{Blame, Need};
use super::Checker;
use super::model::Type;
use super::own::TypeKey;
use super::types::MethodUse;
use crate::ast::{Arg, Expr, ExprKind};
use crate::source::token::Span;

/// What an error calls a value handed over that isn't a name.
const VALUE: &str = "the value";

impl Checker<'_, '_, '_> {
    /// A call of `used` with `args`: what its crossing parameters take checked.
    pub(super) fn crossing_call(&mut self, used: &MethodUse, args: &[&Arg]) {
        let Some(owner) = &used.owner else {
            return;
        };
        let Some(own) = self.own.of_key(&TypeKey::new(owner, used.owner_arity)) else {
            return;
        };
        let crossings = own
            .crossings
            .get(&used.name)
            .and_then(|o| o.get(used.overload));
        let params = own
            .methods
            .get(&used.name)
            .and_then(|o| o.get(used.overload));
        let (Some(crossings), Some(params)) = (crossings, params) else {
            return;
        };
        let params: Vec<String> = params.params.iter().map(|p| p.name.text.clone()).collect();
        self.crossings(&crossings.clone(), &params, args);
    }

    /// `new name<arity>(args)` by its body's constructor `index`: as a call.
    pub(super) fn crossing_new(&mut self, key: &TypeKey, index: usize, args: &[&Arg]) {
        let Some(own) = self.own.of_key(key) else {
            return;
        };
        let (Some(crossings), Some(params)) = (own.ctor_crossings.get(index), own.ctors.get(index))
        else {
            return;
        };
        let params: Vec<String> = params.iter().map(|p| p.name.text.clone()).collect();
        self.crossings(&crossings.clone(), &params, args);
    }

    /// `unsafe { b }` (at `at`): checked without the threads checks, a warning.
    pub(super) fn unsafe_block(&mut self, b: &crate::ast::Block, at: Span) {
        self.report.warning(
            script_problem::UNSAFE,
            at,
            "`unsafe`: what crosses threads in it isn't checked — it's yours to keep safe",
        );
        self.unsafe_code += 1;
        self.block(b);
        self.unsafe_code -= 1;
    }

    /// Each of `crossings` (a parameter and how it crosses) of the
    /// arguments `args` given `params`.
    fn crossings(&mut self, crossings: &[(String, String)], params: &[String], args: &[&Arg]) {
        if self.unsafe_code > 0 {
            return;
        }
        for (param, crossing) in crossings {
            let Some(at) = params.iter().position(|p| p == param) else {
                continue;
            };
            let named = args
                .iter()
                .find(|a| a.name.as_ref().is_some_and(|n| n.text == *param));
            let arg = named.or_else(|| args.get(at).filter(|a| a.name.is_none()));
            if let Some(arg) = arg {
                let need = if crossing == CONCURRENT {
                    Need::Share
                } else {
                    Need::Send
                };
                self.crossing(&arg.value, need);
            }
        }
    }

    /// `value` going to another thread as `need` asks: a lambda's captures
    /// (its owner keeps reaching `this` and statics: shareable), else itself.
    fn crossing(&mut self, value: &Expr, need: Need) {
        if matches!(value.kind, ExprKind::Lambda { .. }) {
            for c in self.types.captures(value).to_vec() {
                let need = if c.kind == Captured::Local {
                    need
                } else {
                    Need::Share
                };
                if let Some(b) = self.blame(&c.ty, need) {
                    self.crossing_error((&c.name, &c.ty), need, &b, c.at);
                }
            }
            return;
        }
        let Some(t) = self.types.of(value).cloned() else {
            return;
        };
        if let Some(b) = self.blame(&t, need) {
            let name = match &value.kind {
                ExprKind::Ident(n) => n.as_str(),
                _ => VALUE,
            };
            self.crossing_error((name, &t), need, &b, value.span);
        }
    }

    /// `name` (a `t`) can't cross as `need` asks, blamed on `b`.
    fn crossing_error(&mut self, (name, t): (&str, &Type), need: Need, b: &Blame, at: Span) {
        let goes = match need {
            Need::Send => "can't go to another thread",
            Need::Share => "can't be reached by several threads at once",
        };
        let what = if b.path.is_empty() && b.culprit == *t.non_null() {
            format!("it's a `{}`", b.culprit)
        } else if b.path.is_empty() {
            format!("it holds a `{}`", b.culprit)
        } else {
            format!("`{}` is a `{}`", b.path.join("."), b.culprit)
        };
        self.report.error(
            script_problem::THREADS,
            at,
            format!("`{name}` {goes}: {what}, {} (or `unsafe {{ … }}`)", b.why),
        );
    }
}
