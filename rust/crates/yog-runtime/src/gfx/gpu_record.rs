//! Raw GL drawn later than it's asked for. On hosts that extract the GUI
//! first and draw it after (26.x), a screen's or the HUD's raw GL asked
//! for while the GUI is extracted would be drawn over by what the frame
//! draws next. Between [`start`] and [`finish`] the calls that draw or set
//! state are kept (data copied) instead of made; the host [`replay`]s them
//! when the GUI is drawn, into a picture the GUI puts in its place.
//! Making buffers, programs and textures still happens at once — their
//! handles are the answer.

use std::cell::RefCell;
use std::collections::HashMap;

use yog_abi::YogStr;
use yog_sync::Mutex;
use yog_sync::atomic::{AtomicU64, Ordering};

use crate::gfx::gpu::{
    gfx_buf_data, gfx_buf_subdata, gfx_clear_scissor, gfx_draw_arrays, gfx_draw_elements,
    gfx_prog_uniform_1f, gfx_prog_uniform_1i, gfx_prog_uniform_2f, gfx_prog_uniform_3f,
    gfx_prog_uniform_4f, gfx_prog_uniform_mat4, gfx_set_blend, gfx_set_depth, gfx_set_scissor,
    gfx_set_viewport, gfx_tex_bind,
};

