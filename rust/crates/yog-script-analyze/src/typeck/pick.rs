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
        let fitting = (0..sigs.len()).find_map(|i| self.fit(&sigs[i], given, args).map(|f| (i, f)));
        if let Some((i, found)) = fitting {
            let s = &sigs[i];
            self.check_passing(&s.passes, args, given);
            let params: Vec<Type> = s.params.iter().map(|p| p.substitute(&found)).collect();
            for ((arg, g), p) in args.iter().zip(given).zip(&params) {
                if is_lambda(&arg.value) {
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
        for ((g, p), a) in given.iter().zip(&s.params).zip(args) {
            if !is_lambda(&a.value) && !self.unify(p, g, &s.generics, &mut found) {
                return None;
            }
        }
        for (p, a) in s
            .params
            .iter()
            .zip(args)
            .filter(|(_, a)| is_lambda(&a.value))
        {
            let Some((params, ret)) = self.delegate_sig(&p.substitute(&found)) else {
                continue;
            };
            // A lambda naming its parameters takes as many as the delegate gives.
            if !takes_as_many(&a.value, params.len()) {
                return None;
            }
            if params.iter().any(open) {
                continue;
            }
            let got = self.quietly(|c| c.lambda_result(&a.value, &params));
            if got.known() && !self.unify(&ret, &got, &s.generics, &mut found) {
                return None;
            }
        }
        Some(found)
    }

    /// No overload fits: one taking as many checked against, or a report.
    fn none_fits(
        &mut self,
        sigs: &[Signature],
        given: &[Type],
        args: &[&Arg],
        at: Span,
    ) -> (Type, Option<Chosen>) {
        let takes = |s: &Signature| (s.required..=s.params.len()).contains(&given.len());
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
                    if is_lambda(&a.value) {
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
