//! The event structs (Java → Rust).

use super::*;

// ── Event structs (Java → Rust) ───────────────────────────────────────────────

// ── ABI minor 9–10 event structs ──────────────────────────────────────────────

#[repr(C)]
pub struct YogCommandEvent {
    pub name: YogStr,
    pub args: YogStr,
    pub source: YogStr,
    pub uuid: YogStr,
    /// Empty for console-run commands (no world).
    pub dimension: YogStr,
}

// ── ABI minor 10 client event structs ─────────────────────────────────────────

