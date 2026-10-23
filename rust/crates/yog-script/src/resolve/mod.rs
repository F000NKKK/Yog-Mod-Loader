//! What the names of a source stand for, and what's out of place: each
//! name looked up in the scopes around it (a declaration's anywhere in its
//! body, a block's from where it's bound), declared twice, assigned while
//! constant, `break` outside a loop, `skip` outside a `before`, `it`
//! outside a lambda, `self` outside members. Names a source doesn't
//! declare are the game's and the library's: the type checker looks them up.

mod decls;
mod exprs;
mod patches;
mod scope;
mod stmts;

pub use scope::{Def, DefKind, Symbols};

use crate::ast;
use crate::report::Report;
use scope::Scope;

/// Where the code being walked is, as far as what it may say goes.
#[derive(Clone, Copy, Default)]
struct Context {
    /// Loops around it (in its function or lambda).
    loops: u32,
    /// A lambda without parameters is around it: `it` is its argument.
    it: bool,
    /// In a patch's `before`: `skip` ends the method.
    before: bool,
    /// In a type's, a content's or a patch's members: `self` is the object.
    member: bool,
}

pub(crate) struct Resolver<'r, 's> {
    report: &'r mut Report<'s>,
    scopes: Vec<Scope>,
    symbols: Symbols,
    cx: Context,
}

/// `file`'s names resolved, what's out of place reported.
pub(crate) fn resolve(file: &ast::File, report: &mut Report<'_>) -> Symbols {
    let mut r = Resolver {
        report,
        scopes: vec![Scope::default()],
        symbols: Symbols::default(),
        cx: Context::default(),
    };
    r.decls(&file.decls);
    r.symbols
}

impl Resolver<'_, '_> {
    /// Runs `f` in context `cx`, the current one back after.
    fn within<R>(&mut self, cx: Context, f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.cx, cx);
        let out = f(self);
        self.cx = outer;
        out
    }

    /// A function's or a handler's body context: no loops, no lambda,
    /// members as around it.
    fn body_cx(&self) -> Context {
        Context {
            member: self.cx.member,
            ..Context::default()
        }
    }
}
