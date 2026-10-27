//! What an accessor has in scope besides its own: `value` (a `set`'s or an
//! `init`'s new value) and, a property's, `field` (its stored value, as
//! C# 13's). One rule, for everything that walks accessors.

use super::{Accessor, Name, Param, Type};
use crate::words::{field, init, set, value};

/// Accessor `a` of a member of type `ty` (`property`: a property's, not an
/// indexer's): the names it has beyond the member's parameters.
pub fn accessor_scope(ty: &Type, a: &Accessor, property: bool) -> Vec<Param> {
    let writes = [set::WORD.text, init::WORD.text].contains(&a.name.text.as_str());
    let mut names = Vec::new();
    if writes {
        names.push(named(value::WORD.text, ty, &a.name));
    }
    if property {
        names.push(named(field::WORD.text, ty, &a.name));
    }
    names
}

/// A `set`'s `value` parameter of type `ty` (an indexer's setter takes it last).
pub fn value_param(ty: &Type, at: &Name) -> Param {
    named(value::WORD.text, ty, at)
}

fn named(text: &str, ty: &Type, at: &Name) -> Param {
    Param {
        params: false,
        extension: false,
        pass: None,
        lifetime: None,
        ty: ty.clone(),
        name: Name {
            text: text.to_string(),
            span: at.span,
        },
        default: None,
    }
}

/// Whether property `d` is a stored value (an auto-property, `T X { get;
/// set; }`): its accessors without bodies, and it not `abstract` (an
/// abstract one's accessors are its overrides').
pub fn stores_value(d: &super::Decl) -> bool {
    match &d.kind {
        super::DeclKind::Property { accessors, .. } => {
            accessors.iter().all(|a| a.body.is_none()) && !d.has(super::Modifier::Abstract)
        }
        _ => false,
    }
}

/// Whether a property of `accessors` keeps a value of its own (C#'s
/// `field`): one with a body reaches `field`, or one has no body beside one
/// that has. A property of none with bodies is all its own value.
pub fn keeps_value(accessors: &[Accessor]) -> bool {
    let bodies = accessors.iter().filter(|a| a.body.is_some()).count();
    if bodies == 0 {
        return false;
    }
    bodies < accessors.len()
        || accessors
            .iter()
            .filter_map(|a| a.body.as_ref())
            .any(reaches_field)
}

fn reaches_field(body: &super::MethodBody) -> bool {
    let mut found = false;
    match body {
        super::MethodBody::Expr(e) => found = names_field(e),
        super::MethodBody::Block(b) => b
            .stmts
            .iter()
            .for_each(|s| s.each_expr(&mut |e| found |= names_field(e))),
    }
    found
}

fn names_field(e: &super::Expr) -> bool {
    if matches!(&e.kind, super::ExprKind::Ident(n) if n == field::WORD.text) {
        return true;
    }
    let mut found = false;
    e.each_child(|c| found |= names_field(c));
    found
}
