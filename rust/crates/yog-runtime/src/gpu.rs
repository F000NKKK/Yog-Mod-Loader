//! The low-level GPU pipeline (ABI 14): buffers, vertex arrays, shaders, textures, draws, render state.
use glow::HasContext;
use super::{GL, GL_GET_PROGRAM_BINARY, GL_GET_PROGRAM_IV, GL_PROGRAM_BINARY, NonZeroU32, PathBuf, YogStr, c_void};


/// A user's cache, in their home (XDG's default).
const USER_CACHE: &str = ".cache";

// ── ABI minor 14 — low-level GPU pipeline ────────────────────────────────────
//
// All raw GL calls go through `glow::Context` stored in GL.
// `draw2d_*` functions still call JNI (NativeDraw) for MC text/texture rendering.
// Everything is called on the render thread — no synchronization needed.

// ── helpers ───────────────────────────────────────────────────────────────────

pub(super) fn gl_draw_mode(mode: u8) -> u32 {
    match mode {
        1 => glow::LINES,
        2 => glow::LINE_STRIP,
        3 => glow::TRIANGLE_STRIP,
        4 => glow::TRIANGLE_FAN,
        _ => glow::TRIANGLES,
    }
}

pub(super) fn gl_attr_type(dtype: u8) -> u32 {
    match dtype {
        1 => glow::UNSIGNED_BYTE,
        2 => glow::INT,
        3 => glow::UNSIGNED_INT,
        _ => glow::FLOAT,
    }
}

pub(super) unsafe fn compile_shader(gl: &glow::Context, stage: u32, src: &str) -> Option<glow::NativeShader> {
    unsafe {
        let sh = gl.create_shader(stage).ok()?;
        gl.shader_source(sh, src);
        gl.compile_shader(sh);
        if !gl.get_shader_compile_status(sh) {
            yog_logging::error!("yog-gfx shader compile: {}", gl.get_shader_info_log(sh));
            gl.delete_shader(sh);
            return None;
        }
        Some(sh)
    }
}

// ── GPU buffers ───────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn gfx_buf_create() -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        g.0.create_buffer().map(|b| b.0.get()).unwrap_or(0)
    }
}

pub(super) unsafe extern "C" fn gfx_buf_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_buffer(glow::NativeBuffer(n));
    }
}

pub(super) unsafe extern "C" fn gfx_buf_data(handle: u32, bytes: *const u8, len: u32, dynamic: bool) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        let gl = &g.0;
        let data = std::slice::from_raw_parts(bytes, len as usize);
        let usage = if dynamic {
            glow::DYNAMIC_DRAW
        } else {
            glow::STATIC_DRAW
        };
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data, usage);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    }
}

pub(super) unsafe extern "C" fn gfx_buf_subdata(handle: u32, offset: u32, bytes: *const u8, len: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        let gl = &g.0;
        let data = std::slice::from_raw_parts(bytes, len as usize);
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
        gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, offset as i32, data);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    }
}

// ── Vertex arrays ─────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn gfx_vao_create() -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        g.0.create_vertex_array().map(|v| v.0.get()).unwrap_or(0)
    }
}

pub(super) unsafe extern "C" fn gfx_vao_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_vertex_array(glow::NativeVertexArray(n));
    }
}

pub(super) unsafe extern "C" fn gfx_vao_attrib(
    vao: u32,
    vbo: u32,
    index: u32,
    components: u8,
    dtype: u8,
    normalized: bool,
    stride: u32,
    offset: u32,
) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(bn)) = (NonZeroU32::new(vao), NonZeroU32::new(vbo)) else {
            return;
        };
        let gl = &g.0;
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(bn)));
        let gl_type = gl_attr_type(dtype);
        if dtype == 2 || dtype == 3 {
            gl.vertex_attrib_pointer_i32(
                index,
                components as i32,
                gl_type,
                stride as i32,
                offset as i32,
            );
        } else {
            gl.vertex_attrib_pointer_f32(
                index,
                components as i32,
                gl_type,
                normalized,
                stride as i32,
                offset as i32,
            );
        }
        gl.enable_vertex_attrib_array(index);
        gl.bind_vertex_array(None);
    }
}

