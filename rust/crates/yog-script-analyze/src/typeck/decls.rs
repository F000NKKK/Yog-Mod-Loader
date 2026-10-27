//! Declarations: fields' values fitting their types, methods' bodies with
//! their side, `this` and result, types' and content's members.

use super::model::Type;
use super::{Checker, generics};
use crate::ast::{
    Accessor, Decl, DeclKind, Method, MethodBody, Modifier, Param, PatchItem, Point, accessor_scope,
};
use crate::index::Side;

impl Checker<'_, '_, '_> {
    pub(super) fn decls(&mut self, decls: &[Decl]) {
        self.check_overloads(decls);
        for d in decls {
            self.decl(d);
        }
    }

    /// Method `m` (declaration `d`): its body, `this` read-only in it when
    /// it's `readonly` — or implements a member that is.
    pub(super) fn method_decl(&mut self, d: &Decl, m: &Method, side: Side) {
        let readonly = d.has(Modifier::Readonly)
            || self.this_readonly()
            || (!d.has(Modifier::Static) && self.implements_readonly(m));
        let outer = std::mem::replace(&mut self.readonly_this, readonly);
        self.method(m, side, d.has(Modifier::Async));
        self.readonly_this = outer;
    }

    pub(super) fn decl(&mut self, d: &Decl) {
        self.attributes(&d.attributes);
        self.extern_has_service(d);
        self.partial_generated(d);
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
                let t = self.accessors(ty, accessors, side);
                if let Some(v) = value {
                    self.fits(v, &t);
                }
            }
            DeclKind::Indexer(ix) => self.indexer_decl(ix, side),
            DeclKind::Operator(op) => self.method(&op.method, side, false),
            DeclKind::Method(m) => self.method_decl(d, m, side),
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
            DeclKind::Mod { name, body } => {
                let this = Some(Type::own(name.text.clone()));
                self.within(this, side, None, |c| c.decls(body));
            }
            DeclKind::Event { ty, .. } => {
                self.type_of(ty);
            }
            DeclKind::Command { body, .. } => self.command(body),
            DeclKind::System { .. } | DeclKind::Test { .. } => {
                self.handler(d, side);
            }
            DeclKind::Patch(p) => {
                let target = self.type_of(&p.target);
                self.within(Some(target), side, None, |c| c.patch(&p.items));
            }
            DeclKind::Namespace { path, body } => {
                let path: Vec<String> = path.iter().map(|n| n.text.clone()).collect();
                match body {
                    None => self.ns = [self.ns.clone(), path].concat(),
                    Some(body) => {
                        let inner = [self.ns.clone(), path].concat();
                        let outer = std::mem::replace(&mut self.ns, inner);
                        self.decls(body);
                        self.ns = outer;
                    }
                }
            }
            // A content's are an error (`content_decl`): its file's.
            DeclKind::Prop(p) if !self.in_content => {
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
                let args: Vec<Type> = header
                    .generics
                    .iter()
                    .filter(|g| !g.lifetime)
                    .map(|g| Type::Param(g.name.text.clone()))
                    .collect();
                let own = (self.own.get(&name.text, args.len()).is_some())
                    .then(|| Type::Own(name.text.clone(), args));
                let bound = generics::params_of(&header.generics);
                self.with_types(bound, |c| {
                    c.with_constraints(&header.constraints, |c| {
                        if let Some(this) = &own {
                            c.check_operators(this, body);
                        }
                        let content = match &d.kind {
                            DeclKind::Content { kind, .. } => Some(kind.text.clone()),
                            _ => None,
                        };
                        c.within(own, side, None, |c| {
                            c.params(header.ctor.as_deref().unwrap_or_default());
                            let outer = std::mem::replace(&mut c.in_content, content.is_some());
                            c.decls(body);
                            c.in_content = outer;
                            if let Some(word) = &content {
                                c.content_decl(word, name, body);
                            }
                        });
                    })
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
            DeclKind::Test { body, .. } => {
                self.within(None, side, Some(Type::Void), |c| c.block(body))
            }
            _ => {}
        }
    }

    fn method(&mut self, m: &Method, side: Side, is_async: bool) {
        self.with_types(generics::params_of(&m.generics), |c| {
            c.with_constraints(&m.constraints, |c| {
                let ret = m.ret.as_ref().map_or(Type::Void, |t| c.type_of(t));
                // An iterator: its items checked by `yield`, not its `return`s.
                let item = super::iterators::body_yields(m.body.as_ref())
                    .then(|| super::iterators::item_of(&ret).cloned())
                    .flatten();
                let ret = if is_async && item.is_none() {
                    c.async_returns(&ret, m.name.span)
                } else {
                    ret
                };
                let outer = std::mem::replace(&mut c.yields, item);
                c.body(
                    (m.body.as_ref(), m.requires.as_ref()),
                    &m.params,
                    Some(ret),
                    side,
                );
                c.yields = outer;
            })
        });
    }

    /// A body: its parameters bound, what it `requires` a `bool` of them,
    /// its result expected.
    fn body(
        &mut self,
        (body, requires): (Option<&MethodBody>, Option<&crate::ast::Expr>),
        params: &[Param],
        returns: Option<Type>,
        side: Side,
    ) {
        let this = self.this.clone();
        self.within(this, side, returns.clone(), |c| {
            c.params(params);
            if let Some(r) = requires {
                c.condition(r);
            }
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
            self.bind_as(&p.name.text, t, p.pass == Some(crate::ast::Pass::In));
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

impl Checker<'_, '_, '_> {
    /// A property's accessors checked, each with what it has in scope; the
    /// property's type.
    fn accessors(&mut self, ty: &crate::ast::Type, accessors: &[Accessor], side: Side) -> Type {
        let t = self.type_of(ty);
        for a in accessors {
            let scope = accessor_scope(ty, a, true);
            self.body(
                (a.body.as_ref(), None),
                &scope,
                Some(accessor_result(a, &t)),
                side,
            );
        }
        t
    }
}

impl Checker<'_, '_, '_> {
    /// An indexer's accessors, as the methods they're compiled as.
    fn indexer_decl(&mut self, ix: &crate::ast::Indexer, side: Side) {
        let t = self.type_of(&ix.ty);
        if let Some(get) = ix.getter() {
            self.body((get.body.as_ref(), None), &get.params, Some(t), side);
        }
        if let Some(set) = ix.setter() {
            self.body(
                (set.body.as_ref(), None),
                &set.params,
                Some(Type::Void),
                side,
            );
        }
    }
}

/// What accessor `a`'s body gives: a `get` the member's type `t`, a `set`
/// or an `init` nothing.
fn accessor_result(a: &Accessor, t: &Type) -> Type {
    if a.name.text == crate::words::get::WORD.text {
        t.clone()
    } else {
        Type::Void
    }
}
