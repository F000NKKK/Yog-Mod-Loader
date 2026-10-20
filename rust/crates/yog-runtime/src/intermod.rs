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

pub(super) fn build_api_table(ctx: *const RwLock<RuntimeHandlers>, server: *const YogServer) -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        ctx: ctx as *mut c_void,
        server,
        register_content: api_register_content,
        register_ui: api_register_ui,
        register_menu_entry: api_register_menu_entry,
        mods_list: api_mods_list,
        free_str: yog_free_str,
        ui_open: api_ui_open,
        interop_export: interop_export_impl,
        interop_import: interop_import_impl,
        register_chunk_generator: api_register_chunk_generator,
        ui_close: api_ui_close,
        ui_consume_event: api_ui_consume_event,
        translate: api_translate,
        ui_open_flags: client_api::api_ui_open_flags,
        client_play_sound: client_api::api_client_play_sound,
        client_stop_sound: client_api::api_client_stop_sound,
        client_held_item: client_api::api_client_held_item,
        client_set_block_tint: client_api::api_client_set_block_tint,
        client_key_down: client_api::api_client_key_down,
        on_book_page: api_on_book_page,
        on_item_tooltip: api_on_item_tooltip,
        is_mod_loaded: api_is_mod_loaded,
        java_call: api_java_call,
        client_body_track: body::api_client_body_track,
        client_body_track_type: body::api_client_body_track_type,
        client_set_dimensions: body::api_client_set_dimensions,
        client_entity_data: entity_data::api_client_entity_data,
        client_attribute_value: api_client_attribute_value,
        client_advancement_done: api_client_advancement_done,
        client_op: client_api::api_client_op,
        register_key_binding: api_register_key_binding,
        log: api_log,
        bus: crate::bus::table(),
        gfx: crate::gfx_frame::api_gfx,
    }
}