pub(super) unsafe extern "C" fn gfx_vao_set_ebo(vao: u32, ebo: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(en)) = (NonZeroU32::new(vao), NonZeroU32::new(ebo)) else {
            return;
        };
        let gl = &g.0;
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(glow::NativeBuffer(en)));
        gl.bind_vertex_array(None);
    }
}

// ── Shader programs ───────────────────────────────────────────────────────────

/// Returns a path under `~/.cache/yog/shaders/<hash>.ysc` for the given GLSL source pair,
/// creating the directory if needed.  Returns `None` if the home directory is unknown.
pub(super) fn shader_cache_path(vert: &str, frag: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    vert.hash(&mut h);
    frag.hash(&mut h);
    let hash = h.finish();
    let dir = std::env::var("HOME")
        .map(|home| {
            PathBuf::from(home)
                .join(USER_CACHE)
                .join(yog_ids::user_cache::CACHE)
                .join(yog_ids::user_cache::SHADERS)
        })
        .ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{hash:016x}.ysc")))
}

/// Try restoring a program from a binary blob (`[4 LE bytes: GL format][binary…]`).
/// Returns `Some(handle)` if the driver accepts the binary, `None` otherwise.
///
/// Uses `glProgramBinary` (GL 4.1 / ARB_get_program_binary) via the raw pointer
/// captured in `nativeGlInit`.  Returns `None` if the extension is unavailable.
pub(super) unsafe fn load_shader_binary(gl: &glow::Context, data: &[u8]) -> Option<glow::NativeProgram> {
    unsafe {
        if data.len() < 4 {
            return None;
        }
        type ProgramBinaryFn = unsafe extern "system" fn(u32, u32, *const c_void, i32);
        let fn_ptr = (*GL_PROGRAM_BINARY.get()?)?;
        let program_binary: ProgramBinaryFn = std::mem::transmute(fn_ptr);

        let fmt = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let prog = gl.create_program().ok()?;
        program_binary(
            prog.0.get(),
            fmt,
            data[4..].as_ptr() as *const c_void,
            (data.len() - 4) as i32,
        );
        if gl.get_program_link_status(prog) {
            Some(prog)
        } else {
            gl.delete_program(prog);
            None
        }
    }
}

/// Read the compiled binary of a linked program.  Returns empty `Vec` if unsupported.
pub(super) unsafe fn get_shader_binary(prog: glow::NativeProgram) -> (Vec<u8>, u32) {
    unsafe {
        type GetProgramivFn = unsafe extern "system" fn(u32, u32, *mut i32);
        type GetProgramBinaryFn =
            unsafe extern "system" fn(u32, i32, *mut i32, *mut u32, *mut c_void);

        let Some(get_iv_raw) = GL_GET_PROGRAM_IV.get().and_then(|v| *v) else {
            return (vec![], 0);
        };
        let Some(get_bin_raw) = GL_GET_PROGRAM_BINARY.get().and_then(|v| *v) else {
            return (vec![], 0);
        };
        let get_program_iv: GetProgramivFn = std::mem::transmute(get_iv_raw);
        let get_program_binary: GetProgramBinaryFn = std::mem::transmute(get_bin_raw);

        // Query binary size via glGetProgramiv(GL_PROGRAM_BINARY_LENGTH).
        const PROGRAM_BINARY_LENGTH: u32 = 0x8741;
        let mut size: i32 = 0;
        get_program_iv(prog.0.get(), PROGRAM_BINARY_LENGTH, &mut size);
        if size <= 0 {
            return (vec![], 0);
        }

        let mut buf = vec![0u8; size as usize];
        let mut actual_len: i32 = 0;
        let mut fmt: u32 = 0;
        get_program_binary(
            prog.0.get(),
            size,
            &mut actual_len,
            &mut fmt,
            buf.as_mut_ptr() as *mut c_void,
        );
        buf.truncate(actual_len.max(0) as usize);
        (buf, fmt)
    }
}

