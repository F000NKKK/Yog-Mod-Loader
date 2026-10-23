//! Declarations: their names are known in all of the body they're in
//! (declared first, then each walked); members see `this`.

use super::naming::Case;
use super::{Context, DefKind, Resolver};
use crate::ast::{
    Constraint, Decl, DeclKind, Generic, Method, MethodBody, Modifier, Param, TypeHeader, Where,
};
use crate::words::{interface, record};

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
            // A `using` brings in a namespace's types, which aren't this
            // source's to know: it declares no name (an alias does).
            DeclKind::Alias { name, .. } => self.define(name, DefKind::Import, Case::Any, again),
            DeclKind::Unit { name, .. } | DeclKind::Enum { name, .. } => {
                self.define(name, DefKind::Type, Case::Pascal, again);
            }
            DeclKind::Type { word, name, .. } => {
                let case = if word.text == interface::WORD.text {
                    Case::Interface
                } else {
                    Case::Pascal
                };
                self.define(name, DefKind::Type, case, again);
            }
            DeclKind::Method(m) => self.define(&m.name, DefKind::Method, Case::Pascal, again),
            DeclKind::Event { name, .. } => self.define(name, DefKind::Field, Case::Pascal, again),
            DeclKind::Field(f) => self.define(&f.name, field_kind(d), Case::Pascal, again),
            DeclKind::Property {
                name, accessors, ..
            } => {
                let settable = accessors.is_empty()
                    || accessors
                        .iter()
                        .any(|a| a.name.text != crate::words::get::WORD.text);
                let kind = if settable {
                    DefKind::Field
                } else {
                    DefKind::ReadOnly
                };
                self.define(name, kind, Case::Pascal, again);
            }
            // A recipe is named by what it makes: it declares no name.
            DeclKind::Content { kind, name, .. } if kind.text == yog_ids::kind::RECIPE => {
                self.refer(&name.text, name.span);
            }
            DeclKind::Content { name, .. } | DeclKind::Screen { name, .. } => {
                self.define(name, DefKind::Content, Case::Pascal, again);
            }
            DeclKind::Component { name, .. } => {
                self.define(name, DefKind::Component, Case::Pascal, again)
            }
            DeclKind::System { name, .. } => {
                self.define(name, DefKind::Method, Case::Pascal, again)
            }
            DeclKind::Native { methods, .. } => {
                for m in methods {
                    self.define(&m.name, DefKind::Method, Case::Pascal, again);
                }
            }
            _ => {}
        }
    }

    fn walk_decl(&mut self, d: &Decl) {
        match &d.kind {
            DeclKind::Mod { body, .. } => self.members(|r| r.decls(body)),
            DeclKind::Event { ty, .. } => self.ty(ty),
            DeclKind::Alias { ty, .. } => self.ty(ty),
            DeclKind::Method(m) => self.method(m, d.has(Modifier::Async)),
            DeclKind::Field(f) => {
                self.ty(&f.ty);
                self.opt_expr(f.bound.as_ref());
                self.opt_expr(f.value.as_ref());
                if let Some((_, ty)) = &f.was {
                    self.opt_ty(ty.as_ref());
                }
            }
            DeclKind::Property {
                ty,
                accessors,
                value,
                ..
            } => {
                self.ty(ty);
                for a in accessors {
                    self.body(a.body.as_ref(), false);
                }
                self.opt_expr(value.as_ref());
            }
            DeclKind::Ctor(c) => self.scoped(|r| {
                r.params(&c.params);
                c.chain
                    .iter()
                    .flat_map(|(_, args)| args)
                    .for_each(|a| r.expr(&a.value));
                r.block(&c.body);
            }),
            DeclKind::Type {
                word, header, body, ..
            } => {
                let record = word.text == record::WORD.text;
                self.members(|r| {
                    r.header(header, record);
                    r.decls(body);
                });
            }
            DeclKind::Component { header, body, .. } => self.members(|r| {
                r.header(header, false);
                r.decls(body);
            }),
            DeclKind::Enum {
                generics,
                base,
                variants,
                body,
                ..
            } => self.members(|r| {
                r.generics(generics);
                r.opt_ty(base.as_ref());
                for v in variants {
                    r.define(&v.name, DefKind::Case, Case::Pascal, false);
                    v.fields.iter().for_each(|f| r.ty(&f.ty));
                    r.opt_expr(v.value.as_ref());
                }
                r.decls(body);
            }),
            DeclKind::Unit { value, .. } => self.opt_expr(value.as_ref()),
            _ => self.walk_rest(d),
        }
    }

    /// The declarations that hold code but few names of their own.
    fn walk_rest(&mut self, d: &Decl) {
        match &d.kind {
            DeclKind::Content { header, body, .. } | DeclKind::Screen { header, body, .. } => {
                self.members(|r| {
                    r.header(header, false);
                    r.decls(body);
                });
            }
            DeclKind::Extend { ty, header, body } => {
                self.ty(ty);
                self.members(|r| {
                    r.header(header, false);
                    r.decls(body);
                });
            }
            DeclKind::System {
                params,
                every,
                body,
                ..
            } => {
                let cx = self.body_cx(false);
                self.within(cx, |r| {
                    r.scoped(|r| {
                        r.params(params);
                        r.opt_expr(every.as_ref());
                        r.block(body);
                    });
                });
            }
            DeclKind::Namespace {
                body: Some(body), ..
            } => self.scoped(|r| r.decls(body)),
            DeclKind::Prop(p) => self.expr(&p.value),
            DeclKind::Command { body, .. } => self.scoped(|r| r.decls(body)),
            DeclKind::Test { body, .. } => self.handler(&[], true, |r| r.block(body)),
            DeclKind::Native { methods, .. } => methods.iter().for_each(|m| self.method(m, false)),
            DeclKind::Patch(p) => self.patch(p),
            _ => {}
        }
    }

    /// A type's header: its generics, constructor parameters (a record's
    /// are its properties), bases, constraints and components.
    fn header(&mut self, h: &TypeHeader, record: bool) {
        self.generics(&h.generics);
        for p in h.ctor.iter().flatten() {
            self.ty(&p.ty);
            self.opt_expr(p.default.as_ref());
            let (kind, case) = if record {
                (DefKind::ReadOnly, Case::Pascal)
            } else {
                (DefKind::Param, Case::Camel)
            };
            self.define(&p.name, kind, case, false);
        }
        h.bases.iter().for_each(|b| self.ty(b));
        self.constraints(&h.constraints);
        h.has.iter().for_each(|c| self.expr(c));
    }

    /// A method: its generics, parameters, constraints, guard and body in a scope.
    pub(super) fn method(&mut self, m: &Method, awaits: bool) {
        let cx = self.body_cx(awaits);
        self.within(cx, |r| {
            r.scoped(|r| {
                r.generics(&m.generics);
                r.params(&m.params);
                r.opt_ty(m.ret.as_ref());
                r.constraints(&m.constraints);
                r.opt_expr(m.requires.as_ref());
                r.body(m.body.as_ref(), awaits);
            });
        });
    }

    /// A method's or an accessor's body.
    fn body(&mut self, body: Option<&MethodBody>, awaits: bool) {
        let cx = Context { awaits, ..self.cx };
        self.within(cx, |r| match body {
            Some(MethodBody::Block(b)) => r.block(b),
            Some(MethodBody::Expr(e)) => r.expr(e),
            None => {}
        });
    }

    /// An event's or a test's body: its parameters, then `body`.
    fn handler(&mut self, params: &[Param], awaits: bool, body: impl FnOnce(&mut Self)) {
        let cx = self.body_cx(awaits);
        self.within(cx, |r| {
            r.scoped(|r| {
                r.params(params);
                body(r);
            });
        });
    }

    /// A body of members: `this` is the object.
    pub(super) fn members(&mut self, f: impl FnOnce(&mut Self)) {
        let cx = Context {
            member: true,
            ..Context::default()
        };
        self.within(cx, |r| r.scoped(f));
    }

    pub(super) fn params(&mut self, params: &[Param]) {
        for p in params {
            self.ty(&p.ty);
            self.opt_expr(p.default.as_ref());
            self.define(&p.name, DefKind::Param, Case::Camel, false);
        }
    }

    fn generics(&mut self, generics: &[Generic]) {
        for g in generics {
            self.define(&g.name, DefKind::Type, Case::TypeParam, false);
        }
    }

    fn constraints(&mut self, constraints: &[Where]) {
        for b in constraints.iter().flat_map(|w| &w.bounds) {
            if let Constraint::Type(t) = b {
                self.ty(t);
            }
        }
    }
}

/// What a field is by its modifiers.
fn field_kind(d: &Decl) -> DefKind {
    if d.has(Modifier::Const) {
        DefKind::Constant
    } else if d.has(Modifier::Readonly) {
        DefKind::ReadOnly
    } else {
        DefKind::Field
    }
}
