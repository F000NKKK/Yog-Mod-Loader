//! UI system JNI — layer management, show/hide, hit-test, event dispatch.
//! Replace the // ── UI system JNI ── section in lib.rs with this file.
//! Then add `mod ui_jni;` to lib.rs.

use crate::{handlers, UiLayer};
use jni::{JNIEnv, objects::JClass, objects::JString, sys::*};
use yog_abi::YogStr;

macro_rules! jstr {
    ($env:expr, $s:expr) => {
        match $env.get_string(&$s) { Ok(s) => String::from(s), Err(_) => return Default::default() }
    };
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIShow<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>,
    parent_id: JString<'l>, modal: jboolean, pause_game: jboolean, _w: jint, _h: jint,
) {
    let id = jstr!(env, ui_id);
    let parent = jstr!(env, parent_id);
    let parent = if parent.is_empty() { None } else { Some(parent) };
    let mut active = handlers().active_uis.lock().expect("active_uis");
    active.retain(|l| l.id != id);
    let layer = UiLayer {
        id: id.clone(), parent, modal: modal != 0, pause_game: pause_game != 0,
        visible: true, enabled: true,
        z_index: active.iter().map(|l| l.z_index).max().unwrap_or(0) + 1,
    };
    active.push(layer);
    active.sort_by_key(|l| l.z_index);
    yog_logging::info!("UI show: {} modal={} pause={} layers={}", id, modal != 0, pause_game != 0, active.len());
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIHide<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>,
) {
    let id = jstr!(env, ui_id);
    let mut active = handlers().active_uis.lock().expect("active_uis");
    // Remove layer and all children
    let ids_to_remove: Vec<String> = active.iter()
        .filter(|l| l.id == id || l.parent.as_deref() == Some(&id))
        .map(|l| l.id.clone()).collect();
    active.retain(|l| !ids_to_remove.contains(&l.id));
    yog_logging::info!("UI hide: {} (layers left: {})", id, active.len());
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeIsUIActive<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>,
) -> jboolean {
    let id = jstr!(env, ui_id);
    let active = handlers().active_uis.lock().expect("active_uis");
    active.iter().any(|l| l.id == id && l.visible) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIClick<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    ui_id: JString<'l>, mx: jfloat, my: jfloat, button: jint,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    let active = h.active_uis.lock().expect("active_uis");
    // Find topmost modal layer that contains this click
    let top_modal = active.iter().rev()
        .find(|l| l.visible && l.modal)
        .map(|l| l.id.clone());
    // Only dispatch if click is on the topmost modal layer (or there's no modal)
    if let Some(ref modal_id) = top_modal {
        if id != *modal_id { return; }
    }
    drop(active);

    // Try book click first: hit-test against the book renderer's last layout.
    {
        let mut renderers = h.book_renderers.lock().expect("book_renderers");
        if let Some(renderer) = renderers.get_mut(&id) {
            if let Some(ui) = &renderer.ui {
                if let Some(hit) = yog_ui::layout::hit_test(&ui.layout_root, mx, my) {
                    if let Some(event) = &hit.on_click {
                        yog_logging::info!("book click '{}' → '{}'", id, event);
                        let ev = event.clone();
                        drop(renderers);
                        h.book_renderers.lock().unwrap().get_mut(&id)
                            .map(|r| r.handle_event(&ev));
                        return;
                    }
                }
            }
            return; // book UI consumed the click
        }
    }

    // Generic UI handler.
    if let Some((ud, handler)) = h.ui_handlers.get(&id).copied() {
        if let Some(ui_root) = h.uis.get(&id) {
            if let Some(hit) = yog_ui::layout::hit_test(ui_root, mx, my) {
                if let Some(event) = &hit.on_click {
                    yog_logging::info!("UI click '{}' → '{}'", id, event);
                    unsafe { handler(ud, YogStr::from_str(&id), YogStr::from_str(event)); }
                }
            }
        }
    }
    let _ = button;
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIKey<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    ui_id: JString<'l>, key: jint, _scan: jint, _mods: jint, action: jint,
) {
    let id = jstr!(env, ui_id);
    yog_logging::info!("UI key: {} key={} action={}", id, key, action);
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRender<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    ui_id: JString<'l>, screen_w: jint, screen_h: jint,
) {
    let id = jstr!(env, ui_id);
    let h = crate::handlers();
    // Build a GfxContext using GFX_FN_TABLE function pointers with current screen dims.
    let mut gfx = crate::GFX_FN_TABLE;
    gfx.screen_w    = screen_w;
    gfx.screen_h    = screen_h;
    gfx.delta_tick  = 1.0;
    gfx.scale_factor = 1.0;
    let ctx = unsafe { yog_gfx::GfxContext::from_raw(&gfx as *const _) };
    let sw = screen_w as f32;
    let sh = screen_h as f32;
    let fonts = h.book_fonts.lock().expect("book_fonts");
    let mut book_renderers = h.book_renderers.lock().expect("book_renderers");
    if let Some(book_renderer) = book_renderers.get_mut(&id) {
        book_renderer.render(&ctx, sw, sh, &fonts);
    }
}
