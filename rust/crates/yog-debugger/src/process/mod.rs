//! The debugged process: what a backend offers, this system's backend, the
//! machine it runs on, finding the process to attach to.

pub(crate) mod arch;
#[cfg(target_os = "linux")]
pub mod discovery;
pub(crate) mod platform;
pub mod target;
