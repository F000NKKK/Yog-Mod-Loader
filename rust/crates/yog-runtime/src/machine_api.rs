//! Machines (ABI 2.5).

use super::*;
use yog_core::BlockPos;
use yog_event::container::machine::Craft;

// ── Machines (ABI 2.5) ───────────────────────────────────────────────────────

/// A machine's event about recipe `found`.
pub(super) fn machine_event(
    m: &yog_inventory::Machine,
    def: &str,
    found: &machine::Found,
    dim: &str,
    pos: YogBlockPos,
    player: &str,
) -> Craft {
    Craft {
        machine: def.to_owned(),
        kind: m.kind.clone(),
        recipe: found.recipe.clone(),
        result_item: found.result.clone(),
        result_count: found.count,
        values: found.values.clone(),
        pos: BlockPos { x: pos.x, y: pos.y, z: pos.z },
        dimension: dim.to_owned(),
        player: player.to_owned(),
    }
}

/// The recipe machine `def` at `pos` makes of `items` (by inventory slot,
/// `""` empty): the first that fits and every mod allows; `recipe` only it.
pub(super) fn machine_recipe(def: &str, dim: &str, pos: YogBlockPos, items: &[String], recipe: Option<&str>) -> Option<machine::Found> {
    let h = handlers();
    let m = h.inventories.iter().find(|d| d.id == def)?.machine.as_ref()?;
    let has_tag = |item: &str, tag: &str| unsafe { srv_has_item_tag(std::ptr::null_mut(), YogStr::from_str(item), YogStr::from_str(tag)) };
    let recipes = h.recipes.iter().map(|(ns, name, r)| (format!("{ns}:{name}"), r)).filter(|(id, _)| recipe.is_none_or(|r| r == id));
    machine::find(m, recipes, items, &has_tag).find(|found| crate::bus::allowed(&machine_event(m, def, found, dim, pos, "")))
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
    let (def, d, items) = (jstr!(env, def, std::ptr::null_mut()), jstr!(env, dimension, std::ptr::null_mut()), jstr!(env, items, std::ptr::null_mut()));
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let mut found = None;
    guard("machine_result", || found = machine_recipe(&def, &d, YogBlockPos { x, y, z }, &items, None));
    match found {
        Some(f) => env.new_string(format!("{}\n{}\n{}", f.recipe, f.result, f.count)).map(JString::into_raw).unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// `player` took machine `def`'s result, made by `recipe` of `items` (the
/// ingredients still in): whether it may — then the mods take their share.
#[unsafe(no_mangle)]
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
    let (def, recipe, d, items, player) =
        (jstr!(env, def, 0), jstr!(env, recipe, 0), jstr!(env, dimension, 0), jstr!(env, items, 0), jstr!(env, player, 0));
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let pos = YogBlockPos { x, y, z };
    let mut made = false;
    guard("machine_craft", || {
        let Some(found) = machine_recipe(&def, &d, pos, &items, Some(&recipe)) else { return };
        let h = handlers();
        let Some(m) = h.inventories.iter().find(|i| i.id == def).and_then(|i| i.machine.as_ref()) else { return };
        crate::bus::emitted(&machine_event(m, &def, &found, &d, pos, &player));
        made = true;
    });
    made as jni::sys::jboolean
}

/// The Yog mods being loaded, by id (all of them, before the first registers).
pub(super) static YOG_MOD_IDS: RwLock<std::collections::BTreeSet<String>> = RwLock::new(std::collections::BTreeSet::new());

/// Whether mod `id` is loaded: a Yog mod, else one in the host's list
/// (Fabric's, Forge's, NeoForge's).
pub(super) fn mod_loaded(id: &str) -> bool {
    YOG_MOD_IDS.read().unwrap_or_else(|e| e.into_inner()).contains(id)
        || HOT_RELOADER.get().is_some_and(|r| r.all_active().iter().any(|(m, _)| m == id))
        // SAFETY: a host call with a string argument, on the calling thread.
        || unsafe { bridge!("isModLoaded", [JArg::S(YogStr::from_str(id))]) }.unwrap_or(false)
}

pub(super) unsafe extern "C" fn api_is_mod_loaded(_ctx: *mut c_void, id: YogStr) -> bool {
    unsafe { mod_loaded(id.as_str()) }
}

pub(super) unsafe extern "C" fn api_java_call(_ctx: *mut c_void, expr: YogStr, args: YogStr) -> YogOwnedStr {
    unsafe {
        let args = match yog_data::text::parse_value(args.as_str()) {
            Ok(v) => v,
            Err(e) => {
                yog_logging::warn!("java_call {}: its arguments aren't Yog Data: {}", expr.as_str(), e.message);
                return YogOwnedStr::NONE;
            }
        };
        let bytes = yog_data::binary::encode(&args);
        let out: Option<Vec<u8>> = host!("dev/yog/data/YogJava", "call", [JArg::S(expr), JArg::Bytes(&bytes)]);
        match out.map(|b| yog_data::binary::decode(&b)) {
            Some(Ok(v)) => YogOwnedStr::from_string(yog_data::text::to_string(&v)),
            _ => YogOwnedStr::NONE,
        }
    }
}

/// Strip TSV separators so one mod can't corrupt the line format.
pub(super) fn tsv_clean(v: &str) -> String {
    v.replace(['\t', '\n', '\r'], " ")
}

pub(super) unsafe extern "C" fn api_ui_open(_ctx: *mut c_void, ui_id: YogStr, modal: bool, pause: bool) {
    unsafe {
        if ui_id.as_str().is_empty() {
            return;
        }
        // Java side schedules onto the render thread; no-op on dedicated servers.
        let _: Option<()> = bridge!("openUI", [S(ui_id), Z(modal), Z(pause)]);
    }
}

pub(super) unsafe extern "C" fn api_ui_close(_ctx: *mut c_void, ui_id: YogStr) {
    unsafe {
        // Java side defers onto the render thread (after the current event) and
        // checks the open screen is that Yog UI; no-op on dedicated servers.
        let _: Option<()> = bridge!("closeUI", [S(ui_id)]);
    }
}

pub(super) unsafe extern "C" fn api_translate(_ctx: *mut c_void, key: YogStr) -> YogOwnedStr {
    unsafe { translate(key.as_str()).map_or(YogOwnedStr::NONE, YogOwnedStr::from_string) }
}

/// The game's text for translation `key` in the current language — `None`
/// when no lang file has it (or there's no JVM).
pub(super) fn translate(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    unsafe { bridge!("translate", [S(YogStr::from_str(key))]) }
}

pub(super) unsafe extern "C" fn api_mods_list(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe {
        let mut lines: Vec<String> = Vec::new();
        let infos = MOD_INFOS.locked().clone();
        for m in &infos {
            // A `.yres` name/description shows in the player's language.
            let localized = |text: &str, key: &str| {
                translate(key).map_or_else(|| text.to_string(), |t| tsv_clean(&t))
            };
            lines.push(format!(
                "yog\t{}\t{}\t{}\t{}\t{}",
                m[0],
                localized(&m[1], &m[5]),
                m[2],
                m[3],
                localized(&m[4], &m[6])
            ));
        }
        // Loader-side (Java) mods, if the host provides them.
        let platform: Option<String> = bridge!("listPlatformMods", []);
        for line in platform.iter().flat_map(|s| s.lines()) {
            if !line.trim().is_empty() {
                lines.push(format!("platform\t{line}"));
            }
        }
        YogOwnedStr::from_string(lines.join("\n"))
    }
}

