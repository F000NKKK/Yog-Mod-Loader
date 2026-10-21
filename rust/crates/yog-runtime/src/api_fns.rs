//! `YogApi`'s registration functions.

use yog_sync::Arc;

use yog_ids::dimension_field;

use yog_registry::{BlockDef, ItemDef};

use crate::handlers::RuntimeHandlers;
use crate::machine_api::{mod_loaded, translate};

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
pub(super) fn register_content(
    h: &mut RuntimeHandlers,
    kind: &str,
    id: String,
    value: &yog_data::ValueRef,
) {
    *h.content_blob.get_mut() = None;
    use yog_ids::kind;
    let (k, i, v) = (kind, id.as_str(), value);
    match kind {
        kind::ITEM => register_item(h, k, i, v),
        kind::BLOCK => register_block(h, k, i, v),
        kind::ENTITY => h.entities.extend(read(k, i, v)),
        kind::ATTRIBUTE => h.attributes.extend(read(k, i, v)),
        kind::EFFECT => h.effects.extend(read(k, i, v)),
        kind::POTION => h.potions.extend(read(k, i, v)),
        kind::BIOME_FEATURE => h.biome_features.extend(read(k, i, v)),
        kind::TOOL_MATERIAL => {
            if let Some(m) = read::<yog_registry::ToolMaterial>(k, i, v) {
                h.tool_materials.insert(m.id.clone(), m);
            }
        }
        kind::ARMOR_MATERIAL => {
            if let Some(m) = read::<yog_registry::ArmorMaterial>(k, i, v) {
                h.armor_materials.insert(m.id.clone(), m);
            }
        }
        kind::SOUND_GROUP => {
            if let Some(g) = read::<yog_registry::SoundGroup>(k, i, v) {
                h.sound_groups.insert(g.id.clone(), g);
            }
        }
        kind::INVENTORY => h.inventories.extend(read(k, i, v)),
        kind::STARTUP_GRANT => {
            if let Some(mut g) = read::<yog_registry::StartupGrant>(k, i, v) {
                none_if_empty(&mut [&mut g.book, &mut g.command]);
                h.startup_grants.push(g);
            }
        }
        kind::RECIPE => register_recipe(h, i, v),
        kind::FONT => {
            if let Some(font) = read::<yog_font::Font>(k, i, v) {
                crate::fonts::register(id, font);
            }
        }
        kind::BOOK => register_book(h, k, id, v),
        kind::DIMENSION => register_dimension(h, k, id, v),
        other => yog_logging::warn!(
            "unknown kind of content `{other}` ({id}) — this runtime doesn't know it"
        ),
    }
}

/// An item; an empty name or tooltip is none.
fn register_item(h: &mut RuntimeHandlers, kind: &str, id: &str, value: &yog_data::ValueRef) {
    if let Some(mut d) = read::<ItemDef>(kind, id, value) {
        none_if_empty(&mut [&mut d.name, &mut d.tooltip]);
        h.items.push(d);
    }
}

/// A block; its empty optional names are none.
fn register_block(h: &mut RuntimeHandlers, kind: &str, id: &str, value: &yog_data::ValueRef) {
    if let Some(mut d) = read::<BlockDef>(kind, id, value) {
        none_if_empty(&mut [
            &mut d.name,
            &mut d.sound,
            &mut d.inventory_id,
            &mut d.base,
            &mut d.set,
        ]);
        h.blocks.push(d);
    }
}

/// `v` read as `T`; its problems logged (`kind id: …`) when it won't read.
fn read<T: yog_data::Schema>(kind: &str, id: &str, v: &yog_data::ValueRef) -> Option<T> {
    match yog_data::read_ref_value::<T>(v) {
        Ok(value) => Some(value),
        Err(problems) => {
            let list: Vec<String> = problems
                .iter()
                .map(|p| format!("{}: {}", p.path, p.message))
                .collect();
            yog_logging::warn!("{kind} {id}: {}", list.join("; "));
            None
        }
    }
}

/// Each of `fields` that's empty, none.
fn none_if_empty(fields: &mut [&mut Option<String>]) {
    for s in fields {
        if s.as_deref() == Some("") {
            **s = None;
        }
    }
}

/// A recipe, fed to the books already registered too (their recipe pages).
fn register_recipe(h: &mut RuntimeHandlers, id: &str, value: &yog_data::ValueRef) {
    let (ns, name) = yog_ids::split(id);
    let recipe = value.to_value();
    for r in h.book_renderers.lock().values_mut() {
        r.add_recipe(yog_ids::join(ns, name), &recipe);
    }
    h.recipes.push((ns.to_string(), name.to_string(), recipe));
}

/// A book, read by its schema: the renderer draws it without a round trip
/// to Java. Entries about mods that aren't here aren't shown; i18n books
/// carry translation keys, shown in the game's language.
fn register_book(h: &mut RuntimeHandlers, kind: &str, id: String, value: &yog_data::ValueRef) {
    let Some(mut book) = read::<yog_formats::book::Book>(kind, &id, value) else {
        return;
    };
    book.entries
        .retain(|e| e.requires.iter().all(|m| mod_loaded(m)));
    h.books.insert(id.clone());
    let mut renderer = yog_book::BookRenderer::new(book);
    renderer.set_translator(translate);
    // Recipes may register before the book — replay them into the renderer.
    for (ns, name, recipe) in &h.recipes {
        renderer.add_recipe(yog_ids::join(ns, name), recipe);
    }
    h.book_renderers.get_mut().insert(id, renderer);
}

/// A dimension: its terrain stays here (the generator this one), the host
/// gets the rest, naming the generator.
fn register_dimension(h: &mut RuntimeHandlers, kind: &str, id: String, value: &yog_data::ValueRef) {
    yog_logging::info!("registered dimension type: {}", id);
    let mut def = value.to_value();
    if let Some(t) = value.get(dimension_field::TERRAIN) {
        let Some(terrain) = read::<yog_worldgen::Terrain>(kind, &id, t) else {
            return;
        };
        if let yog_data::Value::Map(m) = &mut def {
            m.remove(dimension_field::TERRAIN);
            m.insert(
                dimension_field::GENERATOR_TYPE,
                yog_data::Value::from(id.as_str()),
            );
        }
        h.terrains
            .insert(id.clone(), Arc::new(yog_worldgen::Generator::new(terrain)));
    }
    h.dimensions.insert(id, def);
}
