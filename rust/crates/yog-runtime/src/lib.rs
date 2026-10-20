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
mod api_fns;
mod gfx_frame;
mod gfx_table;
mod gl_context;
mod globals;
mod gpu;
mod handlers;
mod intermod;
mod jni_client;
mod jni_init;
mod jni_server;
mod jni_util;
mod logging;
mod machine_api;
mod mod_loading;
mod ticks;

mod bus;
mod client_api;
mod content_blob;
// Hot reload: dev builds only (release ones are optimised past it).
#[cfg(feature = "dev")]
mod control_socket;
mod entity_data;
mod fonts;
#[cfg(feature = "dev")]
mod hot_reload;
mod images;
mod jcall;
mod machine;
#[cfg(feature = "dev")]
mod registrations;
mod svg;
mod textures;
mod ui_jni;
// Entry points for the benchmarks only: never in a build of the loader.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
#[cfg(any(test, feature = "bench"))]
#[doc(hidden)]
pub mod test_jvm;
#[cfg(test)]
mod tests;
