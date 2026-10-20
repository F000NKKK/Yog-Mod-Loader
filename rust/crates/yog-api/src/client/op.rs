//! The one call behind the local player's and the client world's
//! functions (`client_op`, ABI 3.9): an operation of `yog_ids::client_op`
//! with JSON arguments, a JSON answer.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// Asks the host: `op` with `args`; its answer, when there is one.
pub(crate) fn call(op: &str, args: Value) -> Option<Value> {
    let text = crate::call(&yog_event::client::Op { op: op.to_owned(), args: args.to_string() }).flatten()?;
    serde_json::from_str(&text).ok()
}

/// An answer read as `T`.
pub(crate) fn ask<T: DeserializeOwned>(op: &str, args: Value) -> Option<T> {
    call(op, args).and_then(|v| serde_json::from_value(v).ok())
}
