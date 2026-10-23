//! Yog runtime — the native library the Java hosts load.
//!
//! Exposes JNI entry points (`Java_dev_yog_*`) that the hosts call, and the
//! C ABI (`YogApi`, with the bus `YogBus`) that mods program against.
//!
//! Architecture:
//!   - the bus (`bus`) — the router of calls, events and services between
//!     mods, the runtime and the Java host; the world's actions are services
//!     the host provides (`docs/design/bus.md`).
//!   - `YogApi` — a `#[repr(C)]` table of what isn't a message: content
//!     registration, the bus, the frame's graphics, the client's own calls.
//!   - `RuntimeHandlers` — the content mods registered, read by the hosts.

#[macro_use]
mod macros;

mod api;
mod bus;
mod game;
mod gfx;
mod globals;
mod jni;
// `jcall!` reaches its types as `$crate::jcall`: here, and in the JNI
// benchmark that includes the file as its own `jcall` module.
use jni::call as jcall;
mod logging;
mod mods;
mod nanites;
// Benchmarks' and tests' entry points: never in a build of the loader.
#[cfg(any(test, feature = "bench"))]
#[doc(hidden)]
pub mod testing;
mod video;
