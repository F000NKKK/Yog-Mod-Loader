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
