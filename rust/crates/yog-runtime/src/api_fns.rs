//! `YogApi`'s registration functions.

use super::*;

// ── YogApi registration functions ─────────────────────────────────────────────
//
// ctx is *const RwLock<RuntimeHandlers> (cast from *mut c_void) — see
// `current_registration()` for how each entry gets tagged with the
// `LoadedModule` that's registering it.

macro_rules! api_event {
    ($name:ident, $field:ident, $fn_ty:ty) => {
        pub(super) unsafe extern "C" fn $name(ctx: *mut c_void, ud: *mut c_void, h: $fn_ty) {
            unsafe {
                let mut handlers = handlers_mut(ctx);
                handlers.$field.push((current_registration(), ud, h));
            }
        }
    };
}

api_event!(api_on_hud_render, hud_render, YogHudRenderFn);
api_event!(api_on_world_render, world_render, YogWorldRenderFn);

/// The handlers a registration call writes into: `ctx` is the
/// `RwLock<RuntimeHandlers>` the runtime handed the mod's `yog_mod_register`.
/// A poisoned lock (a mod panicked mid-registration) is taken as it is —
/// one mod's panic doesn't stop the others registering.
///
/// # Safety
/// `ctx` must be that registration context.
pub(super) unsafe fn handlers_mut<'a>(ctx: *mut c_void) -> std::sync::RwLockWriteGuard<'a, RuntimeHandlers> {
    unsafe { (*(ctx as *const RwLock<RuntimeHandlers>)).write().unwrap_or_else(|e| e.into_inner()) }
}

/// `register_content`: content of `kind` named `id`, its definition a Yog
/// Data blob — the one way mods hand the runtime what the hosts register.
pub(super) unsafe extern "C" fn api_register_content(ctx: *mut c_void, kind: YogStr, id: YogStr, data: *const u8, len: usize) {
    unsafe {
        let kind = kind.as_str();
        let id = id.as_str().to_owned();
        let bytes = if data.is_null() || len == 0 { &[][..] } else { std::slice::from_raw_parts(data, len) };
        // Borrowed: strings stay in `bytes`, only what's kept is copied.
        let arena = yog_data::Bump::new();
        let value = match yog_data::binary::decode_ref(bytes, &arena) {
            Ok(v) => v,
            Err(e) => {
                yog_logging::warn!("{kind} {id}: not Yog Data: {e}");
                return;
            }
        };
        register_content(&mut handlers_mut(ctx), kind, id, &value);
    }
}

