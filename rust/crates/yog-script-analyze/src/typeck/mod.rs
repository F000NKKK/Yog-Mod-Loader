//! The type checker: each expression's type found, and what doesn't fit
//! reported — a value of one type where another is wanted, `null` where it
//! can't be, a member a type doesn't have, a call that doesn't match, a
//! game class of the other side. The game's types come from the class
//! index of a targeted version (C#'s names over Java's); what neither the
//! source nor the index knows (the library's) is not checked yet.

mod decls;
mod exprs;
mod game;
mod lambdas;
mod members;
mod model;
mod names;
mod own;
mod shapes;
mod stmts;
mod surface;
mod written;

use std::collections::HashMap;

pub use model::Type;

use crate::ast::File;
use crate::index::{ClassIndex, Lookup, Side};
use own::OwnTypes;
use yog_analysis::Report;

pub(crate) struct Checker<'r, 's, 'i> {
    report: &'r mut Report<'s>,
    lookup: Option<Lookup<'i>>,
    own: OwnTypes,
    /// Locals and parameters, innermost scope last.
    scopes: Vec<HashMap<String, Type>>,
    /// The type whose members the code is in (`this`).
    this: Option<Type>,
    /// The side the code runs on.
    side: Side,
    /// What a `return` in the code must give (`None`: not in a method).
    returns: Option<Type>,
    /// The file's `using` directives.
    directives: names::Directives,
    /// The namespace the code is in.
    ns: Vec<String>,
    /// A `.yns`: the game's classes reachable.
    nanites: bool,
}

/// `file`'s types checked against `index` (a targeted version's), what
/// doesn't fit reported.
pub(crate) fn check(
    file: &File,
    nanites: bool,
    index: Option<&ClassIndex>,
    report: &mut Report<'_>,
) {
    let mut own = OwnTypes::collect(&file.decls);
    own.extensions(&file.decls);
    let mut c = Checker {
        report,
        lookup: index.map(ClassIndex::lookup),
        own,
        scopes: vec![HashMap::new()],
        this: None,
        side: Side::Shared,
        returns: None,
        directives: names::Directives::collect(&file.decls),
        ns: Vec::new(),
        nanites,
    };
    c.decls(&file.decls);
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
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), t);
        }
    }

    fn local(&self, name: &str) -> Option<&Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
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
