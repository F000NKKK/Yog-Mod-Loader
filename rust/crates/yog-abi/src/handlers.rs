//! The callbacks still outside the bus: byte sinks.

use super::*;

/// Takes bytes a handler writes (copies them; valid for the call).
pub type YogBytesSink = unsafe extern "C" fn(sink: *mut c_void, data: *const u8, len: usize);