/// A kept call.
enum Cmd {
    BufData {
        handle: u32,
        data: Vec<u8>,
        dynamic: bool,
    },
    BufSubdata {
        handle: u32,
        offset: u32,
        data: Vec<u8>,
    },
    Uniform1i {
        prog: u32,
        name: String,
        v: i32,
    },
    Uniform1f {
        prog: u32,
        name: String,
        v: f32,
    },
    Uniform2f {
        prog: u32,
        name: String,
        v: [f32; 2],
    },
    Uniform3f {
        prog: u32,
        name: String,
        v: [f32; 3],
    },
    Uniform4f {
        prog: u32,
        name: String,
        v: [f32; 4],
    },
    UniformMat4 {
        prog: u32,
        name: String,
        m: [f32; 16],
    },
    TexBind {
        unit: u32,
        handle: u32,
    },
    DrawArrays {
        vao: u32,
        prog: u32,
        mode: u8,
        first: u32,
        count: u32,
    },
    DrawElements {
        vao: u32,
        ebo: u32,
        prog: u32,
        mode: u8,
        count: u32,
        u32_idx: bool,
    },
    Blend {
        enabled: bool,
        src: u32,
        dst: u32,
    },
    Depth {
        test: bool,
        write: bool,
    },
    Scissor {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    ClearScissor,
    Viewport {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
}

thread_local! {
    /// The calls kept since [`start`], while recording.
    static RECORDING: RefCell<Option<Vec<Cmd>>> = const { RefCell::new(None) };
}

/// Recordings waiting to be replayed, by id.
static RECORDED: Mutex<Option<HashMap<u64, Vec<Cmd>>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

/// From now on, this thread's drawing calls are kept.
pub(crate) fn start() {
    RECORDING.with(|r| *r.borrow_mut() = Some(Vec::new()));
}

/// Stops keeping them: the recording's id (0: nothing was drawn).
pub(crate) fn finish() -> u64 {
    let Some(cmds) = RECORDING.with(|r| r.borrow_mut().take()) else {
        return 0;
    };
    if !cmds
        .iter()
        .any(|c| matches!(c, Cmd::DrawArrays { .. } | Cmd::DrawElements { .. }))
    {
        return 0;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    RECORDED
        .lock()
        .get_or_insert_with(HashMap::new)
        .insert(id, cmds);
    id
}

/// Makes the calls of recording `id` (now, on the bound target), and forgets it.
pub(crate) fn replay(id: u64) {
    let Some(cmds) = RECORDED.lock().as_mut().and_then(|m| m.remove(&id)) else {
        return;
    };
    let s = |name: &String| YogStr::from_str(name);
    for c in &cmds {
        // SAFETY: the real calls, with what was copied when they were asked for.
        unsafe {
            match c {
                Cmd::BufData {
                    handle,
                    data,
                    dynamic,
                } => gfx_buf_data(*handle, data.as_ptr(), data.len() as u32, *dynamic),
                Cmd::BufSubdata {
                    handle,
                    offset,
                    data,
                } => gfx_buf_subdata(*handle, *offset, data.as_ptr(), data.len() as u32),
                Cmd::Uniform1i { prog, name, v } => gfx_prog_uniform_1i(*prog, s(name), *v),
                Cmd::Uniform1f { prog, name, v } => gfx_prog_uniform_1f(*prog, s(name), *v),
                Cmd::Uniform2f { prog, name, v } => gfx_prog_uniform_2f(*prog, s(name), v[0], v[1]),
                Cmd::Uniform3f { prog, name, v } => {
                    gfx_prog_uniform_3f(*prog, s(name), v[0], v[1], v[2])
                }
                Cmd::Uniform4f { prog, name, v } => {
                    gfx_prog_uniform_4f(*prog, s(name), v[0], v[1], v[2], v[3])
                }
                Cmd::UniformMat4 { prog, name, m } => {
                    gfx_prog_uniform_mat4(*prog, s(name), m.as_ptr())
                }
                Cmd::TexBind { unit, handle } => gfx_tex_bind(*unit, *handle),
                Cmd::DrawArrays {
                    vao,
                    prog,
                    mode,
                    first,
                    count,
                } => gfx_draw_arrays(*vao, *prog, *mode, *first, *count),
                Cmd::DrawElements {
                    vao,
                    ebo,
                    prog,
                    mode,
                    count,
                    u32_idx,
                } => gfx_draw_elements(*vao, *ebo, *prog, *mode, *count, *u32_idx),
                Cmd::Blend { enabled, src, dst } => gfx_set_blend(*enabled, *src, *dst),
                Cmd::Depth { test, write } => gfx_set_depth(*test, *write),
                Cmd::Scissor { x, y, w, h } => gfx_set_scissor(*x, *y, *w, *h),
                Cmd::ClearScissor => gfx_clear_scissor(),
                Cmd::Viewport { x, y, w, h } => gfx_set_viewport(*x, *y, *w, *h),
            }
        }
    }
}

/// Keeps `cmd` if this thread records: whether it did (then the call isn't made now).
fn kept(cmd: impl FnOnce() -> Cmd) -> bool {
    RECORDING.with(|r| match r.borrow_mut().as_mut() {
        Some(cmds) => {
            cmds.push(cmd());
            true
        }
        None => false,
    })
}

/// A string the mod passed, copied.
unsafe fn owned(s: YogStr) -> String {
    // SAFETY: the mod's string, valid during the call.
    unsafe { s.as_str() }.to_owned()
}

/// `n` floats at `p`, copied.
unsafe fn floats<const N: usize>(p: *const f32) -> [f32; N] {
    // SAFETY: the mod's array of `N`, valid during the call.
    unsafe { std::slice::from_raw_parts(p, N) }
        .try_into()
        .unwrap_or([0.0; N])
}

/// Bytes the mod passed, copied.
unsafe fn bytes(p: *const u8, len: u32) -> Vec<u8> {
    // SAFETY: the mod's buffer of `len`, valid during the call.
    unsafe { std::slice::from_raw_parts(p, len as usize) }.to_vec()
}

// The table's entries: kept while recording, made otherwise.

pub(crate) unsafe extern "C" fn rec_buf_data(handle: u32, p: *const u8, len: u32, dynamic: bool) {
    if !kept(|| Cmd::BufData {
        handle,
        data: unsafe { bytes(p, len) },
        dynamic,
    }) {
        unsafe { gfx_buf_data(handle, p, len, dynamic) }
    }
}

pub(crate) unsafe extern "C" fn rec_buf_subdata(handle: u32, offset: u32, p: *const u8, len: u32) {
    if !kept(|| Cmd::BufSubdata {
        handle,
        offset,
        data: unsafe { bytes(p, len) },
    }) {
        unsafe { gfx_buf_subdata(handle, offset, p, len) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_1i(prog: u32, name: YogStr, v: i32) {
    if !kept(|| Cmd::Uniform1i {
        prog,
        name: unsafe { owned(name) },
        v,
    }) {
        unsafe { gfx_prog_uniform_1i(prog, name, v) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_1f(prog: u32, name: YogStr, v: f32) {
    if !kept(|| Cmd::Uniform1f {
        prog,
        name: unsafe { owned(name) },
        v,
    }) {
        unsafe { gfx_prog_uniform_1f(prog, name, v) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_2f(prog: u32, name: YogStr, x: f32, y: f32) {
    if !kept(|| Cmd::Uniform2f {
        prog,
        name: unsafe { owned(name) },
        v: [x, y],
    }) {
        unsafe { gfx_prog_uniform_2f(prog, name, x, y) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_3f(prog: u32, name: YogStr, x: f32, y: f32, z: f32) {
    if !kept(|| Cmd::Uniform3f {
        prog,
        name: unsafe { owned(name) },
        v: [x, y, z],
    }) {
        unsafe { gfx_prog_uniform_3f(prog, name, x, y, z) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_4f(
    prog: u32,
    name: YogStr,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
) {
    if !kept(|| Cmd::Uniform4f {
        prog,
        name: unsafe { owned(name) },
        v: [x, y, z, w],
    }) {
        unsafe { gfx_prog_uniform_4f(prog, name, x, y, z, w) }
    }
}

pub(crate) unsafe extern "C" fn rec_uniform_mat4(prog: u32, name: YogStr, m: *const f32) {
    if !kept(|| Cmd::UniformMat4 {
        prog,
        name: unsafe { owned(name) },
        m: unsafe { floats(m) },
    }) {
        unsafe { gfx_prog_uniform_mat4(prog, name, m) }
    }
}

pub(crate) unsafe extern "C" fn rec_tex_bind(unit: u32, handle: u32) {
    if !kept(|| Cmd::TexBind { unit, handle }) {
        unsafe { gfx_tex_bind(unit, handle) }
    }
}

pub(crate) unsafe extern "C" fn rec_draw_arrays(
    vao: u32,
    prog: u32,
    mode: u8,
    first: u32,
    count: u32,
) {
    if !kept(|| Cmd::DrawArrays {
        vao,
        prog,
        mode,
        first,
        count,
    }) {
        unsafe { gfx_draw_arrays(vao, prog, mode, first, count) }
    }
}

pub(crate) unsafe extern "C" fn rec_draw_elements(
    vao: u32,
    ebo: u32,
    prog: u32,
    mode: u8,
    count: u32,
    u32_idx: bool,
) {
    if !kept(|| Cmd::DrawElements {
        vao,
        ebo,
        prog,
        mode,
        count,
        u32_idx,
    }) {
        unsafe { gfx_draw_elements(vao, ebo, prog, mode, count, u32_idx) }
    }
}

pub(crate) unsafe extern "C" fn rec_set_blend(enabled: bool, src: u32, dst: u32) {
    if !kept(|| Cmd::Blend { enabled, src, dst }) {
        unsafe { gfx_set_blend(enabled, src, dst) }
    }
}

pub(crate) unsafe extern "C" fn rec_set_depth(test: bool, write: bool) {
    if !kept(|| Cmd::Depth { test, write }) {
        unsafe { gfx_set_depth(test, write) }
    }
}

pub(crate) unsafe extern "C" fn rec_set_scissor(x: i32, y: i32, w: i32, h: i32) {
    if !kept(|| Cmd::Scissor { x, y, w, h }) {
        unsafe { gfx_set_scissor(x, y, w, h) }
    }
}

pub(crate) unsafe extern "C" fn rec_clear_scissor() {
    if !kept(|| Cmd::ClearScissor) {
        unsafe { gfx_clear_scissor() }
    }
}

pub(crate) unsafe extern "C" fn rec_set_viewport(x: i32, y: i32, w: i32, h: i32) {
    if !kept(|| Cmd::Viewport { x, y, w, h }) {
        unsafe { gfx_set_viewport(x, y, w, h) }
    }
}