/// Stores content of `kind` (see `register_content` in yog-abi).
pub(super) fn register_content(h: &mut RuntimeHandlers, kind: &str, id: String, value: &yog_data::ValueRef) {
    *h.content_blob.get_mut().unwrap_or_else(|e| e.into_inner()) = None;
    fn read<T: yog_data::Schema>(kind: &str, id: &str, v: &yog_data::ValueRef) -> Option<T> {
        match yog_data::read_ref_value::<T>(v) {
            Ok(value) => Some(value),
            Err(problems) => {
                let list: Vec<String> = problems.iter().map(|p| format!("{}: {}", p.path, p.message)).collect();
                yog_logging::warn!("{kind} {id}: {}", list.join("; "));
                None
            }
        }
    }
    let none_if_empty = |s: &mut Option<String>| {
        if s.as_deref() == Some("") {
            *s = None;
        }
    };
    match kind {
        yog_ids::kind::ITEM => {
            if let Some(mut d) = read::<ItemDef>(kind, &id, value) {
                none_if_empty(&mut d.name);
                none_if_empty(&mut d.tooltip);
                h.items.push(d);
            }
        }
        yog_ids::kind::BLOCK => {
            if let Some(mut d) = read::<BlockDef>(kind, &id, value) {
                none_if_empty(&mut d.name);
                none_if_empty(&mut d.sound);
                none_if_empty(&mut d.inventory_id);
                none_if_empty(&mut d.base);
                none_if_empty(&mut d.set);
                h.blocks.push(d);
            }
        }
        yog_ids::kind::ENTITY => {
            if read::<yog_registry::EntityDef>(kind, &id, value).is_some() {
                h.entities.push(value.to_value());
            }
        }
        yog_ids::kind::ATTRIBUTE => {
            if read::<yog_registry::AttributeDef>(kind, &id, value).is_some() {
                h.attributes.push(value.to_value());
            }
        }
        yog_ids::kind::EFFECT => {
            if read::<yog_registry::EffectDef>(kind, &id, value).is_some() {
                h.effects.push(value.to_value());
            }
        }
        yog_ids::kind::POTION => {
            if read::<yog_registry::PotionDef>(kind, &id, value).is_some() {
                h.potions.push(value.to_value());
            }
        }
        yog_ids::kind::BIOME_FEATURE => {
            if let Some(f) = read::<yog_registry::BiomeFeature>(kind, &id, value) {
                h.biome_features.push(f);
            }
        }
        yog_ids::kind::TOOL_MATERIAL => {
            if let Some(m) = read::<yog_registry::ToolMaterial>(kind, &id, value) {
                h.tool_materials.insert(m.id.clone(), m);
            }
        }
        yog_ids::kind::ARMOR_MATERIAL => {
            if let Some(m) = read::<yog_registry::ArmorMaterial>(kind, &id, value) {
                h.armor_materials.insert(m.id.clone(), m);
            }
        }
        yog_ids::kind::SOUND_GROUP => {
            if let Some(g) = read::<yog_registry::SoundGroup>(kind, &id, value) {
                h.sound_groups.insert(g.id.clone(), g);
            }
        }
        yog_ids::kind::INVENTORY => {
            if let Some(d) = read::<yog_inventory::InventoryDef>(kind, &id, value) {
                h.inventories.push(d);
            }
        }
        yog_ids::kind::STARTUP_GRANT => {
            if let Some(mut g) = read::<yog_registry::StartupGrant>(kind, &id, value) {
                none_if_empty(&mut g.book);
                none_if_empty(&mut g.command);
                h.startup_grants.push(g);
            }
        }
        yog_ids::kind::RECIPE => {
            let (ns, name) = yog_ids::split(&id);
            let recipe = value.to_value();
            // Feed the recipe to the books already registered (their recipe pages).
            for r in h.book_renderers.locked().values_mut() {
                r.add_recipe(format!("{ns}:{name}"), &recipe);
            }
            h.recipes.push((ns.to_string(), name.to_string(), recipe));
        }
        yog_ids::kind::BOOK => {
            // Read by its schema; the renderer draws it without a round trip to Java.
            if let Some(mut book) = read::<yog_formats::book::Book>(kind, &id, value) {
                // Entries about mods that aren't here aren't shown.
                book.entries.retain(|e| e.requires.iter().all(|m| mod_loaded(m)));
                h.books.insert(id.clone());
                let mut renderer = yog_book::BookRenderer::new(book);
                // i18n books carry translation keys; shown in the game language.
                renderer.set_translator(translate);
                // Recipes may register before the book — replay them into the renderer.
                for (ns, name, recipe) in &h.recipes {
                    renderer.add_recipe(format!("{ns}:{name}"), recipe);
                }
                h.book_renderers.get_mut().unwrap_or_else(|e| e.into_inner()).insert(id, renderer);
            }
        }
        yog_ids::kind::DIMENSION => {
            yog_logging::info!("registered dimension type: {}", id);
            let mut def = value.to_value();
            // The terrain stays here; the host gets the dimension, its generator this one.
            if let Some(t) = value.get("terrain") {
                let Some(terrain) = read::<yog_worldgen::Terrain>(kind, &id, t) else { return };
                if let yog_data::Value::Map(m) = &mut def {
                    m.remove("terrain");
                    m.insert("generator_type", yog_data::Value::from(id.as_str()));
                }
                h.terrains.insert(id.clone(), Arc::new(yog_worldgen::Generator::new(terrain)));
            }
            h.dimensions.insert(id, def);
        }
        other => yog_logging::warn!("unknown kind of content `{other}` ({id}) — this runtime doesn't know it"),
    }
}




