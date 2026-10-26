//! The overload the arguments fit, as C#'s: each argument converting to its
//! parameter, a generic method's type parameters inferred from them — a
//! lambda's last, typed by its delegate once that's known, its result
//! binding what's left (`Select(p => p.Name)` is a `Select<P, string>`).

use yog_ids::script_problem;

use super::Checker;
use super::generics::Bound;
use super::members::Signature;
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind};
use crate::source::token::Span;

/// The overload chosen: which, and its parameters with its type
/// parameters bound.
pub(super) struct Chosen {
    pub index: usize,
    pub params: Vec<Type>,
    /// Its own type parameters' types, in order (unknown where nothing said).
    pub type_args: Vec<Type>,
    /// Its `params` taking the arguments from this one on, packed in an
    /// array (its expanded form); `None` when they're as written.
    pub rest_from: Option<usize>,
}

/// `s` in its expanded form for `count` arguments: its `params T[]`
/// parameter one `T` for each argument past the others. None when it has
/// none, or fewer arguments are given than come before it.
fn expanded(s: &Signature, count: usize) -> Option<Signature> {
    let fixed = s.params.len().checked_sub(1)?;
    if !s.rest || count < fixed {
        return None;
    }
    let Type::Array(item, 1) = &s.params[fixed] else {
        return None;
    };
    let mut params = s.params[..fixed].to_vec();
    params.extend(std::iter::repeat_n((**item).clone(), count - fixed));
    let mut passes = s.passes[..fixed.min(s.passes.len())].to_vec();
    passes.resize(count, None);
    Some(Signature {
        params,
        passes,
        required: count,
        ret: s.ret.clone(),
        generics: s.generics.clone(),
        bound: s.bound.clone(),
        rest: false,
    })
}

