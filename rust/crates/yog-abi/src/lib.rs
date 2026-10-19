//! Yog stable C ABI — the ONLY types that cross the mod/runtime boundary.
//!
//! Rules this file must never break:
//!  - Every type is `#[repr(C)]`.
//!  - No Rust trait objects, no generics, no std types in public structs.
//!  - Adding is a minor change: new fields go at the END of structs, new
//!    callback types and functions are new names; increment `ABI_MINOR`.
//!  - Anything else is a major change: removing (clean out what's obsolete
//!    then, here, in the runtime, the hosts' JNI, yog-api), reordering,
//!    changing a type or a meaning; increment `ABI_MAJOR`, `ABI_MINOR` = 0.
//!
//! Mods and the runtime are compiled independently; the runtime loads a mod
//! of its major version and a minor no newer than its own ([`compatible`]):
//! such a mod sees a prefix of every table, all of it there.

use std::os::raw::c_void;

mod primitives;
pub use primitives::*;
mod events;
pub use events::*;
mod handlers;
pub use handlers::*;
mod gfx;
pub use gfx::*;
mod server;
pub use server::*;
mod chunk_gen;
pub use chunk_gen::*;
mod api;
pub use api::*;
mod bus;
pub use bus::*;
#[cfg(test)]
mod tests;

// ── Version ──────────────────────────────────────────────────────────────────

pub const ABI_MAJOR: u32 = 3;
pub const ABI_MINOR: u32 = 11;
/// `ABI_MAJOR * 10_000 + ABI_MINOR`.  Checked at mod load time.
pub const ABI_VERSION: u32 = ABI_MAJOR * 10_000 + ABI_MINOR;

/// Whether a mod built for ABI `version` runs on this one: the same major,
/// a minor no newer (what it knows is all here; later additions it doesn't use).
pub const fn compatible(version: u32) -> bool {
    version / 10_000 == ABI_MAJOR && version % 10_000 <= ABI_MINOR
}
