//! Statements: locals typed by what's written or their value, what's
//! assigned fitting the place, conditions `bool`, `return` the method's type.

use super::Checker;
use super::model::Type;
use crate::ast::{self, BinOp, Binding, Block, Expr, ExprKind, Pattern, Stmt};
use yog_script_ids::system::ENUMERABLE;

impl Checker<'_, '_, '_> {
    pub(super) fn block(&mut self, b: &Block) {
        self.scoped(|c| c.stmts(&b.stmts));
    }

    pub(super) fn stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local {
                ty,
                binding,
                value,
                mutable,
            } => self.local_decl(ty.as_ref(), binding, value.as_ref(), *mutable),
            Stmt::Assign { place, op, value } => {
                let t = self.expr(place);
                self.written(place);
                // `=`, and `+=`/`-=` on a delegate (an event's subscription):
                // the value must be one.
                match op {
                    Some(op) if self.delegate_sig(&t).is_none() => {
                        self.compound(place, *op, &t, value);
                    }
                    _ => {
                        self.fits(value, &t);
                        self.handed(value, &t);
                    }
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.scoped(|c| {
                    c.patterns_in(cond);
                    c.condition(cond);
                    c.branch(then);
                });
                if let Some(o) = otherwise {
                    self.branch(o);
                }
            }
            Stmt::Foreach {
                ty,
                binding,
                iter,
                body,
                mutable,
                awaits,
            } => self.foreach((ty.as_ref(), binding, *mutable), (iter, *awaits), body),
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.for_loop((init.as_deref(), cond.as_ref(), step.as_deref()), body),
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => self.scoped(|c| {
                c.condition(cond);
                c.branch(body);
            }),
            Stmt::Block(b) => self.block(b),
            Stmt::Yield(v, at) => self.yielded(v.as_ref(), *at),
            Stmt::Lock {
                target,
                value,
                body,
                ..
            } => self.locked(target, value.as_ref(), body),
            Stmt::Return(Some(v), at) => self.returned(v, *at),
            Stmt::Skip(Some(v), _) | Stmt::Expr(v) | Stmt::Value(v) => {
                self.expr(v);
            }
            Stmt::Return(None, _) | Stmt::Skip(None, _) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    /// `T x = v;` / `let x = v;`: the value fits the type written, or gives it.
    fn local_decl(
        &mut self,
        ty: Option<&ast::Type>,
        binding: &Binding,
        value: Option<&Expr>,
        mutable: bool,
    ) {
        let declared = ty.map(|t| self.type_of(t));
        let t = match (declared, value) {
            (Some(d), Some(v)) => {
                self.fits(v, &d);
                d
            }
            (Some(d), None) => d,
            (None, Some(v)) => self.expr(v),
            (None, None) => Type::Unknown,
        };
        if let Some(v) = value {
            self.handed(v, &t);
        }
        self.bind_binding(binding, &t, !mutable);
    }

    /// `foreach (T x in items)`: `x` an item of what's gone through (an
    /// `IEnumerable<T>`'s `T`), or of the type written.
    fn foreach(
        &mut self,
        (ty, binding, mutable): (Option<&ast::Type>, &Binding, bool),
        (iter, awaits): (&Expr, bool),
        body: &Stmt,
    ) {
        let items = self.expr(iter);
        // `await foreach`: an `IAsyncEnumerable<T>`'s items.
        let item = if awaits {
            self.sequence_item(&items, yog_script_ids::system::ASYNC_ENUMERABLE)
        } else {
            self.item_of(&items)
        };
        let t = ty.map_or(item, |t| self.type_of(t));
        self.types.set_item(iter, t.clone());
        self.scoped(|c| {
            c.bind_binding(binding, &t, !mutable);
            c.branch(body);
        });
    }

    /// `return v;`: what the method gives (inferred for a lambda's); none in an iterator.
    fn returned(&mut self, v: &Expr, at: crate::source::token::Span) {
        if self.yields.is_some() {
            self.expr(v);
            return self.returned_from_iterator(at);
        }
        match self.returns.clone() {
            Some(r) if r != Type::Void => self.fits(v, &r),
            _ => {
                let t = self.expr(v);
                if let Some(inferred) = &mut self.inferred {
                    inferred.push(t);
                }
            }
        }
    }

    /// `lock (target, ref let value) body`: `target` a `Lock<T>`, `value` its `T`, changeable.
    fn locked(&mut self, target: &Expr, value: Option<&crate::ast::Name>, body: &Stmt) {
        let t = self.expr(target);
        let held = match t.non_null() {
            Type::Own(name, args) if name == yog_script_ids::system::LOCK && args.len() == 1 => {
                args[0].clone()
            }
            _ => {
                self.report.error(
                    yog_ids::script_problem::TYPE_MISMATCH,
                    target.span,
                    format!("`lock` holds a `Lock<T>`, not a `{t}`"),
                );
                Type::Unknown
            }
        };
        self.scoped(|c| {
            if let Some(n) = value {
                c.bind_as(&n.text, held, false);
            }
            c.branch(body);
        });
    }

    /// `for (init; cond; step) body`, in a scope of its own.
    fn for_loop(
        &mut self,
        (init, cond, step): (Option<&Stmt>, Option<&Expr>, Option<&Stmt>),
        body: &Stmt,
    ) {
        self.scoped(|c| {
            if let Some(i) = init {
                c.stmt(i);
            }
            if let Some(cond) = cond {
                c.condition(cond);
            }
            if let Some(s) = step {
                c.stmt(s);
            }
            c.branch(body);
        });
    }

    fn branch(&mut self, s: &Stmt) {
        self.scoped(|c| c.stmt(s));
    }

    /// Record `name<args>`'s constructor's parameters' types (none for
    /// what isn't a record).
    fn record_params(&mut self, name: &str, args: &[Type]) -> Vec<Type> {
        let Some(params) = self
            .own
            .get(name, args.len())
            .filter(|o| o.record)
            .map(|o| o.ctor.clone())
        else {
            return Vec::new();
        };
        let context = self.context_of(name, args);
        self.in_context(&context, |c| {
            params.iter().map(|p| c.type_of(&p.ty)).collect()
        })
    }

    fn bind_binding(&mut self, b: &Binding, t: &Type, fixed: bool) {
        match (b, t) {
            (Binding::One(n), _) => self.bind_as(&n.text, t.clone(), fixed),
            (Binding::Tuple(names), Type::Tuple(items, _)) if names.len() == items.len() => {
                for (n, item) in names.iter().zip(items) {
                    self.types.set_binding(n, item.clone());
                    self.bind_as(&n.text, item.clone(), fixed);
                }
            }
            // A record taken apart by its constructor's parameters, as C#'s
            // `Deconstruct` (`foreach (let (k, v) in dict)`).
            (Binding::Tuple(names), Type::Own(record, args))
                if self.record_params(record, args).len() == names.len() =>
            {
                for (n, item) in names.iter().zip(self.record_params(record, args)) {
                    self.types.set_binding(n, item.clone());
                    self.bind_as(&n.text, item, fixed);
                }
            }
            (Binding::Tuple(names), _) => names
                .iter()
                .for_each(|n| self.bind_as(&n.text, Type::Unknown, fixed)),
        }
    }

    /// The names `is` patterns in a condition bind, typed, for the branch
    /// it guards (`if (target is Player p && …)`).
    fn patterns_in(&mut self, e: &Expr) {
        match &e.kind {
            // Typed by what's matched, looked at quietly: it's checked after.
            ExprKind::Is { expr, pattern } => {
                let on = self.quietly(|c| c.expr(expr));
                self.bind_arm(pattern, &on);
            }
            ExprKind::Binary { left, right, .. } => {
                self.patterns_in(left);
                self.patterns_in(right);
            }
            _ => {}
        }
    }

    pub(super) fn bind_pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Type { ty, bind: Some(n) } => {
                let t = self.type_of(ty);
                self.types.set_binding(n, t.clone());
                self.bind_as(&n.text, t, true);
            }
            Pattern::Let(n) => self.bind_as(&n.text, Type::Unknown, true),
            Pattern::And(a, b) | Pattern::Or(a, b) => {
                self.bind_pattern(a);
                self.bind_pattern(b);
            }
            _ => {}
        }
    }

    /// `place op= value` (`place` of `t`): `place op value` typed, as C#'s,
    /// its result going back into `place` — a number's narrowed when
    /// `value` itself fits there (`b &= mask` on a `u8`), as a literal does.
    fn compound(&mut self, place: &Expr, op: BinOp, t: &Type, value: &Expr) {
        let v = self.expr(value);
        let r = match self.declared_binary(op, t, &v) {
            Some((used, ret)) => {
                self.types.set_assigned(place, used);
                ret
            }
            None => self.combined(op, (t.clone(), v.clone()), (place.span, value.span)),
        };
        let narrowed = r.is_numeric()
            && t.is_numeric()
            && (super::exprs::is_number_literal(value) || self.converts(&v, t));
        if !narrowed {
            self.want(&r, t, value.span);
        }
    }

    /// What going through a `t` gives: an array's item, a string's `char`,
    /// the `T` of the `IEnumerable<T>` it is.
    pub(super) fn item_of(&self, t: &Type) -> Type {
        match t.non_null() {
            Type::Array(item, _) => (**item).clone(),
            Type::String => Type::Char,
            _ => self.sequence_item(t, ENUMERABLE),
        }
    }

    /// The `T` of sequence interface `of<T>` (`IEnumerable`,
    /// `IAsyncEnumerable`) that `t` is or implements; unknown when it isn't one.
    pub(super) fn sequence_item(&self, t: &Type, of: &str) -> Type {
        match t.non_null() {
            Type::Own(name, args) => self
                .lineage_of(name, args)
                .into_iter()
                .find(|(k, _)| k.name == of && k.arity == 1)
                .and_then(|(_, args)| args.into_iter().next())
                .unwrap_or(Type::Unknown),
            _ => Type::Unknown,
        }
    }
}
