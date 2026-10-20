//! Calling the bus from this binary: the one way it's reached is set once
//! (by `yog-api` in a mod, by the runtime in itself), and every action's
//! function and [`call`] go through it.

use std::sync::OnceLock;

use crate::{Message, Service};

/// Sends `payload` to `address`'s provider: its reply's bytes, `None` when
/// no one answers.
pub type Caller = fn(address: &str, payload: &[u8]) -> Option<Vec<u8>>;

static CALLER: OnceLock<Caller> = OnceLock::new();

/// How this binary reaches the bus; the first one set stays.
pub fn set_caller(caller: Caller) {
    let _ = CALLER.set(caller);
}

/// Asks `request`'s provider: its reply, `None` when no one answers (or the
/// bus isn't reached yet).
pub fn call<S: Service>(request: &S) -> Option<S::Reply> {
    let caller = CALLER.get()?;
    S::Reply::from_bytes(&caller(&request.target(), &request.to_bytes())?)
}
