//! JNI: client-side entry points (ABI minor 10).
use crate::gfx::gl_context::GL;
use crate::gfx::gl_context::GL_GET_PROGRAM_BINARY;
use crate::gfx::gl_context::GL_GET_PROGRAM_IV;
use crate::gfx::gl_context::GL_PROGRAM_BINARY;
use crate::gfx::gl_context::GlCtx;
use crate::gfx::table::GFX_FN_TABLE;
use crate::mods::handlers::UiLayer;
use crate::mods::handlers::handlers;
use jni::JNIEnv;
use jni::objects::JClass;
use jni::objects::JFloatArray;
use jni::sys::jfloat;
use jni::sys::jint;
use yog_abi::YogStr;

// ── ABI minor 10 — client-side JNI entry points ───────────────────────────────

/// The host's GL function pointer for `sym` (0 if it has none).
pub(crate) fn gl_proc_address(sym: &str) -> usize {
    let ptr: Option<i64> = unsafe { bridge!("glProcAddress", [S(YogStr::from_str(sym))]) };
    ptr.unwrap_or(0) as usize
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGlInit<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    crate::jni::util::guard("nativeGlInit", || {
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
    });
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
    crate::jni::util::guard("nativeOnHudRender", || {
        let h = handlers();
        let mut gfx = GFX_FN_TABLE;
        gfx.screen_w = screen_w;
        gfx.screen_h = screen_h;
        gfx.delta_tick = delta_tick;
        gfx.scale_factor = scale_factor;
        gfx.player_pos = [player_x, player_y, player_z];

        crate::gfx::frame::drawing(&gfx, || {
            crate::bus::emitted(&yog_event::client::render::Hud { delta_tick })
        });

        // Render active book UIs.
        let active_layers: Vec<UiLayer> = h.active_uis.lock().clone();
        let sw = screen_w as f32;
        let sh = screen_h as f32;
        let ctx = unsafe { yog_gfx::GfxContext::from_raw(&gfx as *const _) };
        let mut renderers = h.book_renderers.lock();
        for layer in &active_layers {
            if !layer.visible {
                continue;
            }
            if let Some(renderer) = renderers.get_mut(&layer.id) {
                renderer.render(&ctx, sw, sh);
            }
        }
    });
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
    crate::jni::util::guard("nativeOnWorldRender", || {
        if !crate::bus::heard::<yog_event::client::render::World>() {
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
        crate::gfx::frame::drawing(&gfx, || {
            crate::bus::emitted(&yog_event::client::render::World { delta_tick })
        });
    });
}

/// From now on, this thread's raw GL draws are kept for later
/// (`gpu_record`): a GUI extracted now and drawn after (26.x).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGfxRecordStart<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    crate::gfx::gpu_record::start();
}

/// Stops keeping them: the recording's id (0: nothing was drawn).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGfxRecordFinish<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jni::sys::jlong {
    crate::gfx::gpu_record::finish() as jni::sys::jlong
}

/// Draws recording `id` now, on the target bound.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGfxReplay<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    id: jni::sys::jlong,
) {
    crate::jni::util::guard("nativeGfxReplay", || {
        crate::gfx::gpu_record::replay(id as u64)
    });
}
