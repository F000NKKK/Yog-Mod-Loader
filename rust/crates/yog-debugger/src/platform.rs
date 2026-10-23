//! This system's backend ([`crate::target::Target`]): `Process`, and
//! `interrupt` to break into its wait from another thread.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Process, interrupt};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::{Process, interrupt};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Process, interrupt};

/// AArch64's general registers by name, as the debugger shows them.
#[cfg(target_arch = "aarch64")]
const AARCH64_NAMES: [&str; yog_symbols::aarch64::GENERAL] = [
    "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13", "x14",
    "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27",
    "x28", "fp", "lr",
];

/// AArch64's registers by name: pc, sp, the general ones, the flags.
#[cfg(target_arch = "aarch64")]
pub(crate) fn aarch64_named(
    x: &[u64; yog_symbols::aarch64::GENERAL],
    sp: u64,
    pc: u64,
    flags: u64,
) -> Vec<(&'static str, u64)> {
    let mut named = vec![("pc", pc), ("sp", sp)];
    named.extend(AARCH64_NAMES.iter().copied().zip(x.iter().copied()));
    named.push(("cpsr", flags));
    named
}
