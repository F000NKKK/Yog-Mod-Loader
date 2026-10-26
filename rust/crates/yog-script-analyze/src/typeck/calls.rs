//! Calls: the overload the arguments fit chosen (a lambda typed by the
//! delegate it's handed as), its arguments checked against it, and which
//! method it was noted for the compiler (`Types::method`).

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
        // `t"key"(a, b)`: the text with its arguments (`Text.With`), any.
        if let ExprKind::Text(_) = &callee.kind {
            let text = self.expr(callee);
            for a in args {
                self.expr(&a.value);
            }
            return text;
        }
        if let Some(t) = self.message_call(e, callee, args) {
            return t;
        }
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
        let mut given: Vec<Type> = args
            .iter()
            .map(|a| match a.value.kind {
                ExprKind::Lambda { .. } => self.quietly(|c| c.expr(&a.value)),
                _ => self.expr(&a.value),
            })
            .collect();
        let mut args: Vec<&Arg> = args.iter().collect();
        let target = self.instance_or_extension(target, callee, (&given, &args), &explicit);
        // `xs.Where(…)`, an extension method: the receiver its first argument.
        let receiver = match (&target, &callee.kind) {
            (Callee::Extension(_), ExprKind::Member { expr, .. }) => {
                let t = self.types.of(expr).cloned().unwrap_or(Type::Unknown);
                given.insert(0, t.clone());
                Some((
                    &**expr,
                    t,
                    Arg {
                        name: None,
                        pass: None,
                        value: (**expr).clone(),
                    },
                ))
            }
            _ => None,
        };
        if let Some((_, _, arg)) = &receiver {
            args.insert(0, arg);
        }
        let (t, used) = self.called_with(target, (&given, &args), &explicit, callee.span);
        if let (Some((expr, got, _)), Some(used)) = (&receiver, &used) {
            // The receiver's own conversion, the copy handed to `pick` aside.
            if let Some(param) = used.taken.first() {
                self.note_conversion(expr, got, param);
            }
        }
        if let Some(used) = used {
            self.crossing_call(&used, &args);
            self.call_meets(&used, callee.span);
            self.types.set_method(e, used);
        }
        t
    }

    /// Own `target` called with `args` (typed `given`): its result; the
    /// method chosen noted as a call's.
    pub(super) fn called_with_own(
        &mut self,
        target: Callee,
        (given, args): (&[Type], &[&Arg]),
        at: Span,
    ) -> Type {
        let (t, _) = self.called_with(target, (given, args), &[], at);
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
        let args: Vec<&Arg> = args.iter().collect();
        self.called_with(target, (given, &args), &[], at)
    }

    /// [`Self::called`] with type arguments given (`F<i32>(…)`): each
    /// method's own type parameters, in order, bound by them.
    fn called_with(
        &mut self,
        target: Callee,
        (given, args): (&[Type], &[&Arg]),
        explicit: &[Type],
        at: Span,
    ) -> (Type, Option<MethodUse>) {
        match target {
            Callee::Own(methods) => self.own_called(&methods, false, (given, args), explicit, at),
            Callee::Extension(methods) => {
                self.own_called(&methods, true, (given, args), explicit, at)
            }
            Callee::Typed(sigs) => (self.pick(&sigs, given, args, at).0, None),
            Callee::Unknown => (Type::Unknown, None),
        }
    }

    /// `on.name(args)`'s methods (C#'s): its type's when one of them takes
    /// the arguments, else the extension methods of the name.
    fn instance_or_extension(
        &mut self,
        target: Callee,
        callee: &Expr,
        (given, args): (&[Type], &[&Arg]),
        explicit: &[Type],
    ) -> Callee {
        let (Callee::Own(methods), ExprKind::Member { expr, name, .. }) = (&target, &callee.kind)
        else {
            return target;
        };
        if matches!(self.types.of(expr), Some(Type::Static(..)) | None) {
            return target;
        }
        let sigs: Vec<Signature> = methods
            .iter()
            .map(|(m, context)| self.in_context(context, |c| c.signature(m)))
            .map(|s| s.given(explicit))
            .collect();
        let fits = self.quietly(|c| c.fits_any(&sigs, given, args));
        if fits {
            return target;
        }
        match self.quietly(|c| c.extension_or_unknown(&Type::Unknown, name)) {
            Callee::Extension(ext) => Callee::Extension(ext),
            _ => target,
        }
    }

    /// One of own `methods` (extension methods: `extension`) called with
    /// `args`: its result, the one chosen.
    fn own_called(
        &mut self,
        methods: &[(Method, Context)],
        extension: bool,
        (given, args): (&[Type], &[&Arg]),
        explicit: &[Type],
        at: Span,
    ) -> (Type, Option<MethodUse>) {
        let sigs: Vec<Signature> = methods
            .iter()
            .map(|(m, context)| self.in_context(context, |c| c.signature(m)))
            .map(|s| s.given(explicit))
            .collect();
        let (t, chosen) = self.pick(&sigs, given, args, at);
        let used = chosen.map(|c| {
            let (m, context) = &methods[c.index];
            MethodUse {
                extension,
                taken: c.params,
                type_args: c.type_args,
                rest_from: c.rest_from,
                ..self.method_use(m, context)
            }
        });
        (t, used)
    }

    /// Method `m` (its declaration's `context`) as a call uses it: whose,
    /// which of its name, its signature as declared.
    pub(super) fn method_use(&mut self, m: &Method, context: &Context) -> MethodUse {
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
            owner_arity: owner.as_ref().map_or(0, |k| k.arity),
            owner: owner.map(|k| k.name),
            name: m.name.text.clone(),
            overload,
            params,
            ret,
            taken: Vec::new(),
            type_args: Vec::new(),
            extension: false,
            generics: m.generics.iter().filter(|g| !g.lifetime).count(),
            rest_from: None,
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
            required: m
                .params
                .iter()
                .take_while(|p| p.default.is_none() && !p.params)
                .count(),
            ret: m.ret.as_ref().map_or(Type::Void, |t| c.type_of(t)),
            generics,
            bound: Default::default(),
            rest: m.params.last().is_some_and(|p| p.params),
        })
    }
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
        let bound: super::generics::Bound = self
            .generics
            .iter()
            .cloned()
            .zip(explicit.iter().cloned())
            .collect();
        Signature {
            params: self.params.iter().map(|p| p.substitute(&bound)).collect(),
            ret: self.ret.substitute(&bound),
            bound,
            ..self
        }
    }
}