pub(super) unsafe extern "C" fn gfx_prog_create(vert: YogStr, frag: YogStr, out: *mut u32) -> bool {
    unsafe {
        let Some(g) = GL.get() else { return false };
        let gl = &g.0;
        let vert_s = vert.as_str();
        let frag_s = frag.as_str();

        // Fast path: binary shader cache — avoids GLSL re-compilation on subsequent launches.
        let cache_path = shader_cache_path(vert_s, frag_s);
        if let Some(ref path) = cache_path
            && let Ok(data) = std::fs::read(path)
        {
            if let Some(prog) = load_shader_binary(gl, &data) {
                *out = prog.0.get();
                return true;
            }
            // Cache stale (driver updated?); remove and fall through to GLSL compile.
            let _ = std::fs::remove_file(path);
        }

        // GLSL compile path.
        let vs = match compile_shader(gl, glow::VERTEX_SHADER, vert_s) {
            Some(s) => s,
            None => return false,
        };
        let fs = match compile_shader(gl, glow::FRAGMENT_SHADER, frag_s) {
            Some(s) => s,
            None => {
                gl.delete_shader(vs);
                return false;
            }
        };
        let prog = match gl.create_program() {
            Ok(p) => p,
            Err(e) => {
                yog_logging::error!("yog-gfx: create_program: {}", e);
                gl.delete_shader(vs);
                gl.delete_shader(fs);
                return false;
            }
        };
        gl.attach_shader(prog, vs);
        gl.attach_shader(prog, fs);
        gl.link_program(prog);
        gl.detach_shader(prog, vs);
        gl.detach_shader(prog, fs);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !gl.get_program_link_status(prog) {
            yog_logging::error!("yog-gfx: shader link: {}", gl.get_program_info_log(prog));
            gl.delete_program(prog);
            return false;
        }

        // Persist binary so the next launch skips GLSL compilation entirely.
        if let Some(ref path) = cache_path {
            let (binary, fmt) = get_shader_binary(prog);
            if !binary.is_empty() {
                let mut blob = fmt.to_le_bytes().to_vec();
                blob.extend_from_slice(&binary);
                let _ = std::fs::write(path, &blob);
            }
        }

        *out = prog.0.get();
        true
    }
}

pub(super) unsafe extern "C" fn gfx_prog_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_program(glow::NativeProgram(n));
    }
}

macro_rules! with_prog {
    ($handle:expr_2021, |$gl:ident, $prog:ident, $loc:ident ($name:expr_2021)| $body:expr_2021) => {{
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new($handle) else {
            return;
        };
        let $gl = &g.0;
        let $prog = glow::NativeProgram(n);
        $gl.use_program(Some($prog));
        let $loc = $gl.get_uniform_location($prog, $name.as_str());
        $body
    }};
}

pub(super) unsafe extern "C" fn gfx_prog_uniform_1i(prog: u32, name: YogStr, v: i32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_i32(loc.as_ref(), v));
    }
}
pub(super) unsafe extern "C" fn gfx_prog_uniform_1f(prog: u32, name: YogStr, v: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_f32(loc.as_ref(), v));
    }
}
pub(super) unsafe extern "C" fn gfx_prog_uniform_2f(prog: u32, name: YogStr, x: f32, y: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_2_f32(
            loc.as_ref(),
            x,
            y
        ));
    }
}
pub(super) unsafe extern "C" fn gfx_prog_uniform_3f(prog: u32, name: YogStr, x: f32, y: f32, z: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_3_f32(
            loc.as_ref(),
            x,
            y,
            z
        ));
    }
}
pub(super) unsafe extern "C" fn gfx_prog_uniform_4f(prog: u32, name: YogStr, x: f32, y: f32, z: f32, w: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_4_f32(
            loc.as_ref(),
            x,
            y,
            z,
            w
        ));
    }
}
pub(super) unsafe extern "C" fn gfx_prog_uniform_mat4(prog: u32, name: YogStr, col_major: *const f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| {
            let data = std::slice::from_raw_parts(col_major, 16);
            gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, data);
        });
    }
}