impl Checker<'_, '_, '_> {
    /// The signature `given` arguments fit; its result — or a report when
    /// none takes that many, or the one that does gets a wrong type.
    pub(super) fn pick(
        &mut self,
        sigs: &[Signature],
        given: &[Type],
        args: &[&Arg],
        at: Span,
    ) -> (Type, Option<Chosen>) {
        if sigs.is_empty() || args.iter().any(|a| a.name.is_some()) {
            self.lambdas_alone(args);
            return (Type::Unknown, None);
        }
        // As written first, then a `params` one expanded (C#'s order).
        // A `params` one as written takes its array as its last argument.
        let as_written = |s: &Signature| !s.rest || given.len() == s.params.len();
        let written: Vec<_> = (0..sigs.len())
            .filter(|&i| as_written(&sigs[i]))
            .filter_map(|i| self.fit(&sigs[i], given, args).map(|f| (i, f, None)))
            .collect();
        let fits = if written.is_empty() {
            (0..sigs.len())
                .filter_map(|i| {
                    let x = expanded(&sigs[i], given.len())?;
                    self.fit(&x, given, args).map(|f| (i, f, Some(x)))
                })
                .collect()
        } else {
            written
        };
        let fitting = self.best(sigs, fits, given);
        if let Some((i, found, expanded)) = fitting {
            let rest_from = expanded.as_ref().map(|_| sigs[i].params.len() - 1);
            let s = expanded.as_ref().unwrap_or(&sigs[i]);
            self.check_passing(&s.passes, args, given);
            let params: Vec<Type> = s.params.iter().map(|p| p.substitute(&found)).collect();
            for ((arg, g), p) in args.iter().zip(given).zip(&params) {
                let group = self.is_group(&arg.value);
                if let ExprKind::Declare {
                    ty: None,
                    name,
                    mutable,
                } = &arg.value.kind
                {
                    // `out let x`: of the type its parameter gives.
                    self.bind_as(&name.text, p.clone(), !mutable);
                    self.types.set(&arg.value, p.clone());
                } else if is_lambda(&arg.value)
                    || is_collection(&arg.value)
                    || group
                    // A literal is the number its parameter wants (`1` an
                    // `i64`'s, a `T` bound to one too), as C#'s.
                    || (super::exprs::is_number_literal(&arg.value) && p.non_null().is_numeric())
                {
                    self.fits(&arg.value, p);
                } else {
                    self.note_conversion(&arg.value, g, p);
                }
            }
            let ret = s.ret.substitute(&found);
            let type_args = s
                .generics
                .iter()
                .map(|g| found.get(g).cloned().unwrap_or(Type::Unknown))
                .collect();
            return (
                ret,
                Some(Chosen {
                    index: i,
                    params,
                    type_args,
                    rest_from,
                }),
            );
        }
        self.none_fits(sigs, given, args, at)
    }

    /// Whether `s` takes `given`: its bound type parameters, if it does.
    /// The lambdas come last, typed by their delegates as far as known.
    fn fit(&mut self, s: &Signature, given: &[Type], args: &[&Arg]) -> Option<Bound> {
        if !(s.required..=s.params.len()).contains(&given.len()) {
            return None;
        }
        let mut found = s.bound.clone();
        let groups: Vec<bool> = args.iter().map(|a| self.is_group(&a.value)).collect();
        for ((g, p), a) in given.iter().zip(&s.params).zip(args) {
            // `[a, b]` is what's wanted: an array, a list, a set (C# 12's).
            if is_collection(&a.value) && collection_target(p) {
                continue;
            }
            if !is_lambda(&a.value) && !self.unify(p, g, &s.generics, &mut found) {
                return None;
            }
        }
        // Each argument must convert to its parameter (C#'s applicability).
        for ((g, p), a) in given.iter().zip(&s.params).zip(args) {
            if !self.takes(g, &p.substitute(&found), a) {
                return None;
            }
        }
        for ((p, a), _) in s
            .params
            .iter()
            .zip(args)
            .zip(&groups)
            .filter(|((_, a), group)| is_lambda(&a.value) || **group)
        {
            let Some((params, ret)) = self.lambda_sig(&p.substitute(&found)) else {
                continue;
            };
            // A lambda naming its parameters takes as many as the delegate gives.
            if !takes_as_many(&a.value, params.len()) {
                return None;
            }
            if params.iter().any(open) {
                continue;
            }
            let got = match self.group_result(&a.value, &params) {
                Some(given) => given,
                None => self.quietly(|c| c.lambda_result(&a.value, &params)),
            };
            if got.known() && !self.unify(&ret, &got, &s.generics, &mut found) {
                return None;
            }
        }
        Some(found)
    }

    /// Whether one of `sigs` takes `given` (as written, or `params` expanded).
    pub(super) fn fits_any(&mut self, sigs: &[Signature], given: &[Type], args: &[&Arg]) -> bool {
        sigs.iter().any(|s| {
            let as_written = !s.rest || given.len() == s.params.len();
            (as_written && self.fit(s, given, args).is_some())
                || expanded(s, given.len()).is_some_and(|x| self.fit(&x, given, args).is_some())
        })
    }

    /// Whether parameter `p` takes argument `a` (of type `g`): what's
    /// typed by it (a lambda, a method, `[…]`, `out let`, a number as
    /// written) or isn't known aside.
    fn takes(&mut self, g: &Type, p: &Type, a: &Arg) -> bool {
        let typed_by_it = is_lambda(&a.value)
            || is_collection(&a.value)
            || self.is_group(&a.value)
            || matches!(a.value.kind, ExprKind::Declare { .. })
            || (super::exprs::is_number_literal(&a.value) && p.non_null().is_numeric());
        typed_by_it || !g.known() || !p.known() || self.converts(g, p)
    }

    /// No overload fits: one taking as many checked against, or a report.
    fn none_fits(
        &mut self,
        sigs: &[Signature],
        given: &[Type],
        args: &[&Arg],
        at: Span,
    ) -> (Type, Option<Chosen>) {
        let takes = |s: &Signature| {
            (s.required..=s.params.len()).contains(&given.len())
                || (s.rest && given.len() + 1 >= s.params.len())
        };
        let taking: Vec<usize> = (0..sigs.len()).filter(|&i| takes(&sigs[i])).collect();
        match taking.as_slice() {
            [] => {
                self.lambdas_alone(args);
                self.report.error(
                    script_problem::WRONG_ARGS,
                    at,
                    format!("no overload takes {} arguments", given.len()),
                );
                (Type::Unknown, None)
            }
            &[i] => {
                let one = &sigs[i];
                for ((g, p), a) in given.iter().zip(&one.params).zip(args) {
                    if is_lambda(&a.value) || is_collection(&a.value) || self.is_group(&a.value) {
                        self.fits(&a.value, p);
                    } else {
                        self.want(g, p, a.value.span);
                    }
                }
                let params = one.params.clone();
                let type_args = one
                    .generics
                    .iter()
                    .map(|g| one.bound.get(g).cloned().unwrap_or(Type::Unknown))
                    .collect();
                (
                    one.ret.clone(),
                    Some(Chosen {
                        index: i,
                        params,
                        type_args,
                        rest_from: None,
                    }),
                )
            }
            _ => {
                self.lambdas_alone(args);
                let types: Vec<String> = given.iter().map(ToString::to_string).collect();
                self.report.error(
                    script_problem::TYPE_MISMATCH,
                    at,
                    format!("no overload takes these: ({})", types.join(", ")),
                );
                (Type::Unknown, None)
            }
        }
    }

    /// The lambdas among `args`, no delegate chosen for them: typed alone,
    /// what's wrong in them reported.
    pub(super) fn lambdas_alone(&mut self, args: &[&Arg]) {
        for a in args.iter().filter(|a| is_lambda(&a.value)) {
            self.expr(&a.value);
        }
    }
}

pub(super) fn is_lambda(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Lambda { .. })
}

/// Whether `t` still names a type parameter not bound.
fn open(t: &Type) -> bool {
    match t {
        Type::Param(_) => true,
        Type::Own(_, args) | Type::Tuple(args, _) => args.iter().any(open),
        Type::Array(item, _) | Type::Nullable(item) => open(item),
        Type::Fn(params, ret) => params.iter().any(open) || open(ret),
        _ => false,
    }
}

/// Whether lambda `e` takes `count` parameters: as many named, or none
/// named and one (`it`) or none taken.
fn takes_as_many(e: &Expr, count: usize) -> bool {
    match &e.kind {
        ExprKind::Lambda { params, .. } if params.is_empty() => count <= 1,
        ExprKind::Lambda { params, .. } => params.len() == count,
        _ => true,
    }
}

/// `[a, b]`: a collection expression, typed by where it goes.
fn is_collection(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Collection(_))
}

/// Whether a collection expression can be what `p` is: an array or a type.
fn collection_target(p: &Type) -> bool {
    matches!(p.non_null(), Type::Array(..) | Type::Own(..))
}
