//! Calls: the overload the arguments fit chosen (a lambda typed by the
//! delegate it's handed as), its arguments checked against it, and which
//! method it was noted for the compiler (`Types::method`).

use yog_ids::script_problem;

use super::Checker;
use super::context::Context;
use super::members::{Callee, Signature};
use super::model::Type;
use super::types::MethodUse;
use crate::ast::{Arg, Expr, ExprKind, Method};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// A call: its arguments checked against the callee's parameters; its result.
    pub(super) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Type {
        // `F<T>(…)`: the method's type parameters given, not inferred.
        let (callee, explicit) = match &callee.kind {
            ExprKind::TypeArgs { expr, args } => {
                (&**expr, args.iter().map(|t| self.type_of(t)).collect())
            }
            _ => (callee, Vec::new()),
        };
        let target = self.callee(callee);
        // A lambda's own type, for choosing the overload only: it's typed
        // against the delegate the chosen one takes (`pick`).
        let given: Vec<Type> = args
            .iter()
            .map(|a| match a.value.kind {
                ExprKind::Lambda { .. } => self.quietly(|c| c.expr(&a.value)),
                _ => self.expr(&a.value),
            })
            .collect();
        let (t, used) = self.called_with(target, (&given, args), &explicit, callee.span);
        if let Some(used) = used {
            self.types.set_method(e, used);
        }
        t
    }

    /// `target` called with `args` (typed `given`): its result, and the
    /// method chosen when it's the source's own (the library's too).
    pub(super) fn called(
        &mut self,
        target: Callee,
        given: &[Type],
        args: &[Arg],
        at: Span,
    ) -> (Type, Option<MethodUse>) {
        self.called_with(target, (given, args), &[], at)
    }

    /// [`Self::called`] with type arguments given (`F<i32>(…)`): each
    /// method's own type parameters, in order, bound by them.
    fn called_with(
        &mut self,
        target: Callee,
        (given, args): (&[Type], &[Arg]),
        explicit: &[Type],
        at: Span,
    ) -> (Type, Option<MethodUse>) {
        match target {
            Callee::Own(methods) => {
                let sigs: Vec<Signature> = methods
                    .iter()
                    .map(|(m, context)| self.in_context(context, |c| c.signature(m)))
                    .map(|s| s.given(explicit))
                    .collect();
                let (t, chosen) = self.pick(&sigs, given, args, at);
                let used = chosen.map(|i| self.method_use(&methods[i].0, &methods[i].1));
                (t, used)
            }
            Callee::Typed(sigs) => (self.pick(&sigs, given, args, at).0, None),
            Callee::Unknown => (Type::Unknown, None),
        }
    }

    /// Method `m` (its declaration's `context`) as a call uses it: whose,
    /// which of its name, its signature as declared.
    fn method_use(&mut self, m: &Method, context: &Context) -> MethodUse {
        let owner = context.owner.clone();
        let (params, ret) = match &owner {
            Some(key) => {
                let generics = self
                    .own
                    .of_key(key)
                    .map(|o| o.generics.clone())
                    .unwrap_or_default();
                self.as_declared(&key.name, &generics, |c| c.signature_types(m))
            }
            None => self.signature_types(m),
        };
        let overload = owner
            .as_ref()
            .and_then(|k| self.own.of_key(k))
            .and_then(|o| o.methods.get(&m.name.text))
            .and_then(|all| all.iter().position(|x| x == m))
            .unwrap_or_default();
        MethodUse {
            owner: owner.map(|k| k.name),
            name: m.name.text.clone(),
            overload,
            params,
            ret,
        }
    }

    /// An own method's signature: parameters, how many are required (the
    /// rest have defaults), result.
    pub(super) fn signature(&mut self, m: &Method) -> Signature {
        let bound = super::generics::params_of(&m.generics);
        // In the order written: `F<A, B>(…)` gives them so.
        let generics = m
            .generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| g.name.text.clone())
            .collect();
        self.with_types(bound, |c| Signature {
            params: m.params.iter().map(|p| c.type_of(&p.ty)).collect(),
            passes: m.params.iter().map(|p| p.pass).collect(),
            required: m.params.iter().take_while(|p| p.default.is_none()).count(),
            ret: m.ret.as_ref().map_or(Type::Void, |t| c.type_of(t)),
            generics,
        })
    }

    /// The signature `given` arguments fit; its result — or a report when
    /// none takes that many, or the one that does gets a wrong type.
    fn pick(
        &mut self,
        sigs: &[Signature],
        given: &[Type],
        args: &[Arg],
        at: Span,
    ) -> (Type, Option<usize>) {
        if sigs.is_empty() || args.iter().any(|a| a.name.is_some()) {
            self.lambdas_alone(args);
            return (Type::Unknown, None);
        }
        let takes = |s: &Signature| (s.required..=s.params.len()).contains(&given.len());
        let fits = |s: &Signature| {
            let mut found = super::generics::Bound::new();
            let all = takes(s)
                && given
                    .iter()
                    .zip(&s.params)
                    .all(|(g, p)| self.unify(p, g, &s.generics, &mut found));
            all.then_some(found)
        };
        if let Some((i, found)) = sigs
            .iter()
            .enumerate()
            .find_map(|(i, s)| fits(s).map(|f| (i, f)))
        {
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
            return (s.ret.substitute(&found), Some(i));
        }
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
                (one.ret.clone(), Some(i))
            }
            _ => {
                self.lambdas_alone(args);
                (Type::Unknown, None)
            }
        }
    }

    /// The lambdas among `args`, no delegate chosen for them: typed alone,
    /// what's wrong in them reported.
    fn lambdas_alone(&mut self, args: &[Arg]) {
        for a in args.iter().filter(|a| is_lambda(&a.value)) {
            self.expr(&a.value);
        }
    }
}

fn is_lambda(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Lambda { .. })
}

impl Signature {
    /// The signature with its own type parameters bound by `explicit` (in
    /// order), none left to infer; as it is when none are given. One of
    /// another count of them takes nothing.
    fn given(self, explicit: &[Type]) -> Signature {
        if explicit.is_empty() {
            return self;
        }
        if explicit.len() != self.generics.len() {
            return Signature {
                required: usize::MAX,
                ..self
            };
        }
        let bound = self
            .generics
            .iter()
            .cloned()
            .zip(explicit.iter().cloned())
            .collect();
        Signature {
            params: self.params.iter().map(|p| p.substitute(&bound)).collect(),
            ret: self.ret.substitute(&bound),
            generics: Vec::new(),
            ..self
        }
    }
}
