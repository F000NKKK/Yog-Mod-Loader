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

impl YogReply {
    /// A reply that appends to `out`; valid while `out` is.
    pub fn to_vec(out: &mut Vec<u8>) -> Self {
        Self { sink: (out as *mut Vec<u8>).cast::<c_void>(), write: append_to_vec }
    }

    /// Writes `bytes` to `reply` — if there's a reply and anything to write.
    ///
    /// # Safety
    /// `reply` is null or a reply valid for the call.
    pub unsafe fn send(reply: *const Self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(r) = unsafe { reply.as_ref() } {
            unsafe { (r.write)(r.sink, bytes.as_ptr(), bytes.len()) };
        }
    }
}

/// [`YogReply::write`] into the `Vec<u8>` of [`YogReply::to_vec`].
unsafe extern "C" fn append_to_vec(sink: *mut c_void, data: *const u8, len: usize) {
    if sink.is_null() || data.is_null() {
        return;
    }
    unsafe { (*sink.cast::<Vec<u8>>()).extend_from_slice(std::slice::from_raw_parts(data, len)) }
}

/// Where a subscriber runs among an event's (higher first; equal ones in
/// the order they subscribed).
pub mod priority {
    /// Before every other handler (the runtime's cleanup for a new world).
    pub const FIRST: i32 = i32::MAX;
    /// Where a mod's handlers run unless they say otherwise.
    pub const DEFAULT: i32 = 0;
    /// After the mods' handlers (what they scheduled for the tick).
    pub const LATE: i32 = i32::MIN + 1;
    /// After everything (the runtime's flush of what the tick queued).
    pub const LAST: i32 = i32::MIN;
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
