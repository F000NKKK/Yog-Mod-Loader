//! A `Result`'s methods.

use super::Native;
use crate::value::{Case, Value};

pub(super) fn call(c: &Case, ok: bool, name: &str, args: &[Value]) -> Option<Native> {
    let value = || c.fields.first().cloned().unwrap_or(Value::Void);
    Some(match (name, args) {
        ("IsOk", []) => Ok(Value::Bool(ok)),
        ("IsErr", []) => Ok(Value::Bool(!ok)),
        ("Unwrap", []) if ok => Ok(value()),
        ("Unwrap", []) => Err(format!("`Unwrap()` on an error: {}", value())),
        ("Expect", [_]) if ok => Ok(value()),
        ("Expect", [message]) => Err(format!("{message}: {}", value())),
        ("UnwrapOr", [fallback]) => Ok(if ok { value() } else { fallback.clone() }),
        _ => return None,
    })
}
