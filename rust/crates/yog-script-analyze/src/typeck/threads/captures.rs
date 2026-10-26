//! What a lambda captures, recorded as its body is checked: a local of the
//! code around it, `this`, a static it reaches — what goes with it where
//! it's handed to another thread (`mod.rs`).

use super::super::model::Type;
use super::super::own::TypeKey;
use super::super::{Checker, Types};
use crate::ast::Expr;
use crate::source::token::Span;

/// What a lambda reaches outside itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    /// The local's name, `this`, or the static's (`Cache.Hits`).
    pub name: String,
    pub ty: Type,
    /// Where it's first read.
    pub at: Span,
    pub kind: Captured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    /// A local of the code around it: handed over with it.
    Local,
    /// `this` (or a member of it by its name): its owner keeps reaching it.
    This,
    /// A static field: every thread reaches it.
    Static,
}

impl Checker<'_, '_, '_> {
    /// `f`, checking lambda `e`'s body: what it reaches outside recorded.
    pub(in super::super) fn in_lambda<R>(&mut self, e: &Expr, f: impl FnOnce(&mut Self) -> R) -> R {
        self.lambdas.push((Types::key_of(e), self.scopes.len()));
        let out = f(self);
        self.lambdas.pop();
        out
    }

    /// Local `name` (of type `ty`, bound in scope `depth`) read at `at`:
    /// captured by each lambda being checked it's outside of.
    pub(in super::super) fn local_read(&mut self, name: &str, ty: &Type, depth: usize, at: Span) {
        self.captured(depth, || Capture {
            name: name.to_string(),
            ty: ty.clone(),
            at,
            kind: Captured::Local,
        });
    }

    /// `this` read at `at`: captured by each lambda being checked.
    pub(in super::super) fn this_read(&mut self, at: Span) {
        let Some(this) = self.this.clone() else {
            return;
        };
        self.captured(0, || Capture {
            name: crate::words::this::WORD.text.to_string(),
            ty: this.clone(),
            at,
            kind: Captured::This,
        });
    }

    fn static_read(&mut self, owner: &str, name: &str, ty: &Type, at: Span) {
        self.captured(0, || Capture {
            name: format!("{owner}.{name}"),
            ty: ty.clone(),
            at,
            kind: Captured::Static,
        });
    }

    /// Name `name` (of type `ty`, not a local) read at `at`: a member of
    /// `this` (`this` captured), or a static field of its type.
    pub(in super::super) fn member_read(&mut self, name: &str, ty: &Type, at: Span) {
        if self.lambdas.is_empty() || !ty.known() {
            return;
        }
        let Some(Type::Own(owner, args)) = self.this.clone() else {
            return;
        };
        let Some(own) = self.own.of_key(&TypeKey::new(&owner, args.len())) else {
            return;
        };
        if own.statics.iter().any(|s| s == name) {
            self.static_field_read(&owner, name, ty, at);
        } else if own.values.contains_key(name) {
            self.this_read(at);
        }
    }

    /// Method `name` of `this`'s type `owner` called by its name alone at
    /// `at`: `this` captured, unless it's static.
    pub(in super::super) fn method_read(&mut self, owner: &str, name: &str, at: Span) {
        let instance = self
            .own
            .named(owner)
            .is_some_and(|(_, o)| !o.static_methods.iter().any(|s| s == name));
        if instance {
            self.this_read(at);
        }
    }

    /// Static field `owner.name` (of type `ty`) read at `at`: reached by
    /// each lambda being checked, unless each thread has its own.
    pub(in super::super) fn static_field_read(
        &mut self,
        owner: &str,
        name: &str,
        ty: &Type,
        at: Span,
    ) {
        let own = self.own.named(owner).map(|(_, o)| o);
        let field = own.filter(|o| o.statics.iter().any(|s| s == name));
        if self.lambdas.is_empty()
            || field.is_none_or(|o| o.thread_statics.iter().any(|s| s == name))
        {
            return;
        }
        self.static_read(owner, name, ty, at);
    }

    /// `made` noted as captured by each lambda being checked whose scopes
    /// start past `depth` (all of them: `0`).
    fn captured(&mut self, depth: usize, made: impl Fn() -> Capture) {
        for &(lambda, floor) in &self.lambdas {
            if depth < floor {
                self.types.capture(lambda, made());
            }
        }
    }
}
