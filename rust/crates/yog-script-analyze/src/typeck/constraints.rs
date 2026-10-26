//! `where` constraints (C#'s): in a generic's code a type parameter has
//! its bounds' members (`where T : IScore` gives `t.Score()`) and `new T()`
//! with `new()`; where it's given a type, the type must meet them (CS0311,
//! CS0452, CS0453, CS0310).

use std::collections::HashMap;

use yog_ids::script_problem;

use super::Checker;
use super::context::Context;
use super::generics::Bound;
use super::model::Type;
use crate::ast::{self, Constraint, Method, Where};
use crate::source::token::Span;

/// A type parameter's constraints, as written.
#[derive(Debug, Clone, Default)]
pub(super) struct Bounds {
    /// Its base class and interfaces.
    pub types: Vec<ast::Type>,
    /// `class`: a reference; `struct`: a value; `new()`: made with nothing.
    pub class: bool,
    pub value: bool,
    pub new: bool,
}

/// Each type parameter's constraints, by its name.
pub(super) type Constraints = HashMap<String, Bounds>;

/// `wheres`' constraints by the type parameter each is of.
pub(super) fn constraints_of(wheres: &[Where]) -> Constraints {
    let mut out = Constraints::new();
    for w in wheres {
        let b: &mut Bounds = out.entry(w.name.text.clone()).or_default();
        for c in &w.bounds {
            match c {
                Constraint::Class => b.class = true,
                Constraint::Struct => b.value = true,
                Constraint::New => b.new = true,
                Constraint::Type(t) => b.types.push(t.clone()),
            }
        }
    }
    out
}

