//! Members and calls: a member of the source's own types (their
//! components' too) or of a game class (by C#'s names); a call matched to
//! the method's parameters.

use yog_ids::script_problem;

use super::Checker;
use super::game::{self, Found};
use super::generics::Bound;
use super::model::Type;
use super::own::TypeKey;
use crate::ast::{Arg, Expr, ExprKind, Method, Pass};
use crate::source::token::Span;

/// What a call's callee stands for.
pub(super) enum Callee {
    /// The source's own methods, each with the type parameters its type
    /// binds (`Box<i32>`'s `T` is `i32`; an inherited one's, by the
    /// arguments its base is given: `Dictionary<K, V>`'s `ICollection`'s
    /// `T` is `KeyValuePair<K, V>`).
    Own(Vec<(Method, Bound)>),
    /// Methods typed already: a game class's, Yog's.
    Typed(Vec<Signature>),
    Unknown,
}

/// What a call is matched against.
pub(super) struct Signature {
    pub params: Vec<Type>,
    /// How each parameter takes its argument (`in`, `ref`, …): only the
    /// source's own methods say.
    pub passes: Vec<Option<Pass>>,
    pub required: usize,
    pub ret: Type,
    /// A generic method's type parameters, inferred from a call.
    pub generics: Vec<String>,
}

