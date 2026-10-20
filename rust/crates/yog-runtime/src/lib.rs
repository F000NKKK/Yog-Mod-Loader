//! Yog runtime — the native library loaded by the Fabric host.
//!
//! Exposes JNI entry points (`Java_dev_yog_NativeBridge_*`) that the host calls,
//! and a stable C ABI (`YogApi` / `YogServer`) that mods program against.
//!
//! Architecture:
//!   - `YogServer`  — a `#[repr(C)]` table of standalone JNI-calling functions
//!     that mods call to mutate the world.
//!   - `YogApi`     — a `#[repr(C)]` table of registration functions; mods call
//!     them inside `yog_mod_register` to subscribe to events.
//!   - `RuntimeHandlers` — the runtime's internal event/handler storage.
//!     Filled during `nativeInit` (write), read-only after. The scheduler sub-
//!     state uses an inner `Mutex` for safe addition during event dispatch.

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
    ABI_VERSION, YogApi, YogBlockPos, YogCommandFn, YogGfxApi, YogHudRenderFn, YogOwnedStr,
    YogScheduledFn, YogServer, YogStr, YogVec3, YogWorldRenderFn,
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
mod owned_str;
use owned_str::*;
mod server_fns;
use server_fns::*;
mod logging;
use logging::*;
mod gpu;
use gpu::*;
mod gfx_table;
use gfx_table::*;
mod api_fns;
use api_fns::*;
mod machine_api;
use machine_api::*;
mod world_fns;
use world_fns::*;
mod tables;
use tables::*;
mod abi3;
use abi3::*;
mod intermod;
use intermod::*;
mod mod_loading;
use mod_loading::*;
mod jni_init;
use jni_init::*;
mod ticks;
mod jni_server;
mod jni_client;


mod bus;
mod body;
// Hot reload: dev builds only (release ones are optimised past it).
#[cfg(feature = "dev")]
mod registrations;
mod entity_data;
mod client_api;
mod content_blob;
mod machine;
#[cfg(feature = "dev")]
mod control_socket;
mod images;
mod svg;
mod jcall;
mod ui_jni;
// Entry points for the benchmarks only: never in a build of the loader.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
#[cfg(test)]
mod tests;
