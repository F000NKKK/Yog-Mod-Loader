//! JNI: client-side entry points (ABI minor 10).

use super::*;

// ── ABI minor 10 — client-side JNI entry points ───────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientTick<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let h = handlers();
    if h.client_tick.is_empty() {
        return;
    }
    guard("on_client_tick", || {
        each(&h.client_tick, |ud, f| unsafe { f(ud) });
    });
}

/// The host's GL function pointer for `sym` (0 if it has none).
pub(super) fn gl_proc_address(sym: &str) -> usize {
    let ptr: Option<i64> = unsafe { bridge!("glProcAddress", [S(YogStr::from_str(sym))]) };
    ptr.unwrap_or(0) as usize
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGlInit<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    if GL.get().is_some() {
        return;
    }
    let mut raw_get_binary: usize = 0;
    let mut raw_prog_binary: usize = 0;
    let gl = unsafe {
        glow::Context::from_loader_function(|sym| {
            let ptr = gl_proc_address(sym);
            // Capture extension pointers while the loader runs.
            match sym {
                "glGetProgramBinary" => raw_get_binary = ptr,
                "glProgramBinary" => raw_prog_binary = ptr,
                _ => {}
            }
            ptr as *const _
        })
    };
    let _ = GL.set(GlCtx(gl));
    let _ = GL_GET_PROGRAM_BINARY.set((raw_get_binary != 0).then_some(raw_get_binary));
    let _ = GL_PROGRAM_BINARY.set((raw_prog_binary != 0).then_some(raw_prog_binary));
    // `glGetProgramiv` is a core function; always available.  We look it up once here
    // to avoid depending on glow internals for the PROGRAM_BINARY_LENGTH query.
    let get_program_iv = gl_proc_address("glGetProgramiv");
    let _ = GL_GET_PROGRAM_IV.set((get_program_iv != 0).then_some(get_program_iv));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnHudRender<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    player_x: jfloat,
    player_y: jfloat,
    player_z: jfloat,
) {
    let h = handlers();
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.player_pos = [player_x, player_y, player_z];

    // Mod hud_render callbacks.
    if !h.hud_render.is_empty() {
        guard("on_hud_render", || {
            each(&h.hud_render, |ud, f| unsafe { f(ud, &gfx) });
        });
    }

    // Render active book UIs.
    let active_layers: Vec<UiLayer> = h.active_uis.locked().clone();
    let sw = screen_w as f32;
    let sh = screen_h as f32;
    let ctx = unsafe { yog_gfx::GfxContext::from_raw(&gfx as *const _) };
    let mut renderers = h.book_renderers.locked();
    for layer in &active_layers {
        if !layer.visible {
            continue;
        }
        if let Some(renderer) = renderers.get_mut(&layer.id) {
            renderer.render(&ctx, sw, sh);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnWorldRender<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    view_proj_arr: JFloatArray<'l>,
    cam_x: jfloat,
    cam_y: jfloat,
    cam_z: jfloat,
    player_x: jfloat,
    player_y: jfloat,
    player_z: jfloat,
) {
    let h = handlers();
    if h.world_render.is_empty() {
        return;
    }
    let mut view_proj = [0f32; 16];
    if env
        .get_float_array_region(&view_proj_arr, 0, &mut view_proj)
        .is_err()
    {
        return;
    }
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.view_proj = view_proj;
    gfx.camera_pos = [cam_x, cam_y, cam_z];
    gfx.player_pos = [player_x, player_y, player_z];
    guard("on_world_render", || {
        each(&h.world_render, |ud, f| unsafe { f(ud, &gfx) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnKeyPress<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    key_code: jint,
    scan_code: jint,
    action: jint,
    modifiers: jint,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.key_press.is_empty() {
        return 1;
    }
    let ev = YogKeyPressEvent {
        key_code,
        scan_code,
        action,
        modifiers,
    };
    allowed(&h.key_press, |ud, f| unsafe { f(ud, &ev) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenOpen<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_open.is_empty() {
        return;
    }
    let sc = match env.get_string(&screen_class) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    guard("on_screen_open", || {
        each(&h.screen_open, |ud, f| unsafe {
            f(ud, YogStr::from_str(&sc))
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenClose<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_close.is_empty() {
        return;
    }
    let sc = match env.get_string(&screen_class) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    guard("on_screen_close", || {
        each(&h.screen_close, |ud, f| unsafe {
            f(ud, YogStr::from_str(&sc))
        });
    });
}
