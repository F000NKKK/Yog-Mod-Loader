//! A type's body as its members: values by type as written, methods,
//! indexers, which methods are `readonly`.

use super::{Indexer, OwnType};
use crate::ast::{self, Decl, DeclKind};

/// The values, methods and indexers of a body.
pub(super) fn members(t: &mut OwnType, body: &[Decl]) {
    for d in body {
        match &d.kind {
            DeclKind::Field(f) => {
                t.values.insert(f.name.text.clone(), f.ty.clone());
            }
            DeclKind::Property {
                ty,
                name,
                explicit: None,
                ..
            } => {
                t.values.insert(name.text.clone(), ty.clone());
            }
            DeclKind::Event { ty, name } => {
                t.values.insert(name.text.clone(), ty.clone());
            }
            DeclKind::Operator(op) => t.operators.push(op.clone()),
            DeclKind::Indexer(ix) => {
                if let Some(get) = ix.getter() {
                    t.indexers.push(Indexer {
                        get,
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
            }
            _ => {}
        }
    }
}
