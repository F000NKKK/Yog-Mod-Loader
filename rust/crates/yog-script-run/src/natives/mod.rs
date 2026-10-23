//! What the System library declares without a body, done here: strings',
//! lists' and results' members, `Math`.

mod lists;
mod math;
mod results;
mod text;

use crate::value::Value;

/// A native call's end: its value, or a panic's message.
pub(crate) type Native = Result<Value, String>;

/// `on.name` read natively, if it's a native member.
pub(crate) fn member(on: &Value, name: &str) -> Option<Value> {
    match on {
        Value::Str(s) => text::member(s, name),
        Value::List(items) => lists::member(&items.borrow(), name),
        _ => None,
    }
}

/// `on.name(args)` done natively, if it's a native method; `ok`: `on` is a
/// `Result`'s value case.
pub(crate) fn call(on: &Value, ok: bool, name: &str, args: &[Value]) -> Option<Native> {
    match on {
        Value::Str(s) => text::call(s, name, args),
        Value::List(items) => lists::call(items, name, args),
        Value::Case(c) => results::call(c, ok, name, args),
        _ => None,
    }
}

/// `Type.name(args)` done natively.
pub(crate) fn call_static(of: &str, name: &str, args: &[Value]) -> Option<Native> {
    math::call(of, name, args)
}