impl Checker<'_, '_, '_> {
    /// Runs `f` with `wheres`' constraints on the type parameters in scope.
    pub(super) fn with_constraints<R>(
        &mut self,
        wheres: &[Where],
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.constraints.push(constraints_of(wheres));
        let out = f(self);
        self.constraints.pop();
        out
    }

    /// Type parameter `p`'s constraints in scope, if it has any.
    fn bounds_of(&self, p: &str) -> Option<Bounds> {
        self.constraints
            .iter()
            .rev()
            .find_map(|c| c.get(p))
            .cloned()
    }

    /// The types type parameter `p` is bounded by, typed.
    pub(super) fn bound_types(&mut self, p: &str) -> Vec<Type> {
        let written = self.bounds_of(p).map(|b| b.types).unwrap_or_default();
        written.iter().map(|t| self.type_of(t)).collect()
    }

    /// Member `name` of a `p` (a type parameter): its bounds'.
    pub(super) fn param_member(&mut self, p: &str, name: &str) -> Option<(Type, Type)> {
        self.bound_types(p).into_iter().find_map(|b| {
            let t = self.own_member(&b, name);
            t.known().then_some((b, t))
        })
    }

    /// Methods `name` of a `p` (a type parameter): its bounds'.
    pub(super) fn param_methods(
        &mut self,
        p: &str,
        name: &str,
    ) -> Option<(Type, Vec<(Method, Context)>)> {
        self.bound_types(p)
            .into_iter()
            .find_map(|b| self.own_methods(&b, name).map(|m| (b, m)))
    }

    /// `new p(args)` (`p` a type parameter): only with `new()`, and nothing.
    pub(super) fn param_made(&mut self, p: &str, args: usize, at: Span) {
        if args > 0 || !self.bounds_of(p).is_some_and(|b| b.new || b.value) {
            self.report.error(
                script_problem::CONSTRAINT,
                at,
                format!("`new {p}()` needs `where {p} : new()`, and takes no arguments"),
            );
        }
    }

    /// Written type `t` (at `at`): its arguments meeting its type's constraints.
    pub(super) fn type_meets(&mut self, t: &Type, at: Span) {
        let Type::Own(name, args) = t else {
            return;
        };
        let Some(own) = self
            .own
            .get(name, args.len())
            .filter(|o| !o.wheres.is_empty())
        else {
            return;
        };
        let (generics, wheres) = (own.generics.clone(), own.wheres.clone());
        self.meets((&generics, &wheres), args, at);
    }

    /// The type arguments call `used` gives its method meeting its constraints.
    pub(super) fn call_meets(&mut self, used: &super::types::MethodUse, at: Span) {
        if used.type_args.is_empty() {
            return;
        }
        let methods = match &used.owner {
            Some(o) => self
                .own
                .of_key(&super::own::TypeKey::new(o, used.owner_arity))
                .and_then(|t| t.methods.get(&used.name)),
            None => self.own.methods.get(&used.name),
        };
        let Some(m) = methods
            .and_then(|m| m.get(used.overload))
            .filter(|m| !m.constraints.is_empty())
        else {
            return;
        };
        let generics: Vec<String> = m
            .generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| g.name.text.clone())
            .collect();
        let wheres = m.constraints.clone();
        self.meets((&generics, &wheres), &used.type_args.clone(), at);
    }

    /// `args` given for `generics` constrained by `wheres` (at `at`): each
    /// meeting its constraints, read with `args` bound.
    fn meets(&mut self, (generics, wheres): (&[String], &[Where]), args: &[Type], at: Span) {
        if wheres.is_empty() || self.meeting {
            return;
        }
        self.meeting = true;
        self.meets_each((generics, wheres), args, at);
        self.meeting = false;
    }

    fn meets_each(&mut self, (generics, wheres): (&[String], &[Where]), args: &[Type], at: Span) {
        let bound: Bound = generics.iter().cloned().zip(args.iter().cloned()).collect();
        for (p, b) in constraints_of(wheres) {
            let Some(arg) = generics
                .iter()
                .position(|g| *g == p)
                .and_then(|i| args.get(i))
            else {
                continue;
            };
            if !arg.known() || matches!(arg, Type::Param(_)) {
                continue;
            }
            let wanted: Vec<Type> = self.with_types(bound.clone(), |c| {
                b.types.iter().map(|t| c.type_of(t)).collect()
            });
            if let Some(why) = self.unmet(arg, &b, &wanted) {
                self.report.error(
                    script_problem::CONSTRAINT,
                    at,
                    format!("`{arg}` can't be `{p}`: {why}"),
                );
            }
        }
    }

    /// Why `arg` doesn't meet `b` (its types `wanted`), if it doesn't.
    fn unmet(&self, arg: &Type, b: &Bounds, wanted: &[Type]) -> Option<String> {
        let reference = self.is_reference(arg.non_null()) || matches!(arg.non_null(), Type::Fn(..));
        if b.class && !reference {
            return Some("it must be a class (`where … : class`)".into());
        }
        if b.value && reference {
            return Some("it must be a value (`where … : struct`)".into());
        }
        if b.new && !self.makes_empty(arg) {
            return Some("it must be made with no arguments (`where … : new()`)".into());
        }
        // A built-in value is its `[Host]` type, the interfaces it declares.
        let host = self.hosted(arg.non_null());
        let missing = wanted.iter().find(|w| {
            w.known()
                && !self.converts(arg, w)
                && !host.as_ref().is_some_and(|h| self.converts(h, w))
        })?;
        Some(format!("it isn't a `{missing}`"))
    }

    /// Whether `new t()` is: a value, or a class with a constructor taking nothing.
    fn makes_empty(&self, t: &Type) -> bool {
        let Type::Own(name, args) = t else {
            return !self.is_reference(t);
        };
        let Some(own) = self.own.get(name, args.len()) else {
            return true;
        };
        if !own.heap {
            return true;
        }
        !own.is_abstract
            && !own.interface
            && (own.ctors.iter().any(Vec::is_empty) || own.ctors.is_empty() && own.ctor.is_empty())
    }
}

impl Checker<'_, '_, '_> {
    /// Member `name` of a `t` (a type parameter) its bounds don't have:
    /// `object`'s, else not one.
    pub(super) fn object_member(&mut self, t: &Type, name: &str, at: Span) -> Type {
        let object = Type::own(yog_script_ids::clr::OBJECT);
        let found = self.own_member(&object, name);
        if !found.known() && !self.own_has_method(&object, name) {
            self.unknown_member(t, name, at);
        }
        found
    }

    /// Methods `name` of a `t` (a type parameter) its bounds don't have:
    /// `object`'s, else extension methods of the name.
    pub(super) fn object_methods(&mut self, t: &Type, name: &ast::Name) -> super::members::Callee {
        match self.own_methods(&Type::own(yog_script_ids::clr::OBJECT), &name.text) {
            Some(methods) => super::members::Callee::Own(methods),
            None => self.extension_or_unknown(t, name),
        }
    }
}
