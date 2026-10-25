//! A `Result`'s methods.

use yog_script_ids::members::{EXPECT, IS_ERR, IS_OK, UNWRAP, UNWRAP_OR};

use super::Native;
use crate::value::{Case, Value};

pub(super) fn call(c: &Case, ok: bool, name: &str, args: &[Value]) -> Option<Native> {
    let value = || c.fields.first().cloned().unwrap_or(Value::Void);
    Some(match (name, args) {
        (IS_OK, []) => Ok(Value::Bool(ok)),
        (IS_ERR, []) => Ok(Value::Bool(!ok)),
        (UNWRAP, []) if ok => Ok(value()),
        (UNWRAP, []) => Err(format!("`Unwrap()` on an error: {}", value())),
        (EXPECT, [_]) if ok => Ok(value()),
        (EXPECT, [message]) => Err(format!("{message}: {}", value())),
        (UNWRAP_OR, [fallback]) => Ok(if ok { value() } else { fallback.clone() }),
        _ => return None,
    })
}
