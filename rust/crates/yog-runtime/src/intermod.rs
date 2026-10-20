//! Inter-mod communication.

use super::*;

// ── Inter-mod communication ──────────────────────────────────────────────────

pub(super) unsafe extern "C" fn interop_export_impl(
    _ctx: *mut c_void,
    mod_id: YogStr,
    symbol: YogStr,
    ptr: *const c_void,
) {
    unsafe {
        let mod_id = mod_id.as_str();
        let sym = symbol.as_str();
        let registry = INTEROP_SYMBOLS.get_or_init(|| Mutex::new(HashMap::new()));
        registry
            .locked()
            .entry(mod_id.to_string())
            .or_default()
            .insert(sym.to_string(), ptr as usize);
    }
}

pub(super) unsafe extern "C" fn interop_import_impl(
    _ctx: *mut c_void,
    mod_id: YogStr,
    symbol: YogStr,
) -> *const c_void {
    unsafe {
        let mod_id = mod_id.as_str();
        let sym = symbol.as_str();
        let registry = INTEROP_SYMBOLS.get_or_init(|| Mutex::new(HashMap::new()));
        registry
            .locked()
            .get(mod_id)
            .and_then(|m: &HashMap<String, usize>| m.get(sym))
            .map(|&ptr| ptr as *const c_void)
            .unwrap_or(std::ptr::null())
    }
}

pub(super) fn build_api_table(ctx: *const RwLock<RuntimeHandlers>) -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        ctx: ctx as *mut c_void,
        register_content: api_register_content,
        free_str: yog_free_str,
        interop_export: interop_export_impl,
        interop_import: interop_import_impl,
        java_call: api_java_call,
        log: api_log,
        bus: crate::bus::table(),
        gfx: crate::gfx_frame::api_gfx,
    }
}
