//! What the names of a source stand for, and what's out of place: each
//! name looked up in the scopes around it (a declaration's anywhere in its
//! body, a block's from where it's bound), declared twice, assigned while
//! constant, cased against C#'s style, `break` outside a loop, `skip`
//! outside a `before`, `it` outside a block lambda, `this` outside members,
//! `await` outside `async`. Names a source doesn't declare are the game's
//! and the library's: the type checker looks them up.

mod decls;
mod exprs;
mod naming;
mod patches;
mod patterns;
mod scope;
mod stmts;

pub use scope::{Def, DefKind, Symbols};

/// The name a lifetime is kept by among the scope's: `'a`, apart from an
/// `a` there.
fn lifetime_key(name: &str) -> String {
    format!("{}{name}", crate::source::token::QUOTE)
}

use crate::ast;
use scope::Scope;
use yog_analysis::Report;

/// Where the code being walked is, as far as what it may say goes.
#[derive(Clone, Copy, Default)]
struct Context {
    /// Loops around it (in its method or lambda).
    loops: u32,
    /// `switch` statements around it: `break` leaves one.
    switches: u32,
    /// A block lambda is around it: `it` is its argument.
    it: bool,
    /// In a patch's `before`: `skip` ends the method.
    before: bool,
    /// In a type's, a content's, a component's or a patch's members: `this`
    /// is the object.
    member: bool,
    /// In an `async` method (or a test): `await` may wait.
    awaits: bool,
    /// In a constructor's body: its type's `readonly` fields may be set.
    constructing: bool,
    /// In a `lock`'s statement: no `await` (a lock isn't held across a wait).
    locked: bool,
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

    /// A method's or a handler's body context: no loops, no lambda,
    /// members as around it, awaiting when `awaits`.
    fn body_cx(&self, awaits: bool) -> Context {
        Context {
            member: self.cx.member,
            awaits,
            ..Context::default()
        }
    }
}
