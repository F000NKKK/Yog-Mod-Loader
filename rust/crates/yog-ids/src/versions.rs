//! Versions two parts of Yog must agree on.

/// The `.yui` code yog writes (`yog build`) and yog-kit runs: generated
/// code checks it at compile time (`yog_kit::__codegen::require`).
pub const UI_CODEGEN: u32 = 3;
