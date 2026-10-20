//! UI system JNI — layer management, show/hide, hit-test, event dispatch.

use crate::{UiLayer, handlers};
use jni::{JNIEnv, objects::JClass, objects::JString, sys::*};

use crate::jcall::jcall;
use std::cell::Cell;
use yog_abi::YogStr;
use yog_sync::Locked;

/// Sends `ev` to the `register_ui` handler of `id`, if any (result ignored).
fn dispatch(id: &str, ev: &str) {
    let h = handlers();
    let Some((module, ud, handler)) = h.ui_handlers.get(id).cloned() else {
        return;
    };
    if module.is_retiring() {
        return;
    }
    if let Some(_call) = module.enter() {
        unsafe {
            handler(ud, YogStr::from_str(id), YogStr::from_str(ev));
        }
    }
}

/// A Yog UI screen (or inventory screen) opened: a new top layer, and
/// `open:W:H` for its handler (yog-kit's `on_open`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIShow<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    parent_id: JString<'l>,
    modal: jboolean,
    pause_game: jboolean,
    screen_w: jint,
    screen_h: jint,
) {
    let id = jstr!(env, ui_id);
    let parent = jstr!(env, parent_id);
    let parent = (!parent.is_empty()).then_some(parent);
    let h = handlers();
    let mut active = h.active_uis.locked();
    active.retain(|l| l.id != id);
    let layer = UiLayer {
        id: id.clone(),
        parent,
        modal: modal != 0,
        visible: true,
        z_index: active.iter().map(|l| l.z_index).max().unwrap_or(0) + 1,
    };
    active.push(layer);
    active.sort_by_key(|l| l.z_index);
    yog_logging::info!(
        "UI show: {} modal={} pause={} layers={}",
        id,
        modal != 0,
        pause_game != 0,
        active.len()
    );
    drop(active);
    dispatch(&id, &format!("open:{screen_w}:{screen_h}"));
}

/// A Yog UI screen closed: it and its child layers go, each handler gets
/// `close` (yog-kit's `on_close`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIHide<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    let mut active = h.active_uis.locked();
    let closed: Vec<String> = active
        .iter()
        .filter(|l| l.id == id || l.parent.as_deref() == Some(&id))
        .map(|l| l.id.clone())
        .collect();
    active.retain(|l| !closed.contains(&l.id));
    yog_logging::info!("UI hide: {} (layers left: {})", id, active.len());
    drop(active);
    for c in &closed {
        dispatch(c, "close");
    }
}

