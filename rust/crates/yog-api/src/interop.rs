//! Inter-mod communication — export/import function pointers between mods.
//!
//! Uses the runtime's global symbol table (C ABI). Mods export typed function
//! pointers under `"symbol"` names; other mods import them by qualified name
//! `"mod_id:symbol"`.
//!
//! ## Example
//!
//! ```ignore
//! // Mod A exports:
//! fn register_pipe_impl(api: *const YogApi, registry: &mut Registry, def: ...) { ... }
//! registry.interop().export("register_pipe", register_pipe_impl as *const c_void);
//!
//! // Mod B imports:
//! type RegisterPipeFn = unsafe extern "C" fn(api: *const YogApi, ...);
//! let func: RegisterPipeFn = registry.interop().import("yog-pipes:register_pipe").unwrap();
//! unsafe { func(api_ptr, ...) };
//! ```

use std::os::raw::c_void;

/// Safe wrapper around the runtime's inter-mod symbol table.
///
/// Returned by [`Registry::interop`](crate::Registry::interop).
pub struct Interop {
    api: *const crate::YogApi,
}

impl Interop {
    pub(crate) fn new(api: *const crate::YogApi) -> Self {
        Interop { api }
    }

    /// Export a function pointer under `symbol` for the current mod.
    ///
    /// The mod's ID is automatically determined from the manifest (passed
    /// to `yog_mod_register` by the runtime).
    pub fn export(&self, symbol: &str, ptr: *const c_void) {
        let mod_id = crate::__current_mod_id().unwrap_or_else(|| "unknown".into());
        unsafe {
            let api = &*self.api;
            let mid = yog_abi::YogStr::from_str(&mod_id);
            let sym = yog_abi::YogStr::from_str(symbol);
            (api.interop_export)(api.ctx, mid, sym, ptr);
        }
    }

    /// Import a function pointer exported by `mod_id` under `symbol`.
    ///
    /// Use the qualified form `"mod_id:symbol"`:
    ///
    /// ```ignore
    /// let ptr: *const c_void = interop.import("yog-pipes:register_pipe").unwrap();
    /// ```
    ///
    /// Returns `None` if the symbol has not been exported (yet).
    pub fn import_raw(&self, mod_id: &str, symbol: &str) -> Option<*const c_void> {
        unsafe {
            let api = &*self.api;
            let mid = yog_abi::YogStr::from_str(mod_id);
            let sym = yog_abi::YogStr::from_str(symbol);
            let ptr = (api.interop_import)(api.ctx, mid, sym);
            if ptr.is_null() { None } else { Some(ptr) }
        }
    }

    /// Convenience: parse `"mod_id:symbol"` and import.
    pub fn import(&self, qualified: &str) -> Option<*const c_void> {
        let (mod_id, symbol) = qualified.split_once(':')?;
        self.import_raw(mod_id, symbol)
    }
}
