//! UI system JNI — layer management, show/hide, hit-test, event dispatch.

use crate::{UiLayer, handlers};
use jni::{JNIEnv, objects::JClass, objects::JString, sys::*};

use std::cell::Cell;
use yog_abi::YogStr;
use yog_sync::Locked;

/// Sends `ev` to screen `id`'s input handlers; `true` when one used it (cancelled it).
fn dispatch(id: &str, ev: &str) -> bool {
    !crate::bus::allowed(&yog_event::client::ui::Input { ui_id: id.to_owned(), event: ev.to_owned() })
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
    dispatch(&id, &format!("click:{mx:.1}:{my:.1}")) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIScroll<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    dy: jfloat,
) {
    let id = jstr!(env, ui_id);
    dispatch(&id, &format!("scroll:{dy:.2}"));
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
    dispatch(&id, &format!("drag:{mx:.1}:{my:.1}"));
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
    dispatch(&id, &format!("release:{mx:.1}:{my:.1}"));
}

thread_local! {
    /// A UTF-16 high surrogate waiting for its low half (`charTyped` hands
    /// characters outside the BMP over as two calls).
    static HIGH_SURROGATE: Cell<Option<u16>> = const { Cell::new(None) };
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
    dispatch(&id, &format!("key:{key}:{mods}")) as jboolean
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
    dispatch(&id, &format!("char:{}", c as u32)) as jboolean
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
    let slots = crate::bus::call(&yog_event::client::ScreenSlots {}).unwrap_or_default();
    let n = slots.len().min(yog_inventory::MAX_SNAPSHOT_SLOTS);
    // Atoms' names live as long as the process: the pointers stay good.
    let slot_item_ids: Vec<&'static str> = slots.iter().take(n).map(|s| s.stack.as_ref().map_or("", |st| st.item.as_str())).collect();
    let mut snapshot = yog_inventory::YogInvSnapshotRaw::EMPTY;
    if n > 0 {
        snapshot.slot_count = n as u32;
        for ((raw, slot), id) in snapshot.slots.iter_mut().zip(&slots).zip(&slot_item_ids) {
            raw.count = slot.stack.as_ref().map_or(0, |st| st.count);
            raw.x = slot.x;
            raw.y = slot.y;
            raw.item_id = YogStr::from_str(id);
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
            book_renderer.render_with(&ctx, sw, sh, &mut |_, drawer, data, rect| {
                let page = yog_event::client::render::Page { drawer: drawer.to_owned(), data: data.clone(), rect };
                crate::gfx_frame::drawing(&gfx, || crate::bus::emitted(&page));
            });
            return;
        }
        // ui=None → fall through to on_ui_render callbacks
    }

    // The screen's own drawing — after renderBackground(), so on top of the
    // darkening (unlike the HUD, drawn before screens).
    crate::gfx_frame::drawing(&gfx, || crate::bus::emitted(&yog_event::client::render::Ui { ui_id: id.clone() }));
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

// ── Pictures in item tooltips ─────────────────────────────────────

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
    let mut gfx = crate::GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = 1.0;
    gfx.scale_factor = 1.0;
    let tooltip = yog_event::client::render::Tooltip { item, snbt, rect: [x as f32, y as f32, w as f32, hh as f32] };
    crate::gfx_frame::drawing(&gfx, || crate::bus::emitted(&tooltip));
}
