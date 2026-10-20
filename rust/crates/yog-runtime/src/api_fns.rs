//! `YogApi`'s registration functions.

use super::*;

// ── YogApi registration functions ─────────────────────────────────────────────
//
// ctx is *const RwLock<RuntimeHandlers> (cast from *mut c_void) — see
// `current_registration()` for how each entry gets tagged with the
// `LoadedModule` that's registering it.


/// The handlers a registration call writes into: `ctx` is the
/// `RwLock<RuntimeHandlers>` the runtime handed the mod's `yog_mod_register`.
/// A poisoned lock (a mod panicked mid-registration) is taken as it is —
/// one mod's panic doesn't stop the others registering.
///
/// # Safety
/// `ctx` must be that registration context.
/// Content of `kind` named `id`, its definition a Yog Data blob — what
/// the `content/register` service hands the runtime — stored in `h`.
pub(super) fn register_blob(h: &mut RuntimeHandlers, kind: &str, id: String, bytes: &[u8]) {
    // Borrowed: strings stay in `bytes`, only what's kept is copied.
    let arena = yog_data::Bump::new();
    match yog_data::binary::decode_ref(bytes, &arena) {
        Ok(value) => register_content(h, kind, id, &value),
        Err(e) => yog_logging::warn!("{kind} {id}: not Yog Data: {e}"),
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
        yog_ids::kind::FONT => {
            if let Some(font) = read::<yog_font::Font>(kind, &id, value) {
                crate::fonts::register(id, font);
            }
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




