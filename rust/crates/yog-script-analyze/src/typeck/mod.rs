//! The type checker: each expression's type found, and what doesn't fit
//! reported — a value of one type where another is wanted, `null` where it
//! can't be, a member a type doesn't have, a call that doesn't match, a
//! game class of the other side. The game's types come from the class
//! index of a targeted version (C#'s names over Java's); what neither the
//! source nor the index knows (the library's) is not checked yet.

mod arrays;
mod attributes;
mod awaits;
mod bits;
mod builtin;
mod callees;
mod calls;
mod collections;
mod content;
mod context;
mod copy;
mod ctors;
mod decls;
mod delegates;
mod exprs;
mod game;
mod generics;
mod groups;
mod indexers;
mod inherit;
mod lambdas;
mod lineage;
mod macro_calls;
mod members;
mod model;
mod mutation;
mod names;
mod operators;
mod overloads;
mod own;
mod passing;
mod pick;
mod shapes;
mod stmts;
pub(crate) mod surface;
mod tuples;
mod types;
mod variance;
mod variants;
mod written;

use std::collections::HashMap;

pub use model::Type;
pub use types::{MethodUse, OperatorUse, Quoted, Types};

use crate::ast::File;
use crate::index::{ClassIndex, Lookup, Side};
use own::OwnTypes;
use yog_analysis::Report;

/// A local or a parameter: its type, and whether it's read-only.
struct Slot {
    ty: Type,
    fixed: bool,
}

pub(crate) struct Checker<'r, 's, 'i> {
    report: &'r mut Report<'s>,
    lookup: Option<Lookup<'i>>,
    own: OwnTypes,
    /// Locals and parameters, innermost scope last.
    scopes: Vec<HashMap<String, Slot>>,
    /// The code is a `readonly` member's: `this` is lent read-only.
    /// The members being checked are a content declaration's: its
    /// properties its kind's (`content.rs`).
    in_content: bool,
    readonly_this: bool,
    /// The type whose members the code is in (`this`).
    this: Option<Type>,
    /// The side the code runs on.
    side: Side,
    /// What a `return` in the code must give (`None`: not in a method).
    returns: Option<Type>,
    /// While a lambda's result is inferred: what its `return`s give.
    inferred: Option<Vec<Type>>,
    /// The file's `using` directives.
    directives: names::Directives,
    /// The namespace the code is in.
    ns: Vec<String>,
    /// A `.yns`: the game's classes reachable.
    nanites: bool,
    /// Its mod says it sends to the server.
    to_server: bool,
    /// The type parameters in scope, innermost last.
    type_params: Vec<generics::Bound>,
    /// Each expression's type found.
    types: Types,
    /// Each own type's bases, typed.
    bases: lineage::Bases,
    /// Each declared delegate's signature, typed.
    delegates: delegates::Delegates,
    /// Each type's operators, typed.
    operators: operators::Operators,
}

/// `file`'s types checked against `index` (a targeted version's), what
/// doesn't fit reported.
/// What a source may reach: the game's classes (a `.yns`), sending to the
/// server (its mod says so).
pub(crate) struct Allowed {
    pub nanites: bool,
    pub to_server: bool,
}

/// The built-in type its keyword names (`i32`, `string`): the one table
/// of them, for whatever lowers types too.
pub fn builtin_type(word: &str) -> Option<Type> {
    builtin::by_word(word)
}

/// The built-in type `System`'s type `name` is (`Int32` is `i32`, `Void`
/// `void`): the same table read the other way.
pub fn system_type(name: &str) -> Option<Type> {
    builtin::by_system_name(name)
}

/// A built-in type's type in `System` (`string`'s `String`), which a
/// `[Host]` declaration gives its members.
pub fn system_name(t: &Type) -> Option<&'static str> {
    builtin::system_name(t)
}

