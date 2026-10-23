//! The game's classes as Yog Script sees them: JVM descriptors as types,
//! and members by C#'s names — a getter is a property (`Health` is
//! `getHealth()`, `isDay()` is `IsDay`, `level()` is `Level`), a method
//! PascalCase (`Hurt` is `hurt`), a field PascalCase (`FoodData` is `foodData`).

use super::model::Type;
use crate::index::{Class, Lookup, Member, access};

/// The JVM's string class: Yog Script's `string`.
const JAVA_STRING: &str = "java/lang/String";
/// What a getter's name starts with.
const GETTER_PREFIXES: [&str; 2] = ["get", "is"];

/// The type a field descriptor (`I`, `Ljava/lang/String;`, `[F`) names, and
/// the rest of the descriptor after it.
pub(crate) fn parse_desc(desc: &str) -> Option<(Type, &str)> {
    let mut chars = desc.chars();
    let ty = match chars.next()? {
        'I' => Type::I32,
        'S' => Type::I16,
        'J' => Type::I64,
        'F' => Type::F32,
        'D' => Type::F64,
        'Z' => Type::Bool,
        'B' => Type::I8,
        'C' => Type::Char,
        'V' => Type::Void,
        '[' => {
            let (item, rest) = parse_desc(chars.as_str())?;
            return Some((Type::List(Box::new(item)), rest));
        }
        'L' => {
            let rest = chars.as_str();
            let end = rest.find(';')?;
            let name = &rest[..end];
            let ty = if name == JAVA_STRING {
                Type::String
            } else {
                Type::Game(name.to_string())
            };
            return Some((ty, &rest[end + 1..]));
        }
        _ => return None,
    };
    Some((ty, chars.as_str()))
}

/// A method descriptor `(params)ret` as its parameters' and result's types.
pub(crate) fn method_types(desc: &str) -> Option<(Vec<Type>, Type)> {
    let mut rest = desc.strip_prefix('(')?;
    let mut params = Vec::new();
    while !rest.starts_with(')') {
        let (ty, after) = parse_desc(rest)?;
        params.push(ty);
        rest = after;
    }
    let (ret, _) = parse_desc(&rest[1..])?;
    Some((params, ret))
}

/// `Health` → `health`.
fn camel(name: &str) -> String {
    yog_codegen::camel(name)
}

fn is_public(m: &Member) -> bool {
    m.access & access::PUBLIC != 0
}

/// What a C# member name reaches on a game class.
pub(crate) enum Found {
    /// A field or a getter: the value's type.
    Value(Type),
    /// Methods of that name: each one's parameters and result.
    Methods(Vec<(Vec<Type>, Type)>),
    Nothing,
    /// A member it can't use as written (not public, not readable): not checked.
    Unsure,
}

/// The member `name` (as a source writes it) of `class`: a getter or a
/// field when `call` is false, the methods when it's called.
pub(crate) fn member<'i>(lookup: &Lookup<'i>, class: &'i Class, name: &str, call: bool) -> Found {
    let java = camel(name);
    if call {
        let methods: Vec<_> = lookup
            .methods(class, &java)
            .into_iter()
            .filter(|m| is_public(m))
            .filter_map(|m| method_types(&m.desc))
            .collect();
        return if methods.is_empty() {
            Found::Nothing
        } else {
            Found::Methods(methods)
        };
    }
    let getters = GETTER_PREFIXES
        .iter()
        .map(|p| format!("{p}{name}"))
        .chain([java.clone()]);
    for getter in getters {
        let zero_args = lookup
            .methods(class, &getter)
            .into_iter()
            .filter(|m| is_public(m))
            .filter_map(|m| method_types(&m.desc))
            .find(|(params, ret)| params.is_empty() && *ret != Type::Void);
        if let Some((_, ret)) = zero_args {
            return Found::Value(ret);
        }
    }
    match lookup.field(class, &java) {
        Some(f) if is_public(f) => {
            parse_desc(&f.desc).map_or(Found::Unsure, |(t, _)| Found::Value(t))
        }
        Some(_) => Found::Unsure,
        None => Found::Nothing,
    }
}

#[cfg(test)]
mod tests;
