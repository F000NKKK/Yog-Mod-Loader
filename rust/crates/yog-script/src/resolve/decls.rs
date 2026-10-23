//! Declarations: their names are known in all of the body they're in
//! (declared first, then each walked), members see `self`.

use super::{Context, DefKind, Resolver};
use crate::ast::{Decl, DeclKind, FnBody, FnDecl, Generic, Name, Param};

impl Resolver<'_, '_> {
    /// A body of declarations: every name first, then each one walked.
    pub(super) fn decls(&mut self, decls: &[Decl]) {
        self.declare_all(decls, false);
        for d in decls {
            self.walk_decl(d);
        }
    }

    /// The names `decls` declare, here; `again`: a version's other branch,
    /// which may declare the same names differently.
    fn declare_all(&mut self, decls: &[Decl], again: bool) {
        for d in decls {
            self.declare(d, again);
        }
    }

    fn declare(&mut self, d: &Decl, again: bool) {
        match &d.kind {
            DeclKind::Use { path, items, alias } => {
                let names = match (alias, items.is_empty()) {
                    (Some(a), _) => std::slice::from_ref(a),
                    (None, false) => items.as_slice(),
                    (None, true) => path.last().map(std::slice::from_ref).unwrap_or_default(),
                };
                for n in names {
                    self.define(n, DefKind::Import, again);
                }
            }
            DeclKind::Fn(f) | DeclKind::Action(f) => self.define(&f.name, DefKind::Fn, again),
            DeclKind::Let { pattern, .. } => self.pattern_names(pattern, again),
            DeclKind::Var(v) => self.define(&v.name, DefKind::Variable, again),
            DeclKind::Const { name, .. } | DeclKind::Val { name, .. } => {
                self.define(name, DefKind::Value, again);
            }
            DeclKind::Unit { name, .. }
            | DeclKind::Type { name, .. }
            | DeclKind::Record { name, .. }
            | DeclKind::Union { name, .. }
            | DeclKind::Enum { name, .. } => self.define(name, DefKind::Type, again),
            DeclKind::Content { name, .. } | DeclKind::Screen { name, .. } => {
                self.define(name, DefKind::Content, again);
            }
            DeclKind::Slots { name, .. } => self.define(name, DefKind::Field, again),
            DeclKind::Version {
                then, otherwise, ..
            } => {
                self.declare_all(then, again);
                if let Some(o) = otherwise {
                    self.declare_all(o, true);
                }
            }
            DeclKind::Native { fns, .. } => {
                for f in fns {
                    self.define(&f.name, DefKind::Fn, again);
                }
            }
            _ => {}
        }
    }

    fn walk_decl(&mut self, d: &Decl) {
        match &d.kind {
            DeclKind::Mod { props, .. } => props.iter().for_each(|p| self.expr(&p.value)),
            DeclKind::Fn(f) | DeclKind::Action(f) => self.function(f),
            DeclKind::Let { ty, value, .. } => {
                self.opt_ty(ty.as_ref());
                self.expr(value);
            }
            DeclKind::Var(v) => {
                self.opt_ty(v.ty.as_ref());
                self.opt_expr(v.value.as_ref());
            }
            DeclKind::Const { ty, value, .. } | DeclKind::Val { ty, value, .. } => {
                self.opt_ty(ty.as_ref());
                self.expr(value);
            }
            DeclKind::Unit { value, .. } => self.opt_expr(value.as_ref()),
            DeclKind::Type { generics, ty, .. } => self.scoped(|r| {
                r.generics(generics);
                r.ty(ty);
            }),
            DeclKind::Record {
                generics,
                fields,
                body,
                ..
            } => self.members(|r| {
                r.generics(generics);
                r.fields(fields);
                r.decls(body);
            }),
            DeclKind::Union {
                generics,
                variants,
                body,
                ..
            } => self.members(|r| {
                r.generics(generics);
                for v in variants {
                    r.define(&v.name, DefKind::Case, false);
                    r.scoped(|r| r.fields(&v.fields));
                }
                r.decls(body);
            }),
            DeclKind::Enum { cases, body, .. } => self.members(|r| {
                for c in cases {
                    r.define(c, DefKind::Case, false);
                }
                r.decls(body);
            }),
            DeclKind::Extend { ty, adds, body } => {
                self.ty(ty);
                self.opt_ty(adds.as_ref());
                self.members(|r| r.decls(body));
            }
            DeclKind::Content { base, body, .. } => {
                self.opt_ty(base.as_ref());
                self.members(|r| r.decls(body));
            }
            DeclKind::Screen { params, body, .. } => self.members(|r| {
                r.params(params);
                r.decls(body);
            }),
            _ => self.walk_rest(d),
        }
    }

