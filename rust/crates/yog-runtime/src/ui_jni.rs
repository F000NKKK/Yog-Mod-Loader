//! UI system JNI — layer management, show/hide, hit-test, event dispatch.

use crate::handlers::{UiLayer, handlers};
use jni::{JNIEnv, objects::JClass, objects::JString, sys::*};

use std::cell::Cell;

/// Sends `ev` to screen `id`'s input handlers; `true` when one used it (cancelled it).
fn dispatch(id: &str, ev: &str) -> bool {
    !crate::bus::allowed(&yog_event::client::ui::Input {
        ui_id: id.to_owned(),
        event: ev.to_owned(),
    })
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
    crate::jni_util::guard("nativeUIShow", || {
        let id = jstr!(env, ui_id);
        let parent = jstr!(env, parent_id);
        let parent = (!parent.is_empty()).then_some(parent);
        let h = handlers();
        let mut active = h.active_uis.lock();
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
    });
}

/// A Yog UI screen closed: it and its child layers go, each handler gets
/// `close` (yog-kit's `on_close`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIHide<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
) {
    crate::jni_util::guard("nativeUIHide", || {
        let id = jstr!(env, ui_id);
        let h = handlers();
        let mut active = h.active_uis.lock();
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
    });
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
    crate::jni_util::guard("nativeUIMove", || {
        let id = jstr!(env, ui_id);
        dispatch(&id, &format!("move:{mx:.1}:{my:.1}"));
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeIsUIActive<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
) -> jboolean {
    crate::jni_util::guarded("nativeIsUIActive", 0, || {
        let id = jstr!(env, ui_id);
        let h = handlers();
        let active = h.active_uis.lock();
        active.iter().any(|l| l.id == id && l.visible) as jboolean
    })
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
    crate::jni_util::guarded("nativeUIClick", 0, || {
        let id = jstr!(env, ui_id);
        let h = handlers();
        let active = h.active_uis.lock();
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
            let mut renderers = h.book_renderers.lock();
            if let Some(renderer) = renderers.get_mut(&id)
                && renderer.ui.is_some()
            {
                if let Some(ui) = &renderer.ui
                    && let Some(hit) = yog_ui::layout::hit_test(&ui.layout_root, mx, my)
                    && let Some(event) = &hit.on_click
                {
                    let ev = event.clone();
                    drop(renderers);
                    if let Some(r) = h.book_renderers.lock().get_mut(&id) {
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
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIScroll<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    dy: jfloat,
) {
    crate::jni_util::guard("nativeUIScroll", || {
        let id = jstr!(env, ui_id);
        dispatch(&id, &format!("scroll:{dy:.2}"));
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIDrag<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
) {
    crate::jni_util::guard("nativeUIDrag", || {
        let id = jstr!(env, ui_id);
        dispatch(&id, &format!("drag:{mx:.1}:{my:.1}"));
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRelease<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    mx: jfloat,
    my: jfloat,
) {
    crate::jni_util::guard("nativeUIRelease", || {
        let id = jstr!(env, ui_id);
        dispatch(&id, &format!("release:{mx:.1}:{my:.1}"));
    });
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
    crate::jni_util::guarded("nativeUIKey", 0, || {
        let id = jstr!(env, ui_id);
        if action == 0 {
            return 0;
        }
        dispatch(&id, &format!("key:{key}:{mods}")) as jboolean
    })
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
    crate::jni_util::guarded("nativeUIChar", 0, || {
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
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRender<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ui_id: JString<'l>,
    screen_w: jint,
    screen_h: jint,
) {
    crate::jni_util::guard("nativeUIRender", || {
        let id = jstr!(env, ui_id);

        let h = crate::handlers::handlers();
        let mut gfx = crate::gfx_table::GFX_FN_TABLE;
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
            let mut book_renderers = h.book_renderers.lock();
            if let Some(book_renderer) = book_renderers.get_mut(&id)
                && book_renderer.ui.is_some()
            {
                // Pictures of `drawn` pages: the mod that registered the drawer draws them.
                book_renderer.render_with(&ctx, sw, sh, &mut |_, drawer, data, rect| {
                    let page = yog_event::client::render::Page {
                        drawer: drawer.to_owned(),
                        data: data.clone(),
                        rect,
                    };
                    crate::gfx_frame::drawing(&gfx, || crate::bus::emitted(&page));
                });
                return;
            }
            // ui=None → fall through to on_ui_render callbacks
        }

        // The screen's own drawing — after renderBackground(), so on top of the
        // darkening (unlike the HUD, drawn before screens).
        crate::gfx_frame::drawing(&gfx, || {
            crate::bus::emitted(&yog_event::client::render::Ui { ui_id: id.clone() })
        });
    });
}

// ── Pictures in item tooltips ─────────────────────────────────────

/// Draws a stack's tooltip picture in `x, y, w, h` (GUI pixels).
#[unsafe(no_mangle)]
// The native method's signature is Java's: one parameter per argument.
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
    crate::jni_util::guard("nativeTooltipDraw", || {
        let item = jstr!(env, item);
        let snbt = jstr!(env, snbt);
        let mut gfx = crate::gfx_table::GFX_FN_TABLE;
        gfx.screen_w = screen_w;
        gfx.screen_h = screen_h;
        gfx.delta_tick = 1.0;
        gfx.scale_factor = 1.0;
        let tooltip = yog_event::client::render::Tooltip {
            item,
            snbt,
            rect: [x as f32, y as f32, w as f32, hh as f32],
        };
        crate::gfx_frame::drawing(&gfx, || crate::bus::emitted(&tooltip));
    });
}
