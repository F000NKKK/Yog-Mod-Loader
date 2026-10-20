//! Machines (ABI 2.5).

use crate::globals::HOT_RELOADER;
use crate::handlers::handlers;
use crate::jni_util::guard;
use crate::machine;
use jni::JNIEnv;
use jni::objects::JClass;
use jni::objects::JString;
use jni::sys::jint;
use jni::sys::jstring;
use std::sync::RwLock;
use yog_abi::YogStr;
use yog_core::BlockPos;
use yog_event::container::machine::Craft;

// ── Machines (ABI 2.5) ───────────────────────────────────────────────────────

/// A machine's event about recipe `found`.
pub(super) fn machine_event(
    m: &yog_inventory::Machine,
    def: &str,
    found: &machine::Found,
    dim: &str,
    pos: BlockPos,
    player: &str,
) -> Craft {
    Craft {
        machine: def.to_owned(),
        kind: m.kind.clone(),
        recipe: found.recipe.clone(),
        result_item: found.result.clone(),
        result_count: found.count,
        values: found.values.clone(),
        pos,
        dimension: dim.to_owned(),
        player: player.to_owned(),
    }
}

/// The recipe machine `def` at `pos` makes of `items` (by inventory slot,
/// `""` empty): the first that fits and every mod allows; `recipe` only it.
pub(super) fn machine_recipe(
    def: &str,
    dim: &str,
    pos: BlockPos,
    items: &[String],
    recipe: Option<&str>,
) -> Option<machine::Found> {
    let h = handlers();
    let m = h
        .inventories
        .iter()
        .find(|d| d.id == def)?
        .machine
        .as_ref()?;
    let has_tag = |item: &str, tag: &str| {
        crate::bus::call(&yog_event::tag::HasItem {
            item: item.to_owned(),
            tag: tag.to_owned(),
        })
        .unwrap_or(false)
    };
    let recipes = h
        .recipes
        .iter()
        .map(|(ns, name, r)| (format!("{ns}:{name}"), r))
        .filter(|(id, _)| recipe.is_none_or(|r| r == id));
    machine::find(m, recipes, items, &has_tag)
        .find(|found| crate::bus::allowed(&machine_event(m, def, found, dim, pos, "")))
}

/// What machine `def` at `x y z` makes of `items` (`\n`-joined, by slot):
/// `recipe\nresult\ncount`, or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeMachineResult<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    def: JString<'l>,
    dimension: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    items: JString<'l>,
) -> jstring {
    let (def, d, items) = (
        jstr!(env, def, std::ptr::null_mut()),
        jstr!(env, dimension, std::ptr::null_mut()),
        jstr!(env, items, std::ptr::null_mut()),
    );
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let mut found = None;
    guard("machine_result", || {
        found = machine_recipe(&def, &d, BlockPos { x, y, z }, &items, None)
    });
    match found {
        Some(f) => env
            .new_string(format!("{}\n{}\n{}", f.recipe, f.result, f.count))
            .map(JString::into_raw)
            .unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// `player` took machine `def`'s result, made by `recipe` of `items` (the
/// ingredients still in): whether it may — then the mods take their share.
#[unsafe(no_mangle)]
// The native method's signature is Java's: one parameter per argument.
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeMachineCraft<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    def: JString<'l>,
    recipe: JString<'l>,
    dimension: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    items: JString<'l>,
    player: JString<'l>,
) -> jni::sys::jboolean {
    let (def, recipe, d, items, player) = (
        jstr!(env, def, 0),
        jstr!(env, recipe, 0),
        jstr!(env, dimension, 0),
        jstr!(env, items, 0),
        jstr!(env, player, 0),
    );
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let pos = BlockPos { x, y, z };
    let mut made = false;
    guard("machine_craft", || {
        let Some(found) = machine_recipe(&def, &d, pos, &items, Some(&recipe)) else {
            return;
        };
        let h = handlers();
        let Some(m) = h
            .inventories
            .iter()
            .find(|i| i.id == def)
            .and_then(|i| i.machine.as_ref())
        else {
            return;
        };
        crate::bus::emitted(&machine_event(m, &def, &found, &d, pos, &player));
        made = true;
    });
    made as jni::sys::jboolean
}

/// The Yog mods being loaded, by id (all of them, before the first registers).
pub(super) static YOG_MOD_IDS: RwLock<std::collections::BTreeSet<String>> =
    RwLock::new(std::collections::BTreeSet::new());

/// Whether mod `id` is loaded: a Yog mod, else one in the host's list
/// (Fabric's, Forge's, NeoForge's).
pub(super) fn mod_loaded(id: &str) -> bool {
    YOG_MOD_IDS.read().unwrap_or_else(|e| e.into_inner()).contains(id)
        || HOT_RELOADER.all_active().iter().any(|(m, _)| m == id)
        // SAFETY: a host call with a string argument, on the calling thread.
        || unsafe { bridge!("isModLoaded", [S(YogStr::from_str(id))]) }.unwrap_or(false)
}

/// The game's text for translation `key` in the current language — `None`
/// when no lang file has it (or there's no JVM).
pub(super) fn translate(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    unsafe { bridge!("translate", [S(YogStr::from_str(key))]) }
}
