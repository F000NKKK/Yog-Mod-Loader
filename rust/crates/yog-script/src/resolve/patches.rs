//! Patches: `self` is the patched object; a `before` may `skip`; the
//! method's parameters and the locals it asks for are names in its body.

use super::{Context, DefKind, Resolver};
use crate::ast::{Block, Local, Method, Patch, PatchItem, Point, Selector};

impl Resolver<'_, '_> {
    pub(super) fn patch(&mut self, p: &Patch) {
        self.ty(&p.target);
        self.members(|r| {
            for item in &p.items {
                if let PatchItem::Add(d) = item {
                    r.decls(std::slice::from_ref(d.as_ref()));
                }
            }
            for item in &p.items {
                r.patch_item(item);
            }
        });
    }

    fn patch_item(&mut self, item: &PatchItem) {
        match item {
            PatchItem::AddIs(t) => self.ty(t),
            PatchItem::Before { method, uses, body } => self.code(Some(method), uses, body, true),
            PatchItem::After { method, uses, body } => self.code(Some(method), uses, body, false),
            PatchItem::Replace { method, body } => self.code(Some(method), &[], body, false),
            PatchItem::Inside { method, points } => {
                for point in points {
                    self.point(method, point);
                }
            }
            PatchItem::Read { body, .. } => self.code(None, &[], body, false),
            PatchItem::Write { value, body, .. } => self.patch_code(false, |r| {
                r.define(value, DefKind::Param, false);
                r.stmts(&body.stmts);
            }),
            PatchItem::Add(_) | PatchItem::Priority(..) => {}
        }
    }

    /// A point inside a method: its body sees the method's parameters.
    fn point(&mut self, method: &Method, point: &Point) {
        match point {
            Point::Around { select, body, .. } => {
                self.selector(select.as_ref());
                self.code(Some(method), &[], body, false);
            }
            Point::Arg {
                select, arg, body, ..
            } => {
                self.selector(select.as_ref());
                self.patch_code(false, |r| {
                    r.params(&method.params);
                    r.define(arg, DefKind::Variable, true);
                    r.stmts(&body.stmts);
                });
            }
            Point::BeforeCall {
                select, uses, body, ..
            }
            | Point::AfterCall {
                select, uses, body, ..
            }
            | Point::Field {
                select, uses, body, ..
            } => {
                self.selector(select.as_ref());
                self.code(Some(method), uses, body, false);
            }
            Point::New {
                ty,
                select,
                uses,
                body,
            } => {
                self.ty(ty);
                self.selector(select.as_ref());
                self.code(Some(method), uses, body, false);
            }
            Point::Constant { value, to, select } => {
                self.selector(select.as_ref());
                self.expr(value);
                self.expr(to);
            }
            Point::Local { local, value } => {
                self.ty(&local.ty);
                self.expr(value);
            }
        }
    }

    /// Code run in the patched method: its parameters, the locals it asks
    /// for (assignable when written back), then `body`.
    fn code(&mut self, method: Option<&Method>, uses: &[Local], body: &Block, before: bool) {
        self.patch_code(before, |r| {
            if let Some(m) = method {
                r.params(&m.params);
                r.opt_ty(m.ret.as_ref());
            }
            for l in uses {
                r.ty(&l.ty);
                let kind = if l.write {
                    DefKind::Variable
                } else {
                    DefKind::Value
                };
                r.define(&l.name, kind, false);
            }
            r.stmts(&body.stmts);
        });
    }

    /// A patch's code in a scope of its own: `self` the object, `skip`
    /// allowed when `before`.
    fn patch_code(&mut self, before: bool, f: impl FnOnce(&mut Self)) {
        let cx = Context {
            before,
            member: true,
            ..Context::default()
        };
        self.within(cx, |r| r.scoped(f));
    }

    fn selector(&mut self, s: Option<&Selector>) {
        if let Some(Selector::Where(e)) = s {
            self.expr(e);
        }
    }
}
