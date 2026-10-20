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

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use glow::HasContext;
use jcall::JArg;
use jni::objects::{JClass, JFloatArray, JString};
use jni::sys::{jfloat, jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::Symbol;
use yog_hot_reload::{HotReloader, LoadedModule, ModuleGeneration, ModuleRegistry};

use yog_abi::{
    ABI_VERSION, YogApi, YogGfxApi,
    YogStr,
};
use yog_registry::{BlockDef, ItemDef};
use yog_sync::{Locked, ReadWrite};

#[macro_use]
mod macros;
mod globals;
use globals::*;
mod gl_context;
use gl_context::*;
mod handlers;
use handlers::*;
mod jni_util;
use jni_util::*;
mod logging;
use logging::*;
mod gpu;
use gpu::*;
mod gfx_frame;
mod gfx_table;
use gfx_table::*;
mod api_fns;
use api_fns::*;
mod machine_api;
use machine_api::*;
mod intermod;
use intermod::*;
mod mod_loading;
use mod_loading::*;
mod jni_init;
#[cfg(feature = "dev")]
use jni_init::trigger_hot_reload;
mod ticks;
mod jni_server;
mod jni_client;


mod bus;
// Hot reload: dev builds only (release ones are optimised past it).
#[cfg(feature = "dev")]
mod registrations;
mod entity_data;
mod client_api;
use client_api::{client_advancement_done, client_tag_items};
mod content_blob;
mod machine;
#[cfg(feature = "dev")]
mod control_socket;
mod images;
mod svg;
mod textures;
mod fonts;
mod jcall;
mod ui_jni;
// Entry points for the benchmarks only: never in a build of the loader.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
#[cfg(test)]
mod tests;