impl Checker<'_, '_, '_> {
    /// `on.name` (`on?.name` when `safe`): its type; what's missing reported.
    pub(super) fn member(&mut self, on: &Type, name: &str, safe: bool, at: Span) -> Type {
        if is_side_call(name) {
            return Type::Unknown;
        }
        if on.is_nullable() && !safe {
            self.report.error(
                script_problem::NULLABLE,
                at,
                format!("this may be `null`: `?.{name}` reads `{name}` only when it isn't"),
            );
        }
        let t = match on.non_null() {
            Type::Static(of) => self.own_member(&Type::own(of.clone()), name),
            Type::Own(..) => {
                let t = self.own_member(on.non_null(), name);
                if !t.known() && !self.own_has_method(on.non_null(), name) {
                    self.unknown_member(on.non_null(), name, at);
                }
                t
            }
            Type::Game(class) => match self.extension_value(class, name) {
                Some(t) => t,
                None => self.game_value(class, name, at),
            },
            Type::Namespace(ns) => self.in_namespace(&super::names::split(ns), name, at),
            Type::Yog(type_name) => match self.extension_value(type_name, name) {
                Some(t) => t,
                None => self.yog_value(type_name, name, at),
            },
            _ => Type::Unknown,
        };
        if safe { t.nullable() } else { t }
    }

    /// The type of member `name` of the source's own type `t` (or of a
    /// component it has, or the component itself).
    pub(super) fn own_member(&mut self, t: &Type, name: &str) -> Type {
        let (type_name, args) = match t {
            Type::Own(n, args) => (n, args),
            Type::Game(class) | Type::Yog(class) => {
                return self.extension_value(class, name).unwrap_or(Type::Unknown);
            }
            _ => return Type::Unknown,
        };
        for (key, args) in self.lineage_of(type_name, args) {
            let bound = self.bound_of(&key.name, &args);
            let found = self.with_types(bound, |c| c.own_member_of(&key, name));
            if found.known() {
                return found;
            }
        }
        Type::Unknown
    }

    /// Member `name` declared by own type `key` itself (or by a component
    /// it has); a case is a value of the enum.
    fn own_member_of(&mut self, key: &TypeKey, name: &str) -> Type {
        let Some(own) = self.own.of_key(key) else {
            return Type::Unknown;
        };
        if let Some(written) = own.values.get(name).cloned() {
            return self.type_of(&written);
        }
        if own.cases.iter().any(|c| c == name) {
            return Type::own(key.name.clone());
        }
        let components = own.components.clone();
        if components.iter().any(|c| c == name) {
            return Type::own(name.to_string());
        }
        components
            .iter()
            .map(|c| self.own_member(&Type::own(c.clone()), name))
            .find(Type::known)
            .unwrap_or(Type::Unknown)
    }

    /// What `extend` adds to game class `class`: member `name`'s type, a
    /// component it has, or one of the component's members.
    fn extension_value(&mut self, class: &str, name: &str) -> Option<Type> {
        let ext = self.own.extended.get(crate::index::simple_name(class))?;
        if let Some(written) = ext.values.get(name).cloned() {
            return Some(self.type_of(&written));
        }
        let components = ext.components.clone();
        if components.iter().any(|c| c == name) {
            return Some(Type::own(name.to_string()));
        }
        components
            .iter()
            .map(|c| self.own_member(&Type::own(c.clone()), name))
            .find(Type::known)
    }

    fn extension_methods(&self, class: &str, name: &str) -> Option<Vec<(Method, Bound)>> {
        let ext = self.own.extended.get(crate::index::simple_name(class))?;
        match ext.methods.get(name) {
            Some(methods) => Some(unbound(methods)),
            None => ext
                .components
                .iter()
                .find_map(|c| self.own_methods(&Type::own(c.clone()), name)),
        }
    }

    fn own_has_method(&self, t: &Type, name: &str) -> bool {
        self.own_methods(t, name).is_some()
    }

    fn game_value(&mut self, class: &str, name: &str, at: Span) -> Type {
        let Some(lookup) = &self.lookup else {
            return Type::Unknown;
        };
        let Some(c) = lookup.class(class) else {
            return Type::Unknown;
        };
        match game::member(lookup, c, name, false) {
            Found::Value(t) => t,
            Found::Nothing if matches!(game::member(lookup, c, name, true), Found::Nothing) => {
                self.unknown_member(&Type::Game(class.to_string()), name, at);
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    /// Member `name` of Yog's `type_name`: a struct's field (`Health` is
    /// `health`), an enum's case, a group's group (`Client.Control`); a
    /// method is reached by calling it.
    fn yog_value(&mut self, type_name: &str, name: &str, at: Span) -> Type {
        let surface = &*super::surface::SURFACE;
        if let Some(s) = surface.structs.get(type_name) {
            if let Some(f) = s
                .fields
                .iter()
                .find(|f| yog_codegen::pascal(f.name) == name)
            {
                return Type::of_shape(&(f.shape)());
            }
        } else if let Some(e) = surface.enums.get(type_name) {
            if e.variants
                .iter()
                .any(|v| yog_codegen::pascal(v.name) == name)
            {
                return Type::Yog(type_name.to_string());
            }
        } else {
            let sub = super::surface::subgroup(type_name, name);
            if surface.groups.contains_key(&sub) {
                return Type::Yog(sub);
            }
            // An event: a delegate taking its arguments (`Player.Join += e => …`).
            if let Some(args) = surface
                .groups
                .get(type_name)
                .and_then(|g| g.events.get(name))
            {
                return Type::Fn(vec![Type::Yog(args.clone())], Box::new(Type::Void));
            }
            if surface
                .groups
                .get(type_name)
                .is_some_and(|g| g.methods.contains_key(name))
            {
                return Type::Unknown;
            }
        }
        self.unknown_member(&Type::Yog(type_name.to_string()), name, at);
        Type::Unknown
    }

    fn unknown_member(&mut self, on: &Type, name: &str, at: Span) {
        self.report.error(
            script_problem::UNKNOWN_MEMBER,
            at,
            format!("`{on}` has no member `{name}`"),
        );
    }

    /// A call: its arguments checked against the callee's parameters; its result.
    pub(super) fn call(&mut self, callee: &Expr, args: &[Arg]) -> Type {
        let target = self.callee(callee);
        let given: Vec<Type> = args.iter().map(|a| self.expr(&a.value)).collect();
        self.called(target, &given, args, callee.span)
    }

    /// `target` called with `args` (typed `given`): its result.
    pub(super) fn called(
        &mut self,
        target: Callee,
        given: &[Type],
        args: &[Arg],
        at: Span,
    ) -> Type {
        match target {
            Callee::Own(methods) => {
                let sigs: Vec<Signature> = methods
                    .iter()
                    .map(|(m, bound)| self.with_types(bound.clone(), |c| c.signature(m)))
                    .collect();
                self.pick(&sigs, given, args, at)
            }
            Callee::Typed(sigs) => self.pick(&sigs, given, args, at),
            Callee::Unknown => Type::Unknown,
        }
    }

    /// What `callee` calls.
    fn callee(&mut self, callee: &Expr) -> Callee {
        match &callee.kind {
            // A delegate called: `handler(x)`.
            ExprKind::Ident(name) if self.delegate(name).is_some() => {
                // The name's type kept: a compiler calls a delegate by it.
                let t = self.expr(callee);
                self.types.set(callee, t);
                self.delegate(name)
                    .map_or(Callee::Unknown, |sig| Callee::Typed(vec![sig]))
            }
            // `Charged?.Invoke(…)`: a delegate's own call.
            ExprKind::Member { expr, name, .. } if name.text == crate::system::INVOKE => {
                let t = self.expr(expr);
                match self.delegate_sig(&t) {
                    Some((params, ret)) => Callee::Typed(vec![signature_of(&params, &ret)]),
                    None => Callee::Unknown,
                }
            }
            ExprKind::Ident(name) if self.local(name).is_none() => {
                let methods = self.callable(name);
                if methods.is_empty() {
                    // `Ok(v)`, `Err(e)`: the prelude's cases, by their names.
                    match self.variant_ctor(crate::system::RESULT, name) {
                        Callee::Unknown => self.static_import(name, callee.span),
                        made => made,
                    }
                } else {
                    if let Some(Type::Own(of, _)) = self.this.clone()
                        && self.own_methods(&Type::own(of.clone()), name).is_some()
                    {
                        self.called_on(None, &of, name, callee.span);
                    }
                    Callee::Own(methods)
                }
            }
            ExprKind::Member { expr, name, .. } => {
                let on = self.expr(expr);
                match on.non_null() {
                    Type::Own(of, _) | Type::Static(of) => {
                        let on_type = Type::Own(of.clone(), own_args(on.non_null()).to_vec());
                        match self.own_methods(&on_type, &name.text) {
                            Some(methods) => {
                                if matches!(on.non_null(), Type::Own(..)) {
                                    self.called_on(Some(expr), of, &name.text, name.span);
                                }
                                Callee::Own(methods)
                            }
                            None => self.variant_ctor(&of.clone(), &name.text),
                        }
                    }
                    Type::Yog(group) => match self.extension_methods(group, &name.text) {
                        Some(m) => Callee::Own(m),
                        None => self.yog_methods(group, &name.text, name.span),
                    },
                    Type::Game(class) => match self.extension_methods(class, &name.text) {
                        Some(m) => Callee::Own(m),
                        None if is_side_call(&name.text) => Callee::Unknown,
                        None => self.game_methods(class, &name.text, name.span),
                    },
                    _ => Callee::Unknown,
                }
            }
            _ => {
                self.expr(callee);
                Callee::Unknown
            }
        }
    }

    /// The source's own methods `name` reachable unqualified here: `this`'s
    /// (its components' and extensions' too), else the file's.
    pub(super) fn callable(&self, name: &str) -> Vec<(Method, Bound)> {
        let in_this = self.this.clone().and_then(|t| match &t {
            Type::Game(class) | Type::Yog(class) => self.extension_methods(class, name),
            _ => self.own_methods(&t, name),
        });
        in_this
            .or_else(|| self.own.methods.get(name).map(|m| unbound(m)))
            .unwrap_or_default()
    }

    /// An own method's parameters' and result's types.
    pub(super) fn signature_types(&mut self, m: &Method) -> (Vec<Type>, Type) {
        let s = self.signature(m);
        (s.params, s.ret)
    }

    /// Methods `name` of own type `t` or the nearest type it inherits that
    /// declares them (or a component of it), each with its type's bound.
    fn own_methods(&self, t: &Type, name: &str) -> Option<Vec<(Method, Bound)>> {
        let Type::Own(type_name, args) = t else {
            return None;
        };
        self.lineage_of(type_name, args)
            .into_iter()
            .find_map(|(key, args)| {
                let own = self.own.of_key(&key)?;
                match own.methods.get(name) {
                    Some(methods) => {
                        let bound = self.bound_of(&key.name, &args);
                        Some(methods.iter().map(|m| (m.clone(), bound.clone())).collect())
                    }
                    None => own
                        .components
                        .iter()
                        .find_map(|c| self.own_methods(&Type::own(c.clone()), name)),
                }
            })
    }

    /// A name whose value is a delegate (a local, a member, an event): its signature.
    fn delegate(&mut self, name: &str) -> Option<Signature> {
        let t = match self.local(name) {
            Some(t) => t.clone(),
            None => self.this.clone().map(|this| self.own_member(&this, name))?,
        };
        self.delegate_sig(&t)
            .map(|(params, ret)| signature_of(&params, &ret))
    }

    /// Method `name` a `using static` brings in (`using static Yog.Chat;`
    /// → `Broadcast(…)`).
    fn static_import(&mut self, name: &str, at: Span) -> Callee {
        let statics = self.directives.statics.clone();
        for path in statics {
            let names: Vec<crate::ast::Name> = path
                .iter()
                .map(|text| crate::ast::Name {
                    text: text.clone(),
                    span: at,
                })
                .collect();
            if let Type::Yog(group) = self.qualified(&names, at)
                && super::surface::SURFACE
                    .groups
                    .get(&group)
                    .is_some_and(|g| g.methods.contains_key(name))
            {
                return self.yog_methods(&group, name, at);
            }
        }
        Callee::Unknown
    }

    /// Group `group`'s methods `name`: each one's parameters and answer.
    fn yog_methods(&mut self, group: &str, name: &str, at: Span) -> Callee {
        let surface = &*super::surface::SURFACE;
        let Some(g) = surface.groups.get(group) else {
            return Callee::Unknown;
        };
        let Some(methods) = g.methods.get(name) else {
            self.unknown_member(&Type::Yog(group.to_string()), name, at);
            return Callee::Unknown;
        };
        if !self.to_server
            && methods
                .iter()
                .any(|m| m.address == yog_ids::service_net::SEND_TO_SERVER)
        {
            self.report.error(
                script_problem::UNDECLARED,
                at,
                "this mod sends to the server without saying so: `[network]` `to_server = true` in yog.toml (it then needs Yog on the server too)",
            );
        }
        Callee::Typed(
            methods
                .iter()
                .map(|m| Signature {
                    params: m
                        .params
                        .iter()
                        .map(|p| Type::of_shape(&(p.shape)()))
                        .collect(),
                    passes: vec![None; m.params.len()],
                    required: m.params.iter().take_while(|p| p.required).count(),
                    ret: Type::of_shape(&m.reply),
                    generics: Vec::new(),
                })
                .collect(),
        )
    }

    fn game_methods(&mut self, class: &str, name: &str, at: Span) -> Callee {
        let Some(lookup) = &self.lookup else {
            return Callee::Unknown;
        };
        let Some(c) = lookup.class(class) else {
            return Callee::Unknown;
        };
        match game::member(lookup, c, name, true) {
            Found::Methods(sigs) => Callee::Typed(
                sigs.into_iter()
                    .map(|(params, ret)| Signature {
                        passes: vec![None; params.len()],
                        required: params.len(),
                        params,
                        ret,
                        generics: Vec::new(),
                    })
                    .collect(),
            ),
            Found::Nothing => {
                self.unknown_member(&Type::Game(class.to_string()), name, at);
                Callee::Unknown
            }
            _ => Callee::Unknown,
        }
    }

    /// An own method's signature: parameters, how many are required (the
    /// rest have defaults), result.
    fn signature(&mut self, m: &Method) -> Signature {
        let bound = super::generics::params_of(&m.generics);
        let generics = bound.keys().cloned().collect();
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
    fn pick(&mut self, sigs: &[Signature], given: &[Type], args: &[Arg], at: Span) -> Type {
        if sigs.is_empty() || args.iter().any(|a| a.name.is_some()) {
            return Type::Unknown;
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
        if let Some((s, found)) = sigs.iter().find_map(|s| fits(s).map(|f| (s, f))) {
            self.check_passing(&s.passes, args, given);
            return s.ret.substitute(&found);
        }
        let taking: Vec<&Signature> = sigs.iter().filter(|s| takes(s)).collect();
        match taking.as_slice() {
            [] => {
                self.report.error(
                    script_problem::WRONG_ARGS,
                    at,
                    format!("no overload takes {} arguments", given.len()),
                );
                Type::Unknown
            }
            [one] => {
                for ((g, p), a) in given.iter().zip(&one.params).zip(args) {
                    self.want(g, p, a.value.span);
                }
                one.ret.clone()
            }
            _ => Type::Unknown,
        }
    }
}

/// `x.Client` / `x.Server`: a call on the other side (`ys-grammar.md`
/// §3.6), which every object has.
fn is_side_call(name: &str) -> bool {
    [&crate::words::client::WORD, &crate::words::server::WORD]
        .iter()
        .any(|w| yog_codegen::pascal(w.text) == name)
}

/// What a delegate's call is matched against: every parameter required.
fn signature_of(params: &[Type], ret: &Type) -> Signature {
    Signature {
        params: params.to_vec(),
        passes: vec![None; params.len()],
        required: params.len(),
        ret: ret.clone(),
        generics: Vec::new(),
    }
}

/// An own type's type arguments (none for any other type).
fn own_args(t: &Type) -> &[Type] {
    match t {
        Type::Own(_, args) => args,
        _ => &[],
    }
}

/// Methods that bind no type parameters (the file's, an extension's).
fn unbound(methods: &[Method]) -> Vec<(Method, Bound)> {
    methods.iter().map(|m| (m.clone(), Bound::new())).collect()
}
