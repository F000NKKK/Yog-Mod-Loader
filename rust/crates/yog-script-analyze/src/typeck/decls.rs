//! Declarations: fields' values fitting their types, methods' bodies with
//! their side, `this` and result, types' and content's members.

use super::Checker;
use super::model::Type;
use crate::ast::{Decl, DeclKind, Method, MethodBody, Modifier, Param, PatchItem, Point};
use crate::index::Side;

impl Checker<'_, '_, '_> {
    pub(super) fn decls(&mut self, decls: &[Decl]) {
        for d in decls {
            self.decl(d);
        }
    }

    fn decl(&mut self, d: &Decl) {
        let side = side_of(d).unwrap_or(self.side);
        match &d.kind {
            DeclKind::Field(f) => {
                let t = self.type_of(&f.ty);
                if let Some(v) = &f.value {
                    self.within(self.this.clone(), side, None, |c| c.fits(v, &t));
                }
            }
            DeclKind::Property {
                ty,
                accessors,
                value,
                ..
            } => {
                let t = self.type_of(ty);
                for a in accessors {
                    self.body(a.body.as_ref(), &[], Some(t.clone()), side);
                }
                if let Some(v) = value {
                    self.fits(v, &t);
                }
            }
            DeclKind::Method(m) => self.method(m, side),
            DeclKind::Ctor(c) => {
                let this = self.this.clone();
                self.within(this, side, Some(Type::Void), |ch| {
                    ch.params(&c.params);
                    ch.block(&c.body);
                });
            }
            DeclKind::Type { .. }
            | DeclKind::Content { .. }
            | DeclKind::Component { .. }
            | DeclKind::Screen { .. } => self.type_body(d, side),
            DeclKind::Extend { ty, body, .. } => {
                let t = self.type_of(ty);
                self.within(Some(t), side, None, |c| c.decls(body));
            }
            DeclKind::System { .. } | DeclKind::On { .. } | DeclKind::Test { .. } => {
                self.handler(d, side);
            }
            DeclKind::Version {
                then, otherwise, ..
            } => {
                self.decls(then);
                otherwise.iter().for_each(|o| self.decls(o));
            }
            DeclKind::Patch(p) => {
                let target = self.type_of(&p.target);
                self.within(Some(target), side, None, |c| c.patch(&p.items));
            }
            DeclKind::Prop(p) => {
                self.expr(&p.value);
            }
            _ => {}
        }
    }

    /// A type's, a content's, a component's or a screen's members: `this` is it.
    fn type_body(&mut self, d: &Decl, side: Side) {
        match &d.kind {
            DeclKind::Type {
                name, body, header, ..
            }
            | DeclKind::Content {
                name, body, header, ..
            }
            | DeclKind::Component { name, body, header }
            | DeclKind::Screen {
                name, body, header, ..
            } => {
                let own = self
                    .own
                    .types
                    .contains_key(&name.text)
                    .then(|| Type::Own(name.text.clone()));
                self.within(own, side, None, |c| {
                    c.params(header.ctor.as_deref().unwrap_or_default());
                    c.decls(body);
                });
            }
            _ => {}
        }
    }

    /// Code run on an event: a system's, a handler's, a test's.
    fn handler(&mut self, d: &Decl, side: Side) {
        match &d.kind {
            DeclKind::System { params, body, .. } => {
                self.within(None, side, Some(Type::Void), |c| {
                    c.params(params);
                    c.block(body);
                })
            }
            DeclKind::On { params, body, .. } => {
                let this = self.this.clone();
                self.within(this, side, Some(Type::Void), |c| {
                    c.params(params);
                    c.block(body);
                });
            }
            DeclKind::Test { body, .. } => {
                self.within(None, side, Some(Type::Void), |c| c.block(body))
            }
            _ => {}
        }
    }

    fn method(&mut self, m: &Method, side: Side) {
        let ret = m.ret.as_ref().map_or(Type::Void, |t| self.type_of(t));
        let ret = if m.generics.is_empty() {
            ret
        } else {
            Type::Unknown
        };
        self.body(m.body.as_ref(), &m.params, Some(ret), side);
    }

    /// A body: its parameters bound, its result expected.
    fn body(
        &mut self,
        body: Option<&MethodBody>,
        params: &[Param],
        returns: Option<Type>,
        side: Side,
    ) {
        let this = self.this.clone();
        self.within(this, side, returns.clone(), |c| {
            c.params(params);
            match body {
                Some(MethodBody::Block(b)) => c.block(b),
                Some(MethodBody::Expr(e)) => match returns {
                    Some(r) if r != Type::Void => c.fits(e, &r),
                    _ => {
                        c.expr(e);
                    }
                },
                None => {}
            }
        });
    }

    fn params(&mut self, params: &[Param]) {
        for p in params {
            let t = self.type_of(&p.ty);
            if let Some(d) = &p.default {
                self.fits(d, &t);
            }
            self.bind(&p.name.text, t);
        }
    }

    /// A patch's code: `this` the patched class; its methods' parameters bound.
    fn patch(&mut self, items: &[PatchItem]) {
        for item in items {
            match item {
                PatchItem::Add(d) => self.decl(d),
                PatchItem::Before { method, body, .. }
                | PatchItem::After { method, body, .. }
                | PatchItem::Replace { method, body } => {
                    self.scoped(|c| {
                        c.params(&method.params);
                        c.block(body);
                    });
                }
                PatchItem::Inside { method, points } => {
                    for point in points {
                        let body = match point {
                            Point::Around { body, .. }
                            | Point::Arg { body, .. }
                            | Point::BeforeCall { body, .. }
                            | Point::AfterCall { body, .. }
                            | Point::Field { body, .. }
                            | Point::New { body, .. } => body,
                            Point::Constant { .. } | Point::Local { .. } => continue,
                        };
                        self.scoped(|c| {
                            c.params(&method.params);
                            c.block(body);
                        });
                    }
                }
                PatchItem::Read { body, .. } | PatchItem::Write { body, .. } => self.block(body),
                PatchItem::AddBases(_) | PatchItem::Priority(..) => {}
            }
        }
    }
}

/// The side a declaration's modifiers give it.
fn side_of(d: &Decl) -> Option<Side> {
    d.modifiers.iter().find_map(|m| match m {
        Modifier::Server => Some(Side::Server),
        Modifier::Client => Some(Side::Client),
        Modifier::Shared => Some(Side::Shared),
        _ => None,
    })
}