/// The cursor moved over a Yog UI screen — forwarded as `move:X:Y` (hover
/// states, wheel routing).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIMove<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
) {
    let id = jstr!(env, ui_id);
    dispatch(&id, &format!("move:{mx:.1}:{my:.1}"));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeIsUIActive<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
) -> jboolean {
    let id = jstr!(env, ui_id);
    let h = handlers();
    let active = h.active_uis.locked();
    active.iter().any(|l| l.id == id && l.visible) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIClick<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
    button: jint,
) -> jboolean {
    let id = jstr!(env, ui_id);
    let h = handlers();
    let active = h.active_uis.locked();
    // Find topmost modal layer that contains this click
    let top_modal = active
        .iter()
        .rev()
        .find(|l| l.visible && l.modal)
        .map(|l| l.id.clone());
    // Only dispatch if click is on the topmost modal layer (or there's no modal)
    if let Some(ref modal_id) = top_modal
        && id != *modal_id
    {
        return 0;
    }
    drop(active);

    // Try book click first: hit-test against the book renderer's last layout.
    // Only block generic handler if the book actually has a rendered UI (ui.is_some()).
    {
        let mut renderers = h.book_renderers.locked();
        if let Some(renderer) = renderers.get_mut(&id)
            && renderer.ui.is_some()
        {
            if let Some(ui) = &renderer.ui
                && let Some(hit) = yog_ui::layout::hit_test(&ui.layout_root, mx, my)
                && let Some(event) = &hit.on_click
            {
                let ev = event.clone();
                drop(renderers);
                if let Some(r) = h.book_renderers.locked().get_mut(&id) {
                    r.handle_event(&ev);
                }
                yog_logging::info!("book click '{}' → '{}'", &id, &ev);
                return 1;
            }
            return 1; // book UI consumed the click (no hit on widget, but in book area)
        }
        // ui=None (JSON parse failed) → fall through to generic handler
    }

    // Generic UI handler: raw click coords, hit-tested by the mod. A click
    // it doesn't use (on an inventory screen's slot, say) goes on to the game.
    let _ = button;
    dispatch_consumable(&id, &format!("click:{mx:.1}:{my:.1}")) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIScroll<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    dy: jfloat,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    // Forward as a generic UI event; the mod applies its own scroll offset.
    if let Some((module, ud, handler)) = h.ui_handlers.get(&id).cloned()
        && !module.is_retiring()
        && let Some(_call) = module.enter()
    {
        let ev = format!("scroll:{dy:.2}");
        unsafe {
            handler(ud, YogStr::from_str(&id), YogStr::from_str(&ev));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIDrag<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    if let Some((module, ud, handler)) = h.ui_handlers.get(&id).cloned()
        && !module.is_retiring()
        && let Some(_call) = module.enter()
    {
        let ev = format!("drag:{mx:.1}:{my:.1}");
        unsafe {
            handler(ud, YogStr::from_str(&id), YogStr::from_str(&ev));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRelease<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    if let Some((module, ud, handler)) = h.ui_handlers.get(&id).cloned()
        && !module.is_retiring()
        && let Some(_call) = module.enter()
    {
        let ev = format!("release:{mx:.1}:{my:.1}");
        unsafe {
            handler(ud, YogStr::from_str(&id), YogStr::from_str(&ev));
        }
    }
}

thread_local! {
    /// Set by `YogApi::ui_consume_event` while a `key:`/`char:` event is
    /// being dispatched — the mod used the key.
    static CONSUMED: Cell<bool> = const { Cell::new(false) };
    /// A UTF-16 high surrogate waiting for its low half (`charTyped` hands
    /// characters outside the BMP over as two calls).
    static HIGH_SURROGATE: Cell<Option<u16>> = const { Cell::new(None) };
}

/// `YogApi::ui_consume_event`: marks the event being dispatched as used.
pub(crate) fn consume_event() {
    CONSUMED.with(|c| c.set(true));
}

/// Sends `ev` to the `register_ui` handler of `id`; `true` when the handler
/// consumed it (called `ui_consume_event`).
fn dispatch_consumable(id: &str, ev: &str) -> bool {
    let h = handlers();
    let Some((module, ud, handler)) = h.ui_handlers.get(id).cloned() else {
        return false;
    };
    if module.is_retiring() {
        return false;
    }
    let Some(_call) = module.enter() else {
        return false;
    };
    let before = CONSUMED.with(|c| c.replace(false));
    unsafe {
        handler(ud, YogStr::from_str(id), YogStr::from_str(ev));
    }
    CONSUMED.with(|c| c.replace(before))
}

/// A key pressed (or repeating) over a Yog UI screen — forwarded as
/// `key:KEY:MODS` (GLFW key code and modifier bits). Returns whether the mod
/// consumed it; if not, the host does its default (Esc closes the screen).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIKey<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    key: jint,
    _scan: jint,
    mods: jint,
    action: jint,
) -> jboolean {
    let id = jstr!(env, ui_id);
    if action == 0 {
        return 0;
    }
    dispatch_consumable(&id, &format!("key:{key}:{mods}")) as jboolean
}

/// A character typed over a Yog UI screen (one UTF-16 unit, as
/// `Screen.charTyped` gets it) — forwarded as `char:CODEPOINT` once a whole
/// character is known. Returns whether the mod consumed it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIChar<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    chr: jchar,
    _mods: jint,
) -> jboolean {
    let id = jstr!(env, ui_id);
    let pending = HIGH_SURROGATE.with(std::cell::Cell::take);
    let c = match chr {
        0xD800..=0xDBFF => {
            HIGH_SURROGATE.with(|p| p.set(Some(chr)));
            return 1;
        }
        0xDC00..=0xDFFF => match pending {
            Some(hi) => char::decode_utf16([hi, chr])
                .next()
                .and_then(std::result::Result::ok),
            None => None,
        },
        _ => char::from_u32(chr as u32),
    };
    let Some(c) = c else { return 0 };
    dispatch_consumable(&id, &format!("char:{}", c as u32)) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRender<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    screen_w: jint,
    screen_h: jint,
) {
    let id = jstr!(env, ui_id);

    // Build a fresh snapshot of the currently-open inventory screen's slots
    // and push it into every loaded mod (see `yog_inventory::current_slot`)
    // — a plain Rust `static` in `yog-inventory` can't cross the cdylib
    // boundary between this runtime and a mod's own dynamic library, so each
    // mod gets the data via its own exported `yog_inventory_set_snapshot`
    // symbol instead, resolved the same way `yog_mod_register` is.
    // `slot_item_ids` and `snapshot` must outlive the `render_cbs` loop below
    // (not just this `if`) — mods read through the pointer we hand them via
    // `push_inventory_snapshot` for the rest of this function's duration.
    let n = (inv_slot_count(&mut env).max(0) as usize).min(yog_inventory::MAX_SNAPSHOT_SLOTS);
    let mut slot_item_ids: Vec<String> = Vec::with_capacity(n);
    let mut snapshot = yog_inventory::YogInvSnapshotRaw::EMPTY;
    if n > 0 {
        snapshot.slot_count = n as u32;
        for i in 0..n {
            let item = inv_slot_item(&mut env, i as i32);
            let pos = inv_slot_pos(&mut env, i as i32).unwrap_or((0, 0));
            slot_item_ids.push(item.as_ref().map(|(id, _)| id.clone()).unwrap_or_default());
            snapshot.slots[i].count = item.map(|(_, c)| c as u32).unwrap_or(0);
            snapshot.slots[i].x = pos.0;
            snapshot.slots[i].y = pos.1;
        }
        for (slot, id) in snapshot.slots.iter_mut().zip(&slot_item_ids) {
            slot.item_id = YogStr::from_str(id);
        }
        push_inventory_snapshot(&snapshot);
    }

    let h = crate::handlers();
    let mut gfx = crate::GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = 1.0;
    gfx.scale_factor = 1.0;
    let ctx = unsafe { yog_gfx::GfxContext::from_raw(&gfx as *const _) };
    let sw = screen_w as f32;
    let sh = screen_h as f32;

    // Book renderer path (books registered via register_book).
    // Only use if the book has a successfully parsed layout (ui.is_some()).
    {
        let mut book_renderers = h.book_renderers.locked();
        if let Some(book_renderer) = book_renderers.get_mut(&id)
            && book_renderer.ui.is_some()
        {
            // Pictures of `drawn` pages: the mod that registered the drawer draws them.
            let gfx_ptr = &gfx as *const _;
            book_renderer.render_with(&ctx, sw, sh, &mut |_, drawer, data, [x, y, w, bh]| {
                let Some((module, ud, f)) = h.page_drawers.get(drawer).cloned() else {
                    return;
                };
                if module.is_retiring() {
                    return;
                }
                let Some(_call) = module.enter() else {
                    return;
                };
                let bytes = yog_data::binary::encode(data);
                unsafe { f(ud, gfx_ptr, bytes.as_ptr(), bytes.len(), x, y, w, bh) };
            });
            return;
        }
        // ui=None → fall through to on_ui_render callbacks
    }

    // Generic on_ui_render callbacks — run AFTER renderBackground(), so they appear
    // on top of the screen darkening (unlike on_hud_render which runs before screens).
    let render_cbs: Vec<_> = h.ui_render_handlers.get(&id).cloned().unwrap_or_default();
    for (module, ud, f) in render_cbs {
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        unsafe { f(ud, &gfx as *const _) };
    }
}

/// Hand `snapshot` to every loaded mod's own copy of `yog-inventory`'s
/// module-level static, via each mod's exported `yog_inventory_set_snapshot`
/// symbol (present only in mods that depend on `yog-inventory`; missing on
/// others, which is fine — just skip them).
fn push_inventory_snapshot(snapshot: &yog_inventory::YogInvSnapshotRaw) {
    type SetSnapshotFn = unsafe extern "C" fn(*const yog_inventory::YogInvSnapshotRaw);
    for (_id, module) in crate::hot_reloader().all_active() {
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        module.with_library(|lib| unsafe {
            if let Ok(f) = lib.get::<SetSnapshotFn>(b"yog_inventory_set_snapshot") {
                f(snapshot as *const _);
            }
        });
    }
}

// ── Inventory slot queries (Rust → Java, called during yog-ui render) ──────

/// Number of slots in the currently open inventory screen (0 if none).
pub fn inv_slot_count(env: &mut JNIEnv) -> i32 {
    unsafe { jcall!(env, "dev/yog/NativeBridge", "getSlotCount", []) }.unwrap_or(0)
}

/// Query one slot: returns `Some((item_id, count))` or `None` if empty.
/// `item_id` is a namespaced string like `"minecraft:diamond"`.
pub fn inv_slot_item(env: &mut JNIEnv, index: i32) -> Option<(String, i32)> {
    let s: String = unsafe { jcall!(env, "dev/yog/NativeBridge", "getSlotItem", [I(index)]) }?;
    let (id, count) = s.split_once('\t')?;
    Some((id.to_owned(), count.parse().ok()?))
}

/// Pixel position of a slot (relative to screen, already including guiLeft/guiTop).
pub fn inv_slot_pos(env: &mut JNIEnv, index: i32) -> Option<(i32, i32)> {
    let s: String = unsafe { jcall!(env, "dev/yog/NativeBridge", "getSlotPos", [I(index)]) }?;
    let [x, y] = crate::parse_tsv(&s)?;
    Some((x, y))
}

/// Return tab+newline-encoded menu entries: `label\tui_id\n...`
/// Java side parses this and injects buttons into vanilla screens.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeMenuEntries<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let h = handlers();
    let lines: Vec<String> = h
        .menu_entries
        .iter()
        .map(|(label, ui_id)| format!("{label}\t{ui_id}"))
        .collect();
    let out = lines.join("\n");
    env.new_string(out).map_or(std::ptr::null_mut(), |s| s.into_raw())
}

/// The mods' key bindings (ABI 3.10), one a line: `id\tname key\tdefault key\tcategory label`
/// (the label: the mod's name, its namespace's when no mod has it). The host
/// registers each in the game's controls at the client's start.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeKeyBindings<'l>(env: JNIEnv<'l>, _class: JClass<'l>) -> jstring {
    let lines: Vec<String> = crate::key_bindings()
        .into_iter()
        .map(|(id, name, key, label)| format!("{id}\t{name}\t{key}\t{label}"))
        .collect();
    env.new_string(lines.join("\n")).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

// ── Pictures in item tooltips (ABI 2.2) ─────────────────────────────────────

/// The mod's picture for a stack of `item` (custom data `snbt`): which
/// entry, and its size.
fn tooltip_of(h: &crate::RuntimeHandlers, item: &str, snbt: &str) -> Option<(crate::TooltipEntry, f32, f32)> {
    for entry in h.item_tooltips.get(item)? {
        let (module, ud, size, _) = entry.clone();
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        let (mut w, mut hh) = (0.0f32, 0.0f32);
        if unsafe { size(ud, YogStr::from_str(item), YogStr::from_str(snbt), &mut w, &mut hh) } && w > 0.0 && hh > 0.0 {
            return Some((entry.clone(), w, hh));
        }
    }
    None
}

/// `float[]{w, h}` of a stack's tooltip picture, or null for none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTooltipSize<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    item: JString<'l>,
    snbt: JString<'l>,
) -> jfloatArray {
    let item = jstr!(env, item, std::ptr::null_mut());
    let snbt = jstr!(env, snbt, std::ptr::null_mut());
    let h = handlers();
    let Some((_, w, hh)) = tooltip_of(&h, &item, &snbt) else {
        return std::ptr::null_mut();
    };
    let Ok(arr) = env.new_float_array(2) else {
        return std::ptr::null_mut();
    };
    let _ = env.set_float_array_region(&arr, 0, &[w, hh]);
    arr.into_raw()
}

/// Draws a stack's tooltip picture in `x, y, w, h` (GUI pixels).
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTooltipDraw<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    item: JString<'l>,
    snbt: JString<'l>,
    x: jint,
    y: jint,
    w: jint,
    hh: jint,
    screen_w: jint,
    screen_h: jint,
) {
    let item = jstr!(env, item);
    let snbt = jstr!(env, snbt);
    let h = handlers();
    let Some(((module, ud, _, draw), ..)) = tooltip_of(&h, &item, &snbt) else {
        return;
    };
    let Some(_call) = module.enter() else {
        return;
    };
    let mut gfx = crate::GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = 1.0;
    gfx.scale_factor = 1.0;
    unsafe { draw(ud, &gfx as *const _, YogStr::from_str(&item), YogStr::from_str(&snbt), x as f32, y as f32, w as f32, hh as f32) };
}
