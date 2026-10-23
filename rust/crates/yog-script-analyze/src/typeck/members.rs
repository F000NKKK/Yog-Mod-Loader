//! Members and calls: a member of the source's own types (their
//! components' too) or of a game class (by C#'s names); a call matched to
//! the method's parameters.

use yog_ids::script_problem;

use super::Checker;
use super::game::{self, Found};
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind, LambdaBody, Method};
use crate::source::token::Span;

/// What a call's callee stands for.
enum Callee {
    /// The source's own methods.
    Own(Vec<Method>),
    /// A game class's methods: each one's parameters and result.
    Game(Vec<(Vec<Type>, Type)>),
    Unknown,
}

/// What a call is matched against.
struct Signature {
    params: Vec<Type>,
    required: usize,
    ret: Type,
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
            Type::Own(_) => {
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
            _ => Type::Unknown,
        };
        if safe { t.nullable() } else { t }
    }

    /// The type of member `name` of the source's own type `t` (or of a
    /// component it has, or the component itself).
    pub(super) fn own_member(&mut self, t: &Type, name: &str) -> Type {
        let type_name = match t {
            Type::Own(n) => n,
            Type::Game(class) => return self.extension_value(class, name).unwrap_or(Type::Unknown),
            _ => return Type::Unknown,
        };
        let Some(own) = self.own.types.get(type_name) else {
            return Type::Unknown;
        };
        if let Some(written) = own.values.get(name).cloned() {
            return self.type_of(&written);
        }
        if own.cases.iter().any(|c| c == name) {
            return t.clone();
        }
        let components = own.components.clone();
        if components.iter().any(|c| c == name) {
            return Type::Own(name.to_string());
        }
        components
            .iter()
            .map(|c| self.own_member(&Type::Own(c.clone()), name))
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
            return Some(Type::Own(name.to_string()));
        }
        components
            .iter()
            .map(|c| self.own_member(&Type::Own(c.clone()), name))
            .find(Type::known)
    }

    fn extension_methods(&self, class: &str, name: &str) -> Option<Vec<Method>> {
        let ext = self.own.extended.get(crate::index::simple_name(class))?;
        ext.methods.get(name).cloned().or_else(|| {
            ext.components
                .iter()
                .find_map(|c| self.own_methods(&Type::Own(c.clone()), name))
        })
    }

    fn own_has_method(&self, t: &Type, name: &str) -> bool {
        let Type::Own(type_name) = t else {
            return false;
        };
        self.own.types.get(type_name).is_some_and(|o| {
            o.methods.contains_key(name)
                || o.components
                    .iter()
                    .any(|c| self.own_has_method(&Type::Own(c.clone()), name))
        })
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
        match target {
            Callee::Own(methods) => {
                let sigs: Vec<Signature> = methods.iter().map(|m| self.signature(m)).collect();
                self.pick(&sigs, &given, args, callee.span)
            }
            Callee::Game(sigs) => {
                let sigs: Vec<Signature> = sigs
                    .into_iter()
                    .map(|(params, ret)| Signature {
                        required: params.len(),
                        params,
                        ret,
                    })
                    .collect();
                self.pick(&sigs, &given, args, callee.span)
            }
            Callee::Unknown => Type::Unknown,
        }
    }

    /// What `callee` calls.
    fn callee(&mut self, callee: &Expr) -> Callee {
        match &callee.kind {
            ExprKind::Ident(name) if self.local(name).is_none() => {
                let in_this = self.this.clone().and_then(|t| match &t {
                    Type::Game(class) => self.extension_methods(class, name),
                    _ => self.own_methods(&t, name),
                });
                in_this
                    .or_else(|| self.own.methods.get(name).cloned())
                    .map_or(Callee::Unknown, Callee::Own)
            }
            ExprKind::Member { expr, name, .. } => {
                let on = self.expr(expr);
                match on.non_null() {
                    Type::Own(_) => self
                        .own_methods(on.non_null(), &name.text)
                        .map_or(Callee::Unknown, Callee::Own),
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

    fn own_methods(&self, t: &Type, name: &str) -> Option<Vec<Method>> {
        let Type::Own(type_name) = t else { return None };
        let own = self.own.types.get(type_name)?;
        own.methods.get(name).cloned().or_else(|| {
            own.components
                .iter()
                .find_map(|c| self.own_methods(&Type::Own(c.clone()), name))
        })
    }

    fn game_methods(&mut self, class: &str, name: &str, at: Span) -> Callee {
        let Some(lookup) = &self.lookup else {
            return Callee::Unknown;
        };
        let Some(c) = lookup.class(class) else {
            return Callee::Unknown;
        };
        match game::member(lookup, c, name, true) {
            Found::Methods(sigs) => Callee::Game(sigs),
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
        let params = m.params.iter().map(|p| self.type_of(&p.ty)).collect();
        let required = m.params.iter().take_while(|p| p.default.is_none()).count();
        let ret = m.ret.as_ref().map_or(Type::Void, |t| self.type_of(t));
        Signature {
            params,
            required,
            ret,
        }
    }

    /// The signature `given` arguments fit; its result — or a report when
    /// none takes that many, or the one that does gets a wrong type.
    fn pick(&mut self, sigs: &[Signature], given: &[Type], args: &[Arg], at: Span) -> Type {
        if sigs.is_empty() || args.iter().any(|a| a.name.is_some()) {
            return Type::Unknown;
        }
        let takes = |s: &Signature| (s.required..=s.params.len()).contains(&given.len());
        let fits =
            |s: &Signature| takes(s) && given.iter().zip(&s.params).all(|(g, p)| g.converts_to(p));
        if let Some(s) = sigs.iter().find(|s| fits(s)) {
            return s.ret.clone();
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

    /// A lambda's body, its parameters' types not known.
    pub(super) fn lambda_body(&mut self, body: &LambdaBody) {
        match body {
            LambdaBody::Expr(e) => {
                self.expr(e);
            }
            LambdaBody::Block(stmts) => self.stmts(stmts),
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