// ── Textures ──────────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn gfx_tex_create(w: u32, h: u32, rgba: *const u8, linear: bool) -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        let gl = &g.0;
        let tex = match gl.create_texture() {
            Ok(t) => t,
            Err(_) => return 0,
        };
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        let filter = if linear { glow::LINEAR } else { glow::NEAREST };
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        let pixels = std::slice::from_raw_parts(rgba, (w * h * 4) as usize);
        // The game leaves its own unpacking (row length, skips) set from its
        // atlas uploads: read with those, a tight buffer is overrun.
        unpack_tightly(gl);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            w as i32,
            h as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(pixels)),
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        tex.0.get()
    }
}

/// Pixel unpacking as a tightly packed client buffer needs it.
pub(super) unsafe fn unpack_tightly(gl: &glow::Context) {
    unsafe {
        gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
    }
}

pub(super) unsafe extern "C" fn gfx_tex_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_texture(glow::NativeTexture(n));
    }
}

pub(super) unsafe extern "C" fn gfx_tex_bind(unit: u32, handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        gl.active_texture(glow::TEXTURE0 + unit);
        match NonZeroU32::new(handle) {
            Some(n) => gl.bind_texture(glow::TEXTURE_2D, Some(glow::NativeTexture(n))),
            None => gl.bind_texture(glow::TEXTURE_2D, None),
        }
    }
}

pub(super) unsafe extern "C" fn gfx_tex_from_mc(id: YogStr) -> u32 {
    unsafe { host!("dev/yog/NativeDraw", "getMcTextureId", [S(id)]).map_or(0, |id: i32| id as u32) }
}

// ── Draw calls ────────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn gfx_draw_arrays(vao: u32, prog: u32, mode: u8, first: u32, count: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else {
            return;
        };
        let gl = &g.0;
        gl.use_program(Some(glow::NativeProgram(pn)));
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.draw_arrays(gl_draw_mode(mode), first as i32, count as i32);
        gl.bind_vertex_array(None);
    }
}

pub(super) unsafe extern "C" fn gfx_draw_elements(
    vao: u32,
    ebo: u32,
    prog: u32,
    mode: u8,
    count: u32,
    u32_idx: bool,
) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else {
            return;
        };
        let gl = &g.0;
        gl.use_program(Some(glow::NativeProgram(pn)));
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        // EBO is stored in the VAO; ebo param is informational for safety but not re-bound here.
        let _ = ebo;
        let idx_type = if u32_idx {
            glow::UNSIGNED_INT
        } else {
            glow::UNSIGNED_SHORT
        };
        gl.draw_elements(gl_draw_mode(mode), count as i32, idx_type, 0);
        gl.bind_vertex_array(None);
    }
}

// ── Render state ──────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn gfx_set_blend(enabled: bool, src: u32, dst: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        if enabled {
            gl.enable(glow::BLEND);
            gl.blend_func(src, dst);
        } else {
            gl.disable(glow::BLEND);
        }
    }
}

pub(super) unsafe extern "C" fn gfx_set_depth(test: bool, write: bool) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        if test {
            gl.enable(glow::DEPTH_TEST);
        } else {
            gl.disable(glow::DEPTH_TEST);
        }
        gl.depth_mask(write);
        // Disable face culling: our 2D quads use CW winding (back-face) due to y-flip.
        if !test {
            gl.disable(glow::CULL_FACE);
        }
    }
}

pub(super) unsafe extern "C" fn gfx_set_scissor(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(x, y, w, h);
    }
}

pub(super) unsafe extern "C" fn gfx_clear_scissor() {
    unsafe {
        let Some(g) = GL.get() else { return };
        g.0.disable(glow::SCISSOR_TEST);
    }
}

pub(super) unsafe extern "C" fn gfx_set_viewport(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        g.0.viewport(x, y, w, h);
    }
}