    /// The declarations that hold code but no names of their own.
    fn walk_rest(&mut self, d: &Decl) {
        match &d.kind {
            DeclKind::Recipe { result, keys, .. } => {
                self.ty(result);
                keys.iter().for_each(|k| self.expr(&k.value));
            }
            DeclKind::Prop(p) => self.expr(&p.value),
            DeclKind::Slots { size, of, .. } => {
                self.expr(size);
                self.opt_ty(of.as_ref());
            }
            DeclKind::State { body, .. } | DeclKind::Command { body, .. } => {
                self.scoped(|r| r.decls(body));
            }
            DeclKind::On { params, body, .. } => self.handler(params, |r| r.block(body)),
            DeclKind::Test { body, .. } => self.handler(&[], |r| r.block(body)),
            DeclKind::Version {
                then, otherwise, ..
            } => {
                then.iter().for_each(|d| self.walk_decl(d));
                otherwise.iter().flatten().for_each(|d| self.walk_decl(d));
            }
            DeclKind::Native { fns, .. } => fns.iter().for_each(|f| self.function(f)),
            DeclKind::Patch(p) => self.patch(p),
            _ => {}
        }
    }

    /// A function: its generics, parameters, condition and body in a scope.
    pub(super) fn function(&mut self, f: &FnDecl) {
        let cx = self.body_cx();
        self.within(cx, |r| {
            r.scoped(|r| {
                r.generics(&f.generics);
                r.params(&f.params);
                r.opt_ty(f.ret.as_ref());
                r.opt_expr(f.requires.as_ref());
                match &f.body {
                    Some(FnBody::Block(b)) => r.block(b),
                    Some(FnBody::Expr(e)) => r.expr(e),
                    None => {}
                }
            });
        });
    }

    /// An event's or a test's body: its parameters, then `body`.
    fn handler(&mut self, params: &[Name], body: impl FnOnce(&mut Self)) {
        let cx = self.body_cx();
        self.within(cx, |r| {
            r.scoped(|r| {
                for p in params {
                    r.define(p, DefKind::Param, false);
                }
                body(r);
            });
        });
    }

    /// A body of members: `self` is the object.
    pub(super) fn members(&mut self, f: impl FnOnce(&mut Self)) {
        let cx = Context {
            member: true,
            ..Context::default()
        };
        self.within(cx, |r| r.scoped(f));
    }

    pub(super) fn params(&mut self, params: &[Param]) {
        for p in params {
            self.opt_ty(p.ty.as_ref());
            self.opt_expr(p.default.as_ref());
            let kind = if p.var {
                DefKind::Variable
            } else {
                DefKind::Param
            };
            self.define(&p.name, kind, false);
        }
    }

    /// A record's or a variant's fields: assignable when `var`.
    fn fields(&mut self, fields: &[Param]) {
        for f in fields {
            self.opt_ty(f.ty.as_ref());
            self.opt_expr(f.default.as_ref());
            let kind = if f.var {
                DefKind::Field
            } else {
                DefKind::Value
            };
            self.define(&f.name, kind, false);
        }
    }

    fn generics(&mut self, generics: &[Generic]) {
        for g in generics {
            self.opt_ty(g.bound.as_ref());
            self.define(&g.name, DefKind::Type, false);
        }
    }
}
