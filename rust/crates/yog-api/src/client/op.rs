//! The one call behind the local player's and the client world's
//! functions (`client_op`, ABI 3.9): an operation of `yog_ids::client_op`
//! with JSON arguments, a JSON answer.

use serde::de::DeserializeOwned;
use serde_json::Value;
use yog_abi::YogStr;

use crate::registry::{global_api, take_owned};

/// Asks the host: `op` with `args`; its answer, when there is one.
pub(crate) fn call(op: &str, args: Value) -> Option<Value> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    let args = args.to_string();
    let owned = unsafe { ((*api).client_op)((*api).ctx, YogStr::from_str(op), YogStr::from_str(&args)) };
    let text = take_owned(unsafe { (*api).free_str }, owned)?;
    serde_json::from_str(&text).ok()
}

/// An answer read as `T`.
pub(crate) fn ask<T: DeserializeOwned>(op: &str, args: Value) -> Option<T> {
    call(op, args).and_then(|v| serde_json::from_value(v).ok())
}
