//! The mods: loaded, their handlers kept, the `YogApi` they register
//! through; in dev builds hot reload and the launcher's control channel.

// Hot reload: dev builds only (release ones are optimised past it).
#[cfg(feature = "dev")]
pub(crate) mod control_socket;
pub(crate) mod handlers;
#[cfg(feature = "dev")]
pub(crate) mod hot_reload;
pub(crate) mod intermod;
pub(crate) mod loading;
#[cfg(feature = "dev")]
pub(crate) mod registrations;
