//! The bus (ABI 3.11): one channel for every call, event and service, from
//! any side — a Yog mod, the runtime, the Java host, a Java mod. A message
//! is an address (`namespace:path`, resolved once to a [`YogAddress`]) and a
//! payload in `yog-data`'s binary form. See `docs/design/bus.md`.

use std::os::raw::c_void;

use crate::{YogBytesSink, YogStr};

/// An address resolved by [`YogBus::resolve`]: an index, stable for the
/// runtime's lifetime, so hot calls don't hash strings.
pub type YogAddress = u32;

/// What [`YogBus::resolve`] gives for an empty or malformed address.
pub const NO_ADDRESS: YogAddress = u32::MAX;

/// Bytes handed across (`yog-data`'s binary form), borrowed for the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct YogBytes {
    pub ptr: *const u8,
    pub len: usize,
}

impl YogBytes {
    pub const EMPTY: YogBytes = YogBytes { ptr: std::ptr::null(), len: 0 };

    pub fn new(bytes: &[u8]) -> YogBytes {
        YogBytes { ptr: bytes.as_ptr(), len: bytes.len() }
    }

    /// # Safety
    /// `ptr` must point at `len` bytes alive for the returned borrow.
    pub unsafe fn as_slice<'a>(self) -> &'a [u8] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }
}

/// Where a reply goes: `write(sink, …)` with its bytes (once; nothing
/// written is an empty reply).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct YogReply {
    pub sink: *mut c_void,
    pub write: YogBytesSink,
}

/// How a call, an emit or a handler ended.
pub type YogStatus = u32;

/// [`YogStatus`] values.
pub mod status {
    use super::YogStatus;
    /// Done; the reply (if any) is written.
    pub const OK: YogStatus = 0;
    /// No one provides the address (not on this host, this game version, or no such mod loaded).
    pub const NO_HANDLER: YogStatus = 1;
    /// The payload isn't what the address takes.
    pub const BAD_PAYLOAD: YogStatus = 2;
    /// The handler failed; the reply, if any, says why.
    pub const FAILED: YogStatus = 3;
    /// An event's handler stopped it: later handlers don't see it, and the
    /// emitter doesn't do what it announced (a pre-event's cancel).
    pub const CANCELLED: YogStatus = 4;
}

/// A provider or an event handler: `ud` as registered, the payload, where
/// its reply goes. For an event, a reply replaces the payload the next
/// handler (and finally the emitter) sees.
pub type YogHandlerFn = unsafe extern "C" fn(ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus;

/// The bus table — the whole boundary for everything but the per-frame
/// direct tables ([`crate::YogGfxApi`]).
#[repr(C)]
pub struct YogBus {
    pub abi_version: u32,
    /// `size_of::<YogBus>()` as the runtime built it.
    pub size: u32,
    pub ctx: *mut c_void,
    /// `address`'s handle; the same address always gives the same handle.
    pub resolve: unsafe extern "C" fn(ctx: *mut c_void, address: YogStr) -> YogAddress,
    /// Calls the provider of `to`; its reply goes to `reply`.
    pub call: unsafe extern "C" fn(ctx: *mut c_void, to: YogAddress, payload: YogBytes, reply: *const YogReply) -> YogStatus,
    /// Runs `event`'s handlers in priority order (highest first); the last
    /// payload (the handlers' replies chained) goes to `reply`.
    pub emit: unsafe extern "C" fn(ctx: *mut c_void, event: YogAddress, payload: YogBytes, reply: *const YogReply) -> YogStatus,
    /// Subscribes `handler` to `event`; dropped with the registering mod.
    pub on: unsafe extern "C" fn(ctx: *mut c_void, event: YogAddress, priority: i32, ud: *mut c_void, handler: YogHandlerFn),
    /// Makes `handler` answer calls to `address`; [`status::FAILED`] if
    /// someone else does already.
    pub provide: unsafe extern "C" fn(ctx: *mut c_void, address: YogAddress, ud: *mut c_void, handler: YogHandlerFn) -> YogStatus,
    /// Whether anyone provides `address` (a feature this host has).
    pub provided: unsafe extern "C" fn(ctx: *mut c_void, address: YogAddress) -> bool,
}

unsafe impl Send for YogBus {}
unsafe impl Sync for YogBus {}
