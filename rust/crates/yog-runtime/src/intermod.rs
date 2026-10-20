//! The `YogApi` table a mod registers through.

use super::*;

pub(super) fn build_api_table(ctx: *const RwLock<RuntimeHandlers>) -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        ctx: ctx as *mut c_void,
        register_content: api_register_content,
        free_str: yog_free_str,
        java_call: api_java_call,
        log: api_log,
        bus: crate::bus::table(),
        gfx: crate::gfx_frame::api_gfx,
    }
}