pub(super) unsafe extern "C" fn api_register_chunk_generator(
    ctx: *mut c_void,
    id: YogStr,
    ud: *mut c_void,
    h: yog_abi::YogChunkGeneratorFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = id.as_str().to_owned();
        yog_logging::info!("registered chunk generator for dimension: {}", id);
        handlers
            .chunk_generators
            .insert(id, (current_registration(), ud, h));
    }
}

pub(super) unsafe extern "C" fn api_register_ui(
    ctx: *mut c_void,
    ui_id: YogStr,
    _layout_json: YogStr,
    ud: *mut c_void,
    h: yog_abi::YogUIEventFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = ui_id.as_str().to_owned();
        handlers
            .ui_handlers
            .insert(id.clone(), (current_registration(), ud, h));
        yog_logging::info!("registered UI handler: {}", id);
    }
}

pub(super) unsafe extern "C" fn api_on_ui_render(
    ctx: *mut c_void,
    ui_id: YogStr,
    ud: *mut c_void,
    h: YogHudRenderFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = ui_id.as_str().to_owned();
        handlers
            .ui_render_handlers
            .entry(id)
            .or_default()
            .push((current_registration(), ud, h));
    }
}

pub(super) unsafe extern "C" fn api_on_book_page(
    ctx: *mut c_void,
    drawer: YogStr,
    ud: *mut c_void,
    f: yog_abi::YogPageDrawFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .page_drawers
            .insert(drawer.as_str().to_owned(), (current_registration(), ud, f));
    }
}

pub(super) unsafe extern "C" fn api_on_item_tooltip(
    ctx: *mut c_void,
    item: YogStr,
    ud: *mut c_void,
    size: yog_abi::YogTooltipSizeFn,
    draw: yog_abi::YogTooltipDrawFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .item_tooltips
            .entry(item.as_str().to_owned())
            .or_default()
            .push((current_registration(), ud, size, draw));
    }
}

pub(super) unsafe extern "C" fn api_on_item_text(ctx: *mut c_void, item: YogStr, ud: *mut c_void, f: yog_abi::YogItemTextFn) {
    unsafe {
        handlers_mut(ctx).item_texts.entry(item.as_str().to_owned()).or_default().push((current_registration(), ud, f));
    }
}

pub(super) unsafe extern "C" fn copy_text(sink: *mut c_void, text: YogStr) {
    // SAFETY: `sink` is the `String` `item_text` passes; `text` is valid for the call.
    unsafe { (*(sink as *mut String)).push_str(text.as_str()) }
}

/// A stack's own name and tooltip lines (`name\nline…`), from the first
/// mod that gives them; `None`: the item's own.
pub(super) fn item_text(item: &str, snbt: &str) -> Option<String> {
    let entries = handlers().item_texts.get(item).cloned().unwrap_or_default();
    for (module, ud, f) in entries {
        let Some(_call) = module.enter() else { continue };
        let mut out = String::new();
        let mut given = false;
        guard("item_text", || unsafe {
            given = f(ud, YogStr::from_str(item), YogStr::from_str(snbt), &mut out as *mut String as *mut c_void, copy_text);
        });
        if given {
            return Some(out);
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeItemText<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    item: JString<'l>,
    snbt: JString<'l>,
) -> jstring {
    let item: String = env.get_string(&item).map(String::from).unwrap_or_default();
    let snbt: String = env.get_string(&snbt).map(String::from).unwrap_or_default();
    match item_text(&item, &snbt) {
        Some(t) => env.new_string(t).map(JString::into_raw).unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}
