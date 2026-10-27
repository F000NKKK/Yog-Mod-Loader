//! A type's body as its members: values by type as written, methods,
//! indexers, which methods are `readonly`.

use super::{Indexer, OwnType, threads};
use crate::ast::{self, Decl, DeclKind};
use yog_script_ids::system::SETS_REQUIRED_MEMBERS;

/// The values, methods and indexers of a body.
pub(super) fn members(t: &mut OwnType, body: &[Decl]) {
    for d in body {
        match &d.kind {
            DeclKind::Field(f) => {
                marked(t, d, &f.name.text);
                t.values.insert(f.name.text.clone(), f.ty.clone());
                threads::field(t, d, &f.name.text);
            }
            DeclKind::Property {
                ty,
                name,
                explicit: None,
                ..
            } => {
                marked(t, d, &name.text);
                t.values.insert(name.text.clone(), ty.clone());
                if crate::ast::stores_value(d) {
                    threads::field(t, d, &name.text);
                }
            }
            DeclKind::Event { ty, name } => {
                t.values.insert(name.text.clone(), ty.clone());
            }
            DeclKind::Operator(op) => t.operators.push(op.clone()),
            DeclKind::Ctor(c) => {
                t.ctors.push(c.params.clone());
                t.ctor_crossings.push(threads::crossings(d));
                let sets = threads::attributes(d, SETS_REQUIRED_MEMBERS)
                    .next()
                    .is_some();
                t.ctor_sets_required.push(sets);
            }
            DeclKind::Indexer(ix) => {
                if let Some(get) = ix.getter() {
                    t.indexers.push(Indexer {
                        get: get.clone(),
                        settable: ix.setter().is_some(),
                    });
                }
            }
            // An explicit implementation: its interface's, not the type's.
            DeclKind::Method(m) if m.explicit.is_some() => {}
            DeclKind::Method(m) => {
                // A static method has no object to change.
                if d.has(ast::Modifier::Readonly) || d.has(ast::Modifier::Static) {
                    t.readonly.push(m.name.text.clone());
                }
                t.methods
                    .entry(m.name.text.clone())
                    .or_default()
                    .push(m.clone());
                if d.has(ast::Modifier::Static) {
                    t.static_methods.push(m.name.text.clone());
                }
                t.templates
                    .entry(m.name.text.clone())
                    .or_default()
                    .push(super::templates::template(d));
                t.crossings
                    .entry(m.name.text.clone())
                    .or_default()
                    .push(threads::crossings(d));
            }
            _ => {}
        }
    }
}

/// Member `name` (`d`) noted `required` and `synced` when it's marked so.
fn marked(t: &mut OwnType, d: &Decl, name: &str) {
    if d.has(ast::Modifier::Required) {
        t.required.push(name.to_string());
    }
    if d.has(ast::Modifier::Synced) {
        t.synced.push(name.to_string());
    }
}