pub(crate) fn check(
    file: &File,
    allowed: Allowed,
    index: Option<&ClassIndex>,
    report: &mut Report<'_>,
) -> Types {
    let mut own = OwnTypes::collect(&[&crate::system::DECLS, &file.decls]);
    own.extensions(&file.decls);
    let mut c = Checker {
        report,
        lookup: index.map(ClassIndex::lookup),
        own,
        scopes: vec![HashMap::new()],
        this: None,
        side: Side::Shared,
        returns: None,
        inferred: None,
        directives: names::Directives::collect(&file.decls),
        ns: Vec::new(),
        nanites: allowed.nanites,
        to_server: allowed.to_server,
        type_params: Vec::new(),
        readonly_this: false,
        in_content: false,
        types: Types::default(),
        bases: lineage::Bases::new(),
        delegates: delegates::Delegates::new(),
        operators: operators::Operators::new(),
    };
    c.type_bases();
    c.type_delegates();
    c.type_operators();
    c.decls(&file.decls);
    inherit::check(&crate::system::DECLS, &file.decls, c.report);
    c.types
}

impl Checker<'_, '_, '_> {
    /// Runs `f` in a scope of its own.
    fn scoped<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.scopes.push(HashMap::new());
        let out = f(self);
        self.scopes.pop();
        out
    }

    fn bind(&mut self, name: &str, t: Type) {
        self.bind_as(name, t, false);
    }

    /// `name` bound to a `t`; `fixed`: what it holds is read-only here (a
    /// `let`, an `in` parameter, a `foreach` item, a pattern's name).
    fn bind_as(&mut self, name: &str, ty: Type, fixed: bool) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), Slot { ty, fixed });
        }
    }

    fn local(&self, name: &str) -> Option<&Type> {
        self.slot(name).map(|s| &s.ty)
    }

    fn slot(&self, name: &str) -> Option<&Slot> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Whether a `got` goes where `wanted` is: as it is, or through an
    /// implicit conversion one of them declares.
    fn converts(&self, got: &Type, wanted: &Type) -> bool {
        self.converts_plainly(got, wanted) || self.implicit(got, wanted).is_some()
    }

    /// Whether a `got` goes where `wanted` is as it is: a built-in
    /// conversion, or an own type that is the one wanted (inherits or
    /// implements it).
    fn converts_plainly(&self, got: &Type, wanted: &Type) -> bool {
        if got.converts_to(wanted) {
            return true;
        }
        if got.is_nullable() && !wanted.is_nullable() {
            return false;
        }
        match (got.non_null(), wanted.non_null()) {
            // An own type, or one it inherits, given the arguments wanted.
            (Type::Own(a, xs), Type::Own(b, ys)) => self.own_converts(a, xs, (b, ys), true),
            (Type::Array(..), wanted) => self.array_converts(got.non_null(), wanted),
            (Type::Tuple(got, _), Type::Tuple(wanted, _)) => self.tuple_converts(got, wanted),
            // A lambda or a method where a delegate of its shape is wanted.
            (Type::Fn(..), Type::Own(..)) => {
                match (self.delegate_sig(got), self.delegate_sig(wanted)) {
                    (Some((gp, gr)), Some((wp, wr))) => {
                        gp.len() == wp.len()
                            && wp.iter().zip(&gp).all(|(w, g)| self.converts(w, g))
                            && self.converts(&gr, &wr)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `base`: `this` as the class it inherits.
    fn base(&self) -> Type {
        match &self.this {
            Some(Type::Own(name, _)) => self
                .own
                .base_class(name)
                .map_or(Type::Unknown, |b| Type::own(b.to_string())),
            _ => Type::Unknown,
        }
    }

    /// Runs `f` with the code's `this`, side and expected result set.
    fn within<R>(
        &mut self,
        this: Option<Type>,
        side: Side,
        returns: Option<Type>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved = (
            std::mem::replace(&mut self.this, this),
            self.side,
            std::mem::replace(&mut self.returns, returns),
        );
        self.side = side;
        let out = self.scoped(f);
        (self.this, self.side, self.returns) = saved;
        out
    }
}
