//! The arena a thread's bus calls cross Rust ↔ Java in: memory this side
//! owns, seen by Java as a direct `ByteBuffer` (`YogBus.nativeArena`). A
//! call writes its payload at the arena's top and crosses with primitives
//! only (address or handler, offset, length); the reply is written right
//! after the payload and read in place — no `byte[]` made or copied on
//! either side. Calls nest like a stack: the caller moves the top past its
//! payload, a nested call writes above it, and the caller puts the top back
//! when it has read the reply. What doesn't fit goes the old way (a payload
//! as a `byte[]`, a reply kept aside: [`SPILLED`]).
//!
//! ```text
//! [ top: u32 ] [ … header … ] [ frames, from HEADER … ]
//! ```

use std::cell::{Cell, RefCell};

use jni::JNIEnv;
use jni::objects::{JClass, JObject};
use jni::sys::{jbyteArray, jint, jlong, jobject};
use std::os::raw::c_void;

use yog_abi::{YogReply, YogStatus, status};

use super::ROUTER;
use crate::jcall::jcall;

/// The arena's size: most calls' payload and reply, nested a few deep.
pub(crate) const BYTES: usize = 1 << 16;
/// Bytes before the first frame (`YogBus.ARENA_HEADER`).
pub(crate) const HEADER: usize = 16;
/// Where the top (a little-endian `u32`, from `HEADER`) is (`YogBus.ARENA_TOP`).
pub(crate) const TOP: usize = 0;
/// A reply length saying the reply didn't fit and is kept aside
/// (`YogBus.takeReply` / `nativeTakeReply`).
pub(crate) const SPILLED: u32 = u32::MAX;
/// A packed answer: the status in the high half, the reply's length in the low.
const STATUS_SHIFT: u32 = 32;
const TOP_BYTES: usize = 4;

thread_local! {
    /// This thread's arena: made on first use, never moved or freed while
    /// the thread lives (Java holds a buffer over it).
    static ARENA: Cell<*mut u8> = const { Cell::new(std::ptr::null_mut()) };
    /// A reply too big for the arena, until Java takes it.
    static SPILL: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// This thread's arena.
fn base() -> *mut u8 {
    ARENA.with(|a| {
        if a.get().is_null() {
            a.set(Box::leak(vec![0u8; BYTES].into_boxed_slice()).as_mut_ptr());
        }
        a.get()
    })
}

fn top(base: *mut u8) -> usize {
    let mut b = [0u8; TOP_BYTES];
    unsafe { std::ptr::copy_nonoverlapping(base.add(TOP), b.as_mut_ptr(), TOP_BYTES) };
    u32::from_le_bytes(b) as usize
}

fn set_top(base: *mut u8, top: usize) {
    let b = (top as u32).to_le_bytes();
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), base.add(TOP), TOP_BYTES) };
}

/// Whether `len` bytes fit at frame offset `at`.
fn fits(at: usize, len: usize) -> bool {
    at.checked_add(len).is_some_and(|end| HEADER + end <= BYTES)
}

/// `len` bytes of the arena at frame offset `at`, if they're in it.
fn slice<'a>(base: *mut u8, at: usize, len: usize) -> Option<&'a [u8]> {
    fits(at, len).then(|| unsafe { std::slice::from_raw_parts(base.add(HEADER + at), len) })
}

/// `bytes` written at frame offset `at` — they may be in the arena already,
/// overlapping (a reply moved down onto the place its caller reads it).
fn write(base: *mut u8, at: usize, bytes: &[u8]) {
    unsafe { std::ptr::copy(bytes.as_ptr(), base.add(HEADER + at), bytes.len()) };
}

/// Whether `payload` is the arena's last frame, ending at frame offset `top`
/// (a call from Java passed straight on): it's read where it is.
fn is_top_frame(base: *mut u8, top: usize, payload: &[u8]) -> bool {
    top >= payload.len()
        && std::ptr::eq(payload.as_ptr(), unsafe {
            base.add(HEADER + top - payload.len())
        })
}

/// Where this thread's frames end (none taken: 0).
#[cfg(test)]
pub(crate) fn frames_top() -> usize {
    top(base())
}

/// Runs Java handler `handler` through the arena, its reply written to
/// `reply`: `None` when the payload doesn't fit (the caller sends it the
/// old way).
pub(super) fn run_java(
    env: &mut JNIEnv,
    handler: i32,
    payload: &[u8],
    reply: &YogReply,
) -> Option<YogStatus> {
    let base = base();
    let top0 = top(base);
    let at = if is_top_frame(base, top0, payload) {
        top0 - payload.len()
    } else {
        if !fits(top0, payload.len()) {
            return None;
        }
        write(base, top0, payload);
        top0
    };
    set_top(base, at + payload.len());
    let answer: Option<i64> = unsafe {
        jcall!(
            env,
            "dev/yog/YogBus",
            "handleAt",
            [I(handler), I(at as i32), I(payload.len() as i32)]
        )
    };
    set_top(base, top0);
    let Some(answer) = answer else {
        return Some(status::FAILED);
    };
    let (st, len) = unpack(answer);
    if len == SPILLED {
        let spilled: Option<Vec<u8>> = unsafe { jcall!(env, "dev/yog/YogBus", "takeReply", []) };
        unsafe { YogReply::send(reply, &spilled.unwrap_or_default()) };
    } else if let Some(bytes) = slice(base, at + payload.len(), len as usize) {
        // Read in place: straight from the arena to the caller's sink.
        unsafe { YogReply::send(reply, bytes) };
    }
    Some(st)
}

/// `status` and the reply's length, packed.
fn pack(st: YogStatus, len: u32) -> jlong {
    ((i64::from(st)) << STATUS_SHIFT) | i64::from(len)
}

fn unpack(answer: i64) -> (YogStatus, u32) {
    ((answer >> STATUS_SHIFT) as YogStatus, answer as u32)
}

/// Where a call from Java writes its reply: into the arena right after
/// the payload while it fits, else kept aside ([`SPILL`]).
struct ArenaSink {
    at: usize,
    len: usize,
    spilled: bool,
}

/// [`YogReply::write`] into an [`ArenaSink`].
unsafe extern "C" fn write_sink(sink: *mut c_void, data: *const u8, len: usize) {
    let (Some(s), false) = (unsafe { sink.cast::<ArenaSink>().as_mut() }, data.is_null()) else {
        return;
    };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    if !s.spilled && fits(s.at + s.len, bytes.len()) {
        write(base(), s.at + s.len, bytes);
        s.len += bytes.len();
        return;
    }
    SPILL.with(|spill| {
        let mut spill = spill.borrow_mut();
        if !s.spilled {
            // What was written in the arena moves aside too.
            spill.clear();
            spill.extend_from_slice(slice(base(), s.at, s.len).unwrap_or_default());
            s.spilled = true;
        }
        spill.extend_from_slice(bytes);
    });
}

/// What Java sent from frame offset `at`, `len` long, through `send`: its
/// reply written after the payload (or aside), the answer packed.
fn from_java(at: jint, len: jint, send: impl FnOnce(&[u8], &YogReply) -> YogStatus) -> jlong {
    let (Ok(at), Ok(len)) = (usize::try_from(at), usize::try_from(len)) else {
        return pack(status::BAD_PAYLOAD, 0);
    };
    let base = base();
    let Some(payload) = slice(base, at, len) else {
        return pack(status::BAD_PAYLOAD, 0);
    };
    let mut sink = ArenaSink {
        at: at + len,
        len: 0,
        spilled: false,
    };
    let reply = YogReply {
        sink: (&mut sink as *mut ArenaSink).cast(),
        write: write_sink,
    };
    let st = send(payload, &reply);
    pack(
        st,
        if sink.spilled {
            SPILLED
        } else {
            sink.len as u32
        },
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeArena<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jobject {
    let buffer = unsafe { env.new_direct_byte_buffer(base(), BYTES) };
    buffer.map_or(std::ptr::null_mut(), |b| JObject::from(b).into_raw())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeCallAt<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    to: jint,
    at: jint,
    len: jint,
) -> jlong {
    from_java(at, len, |payload, reply| {
        ROUTER.call(to as u32, payload, reply, super::java::run)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeEmitAt<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    event: jint,
    at: jint,
    len: jint,
) -> jlong {
    from_java(at, len, |payload, reply| {
        ROUTER.emit(event as u32, payload, reply, super::java::run)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeTakeReply<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jbyteArray {
    let reply = SPILL.with(|s| std::mem::take(&mut *s.borrow_mut()));
    env.byte_array_from_slice(&reply)
        .map_or(std::ptr::null_mut(), |a| a.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_pack_their_status_and_length() {
        assert_eq!(
            unpack(pack(status::CANCELLED, 1234)),
            (status::CANCELLED, 1234)
        );
        assert_eq!(unpack(pack(status::OK, SPILLED)), (status::OK, SPILLED));
        assert!(fits(0, BYTES - HEADER) && !fits(1, BYTES - HEADER) && !fits(usize::MAX, 2));
    }

    /// `YogBus.java` reads the arena where this writes it.
    #[test]
    fn java_knows_the_layout() {
        let java = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../java-common/src/main/java/dev/yog/YogBus.java");
        let Ok(src) = std::fs::read_to_string(&java) else {
            return;
        };
        let value = |name: &str| -> Option<usize> {
            let line = src
                .lines()
                .find(|l| l.contains(&format!("public static final int {name} =")))?;
            line.split('=')
                .nth(1)?
                .trim()
                .trim_end_matches(';')
                .parse()
                .ok()
        };
        assert_eq!(value("ARENA_HEADER"), Some(HEADER));
        assert_eq!(value("ARENA_TOP"), Some(TOP));
    }

    #[test]
    fn a_big_reply_is_kept_aside() {
        let at = 8;
        let mut sink = ArenaSink {
            at,
            len: 0,
            spilled: false,
        };
        let reply = YogReply {
            sink: (&mut sink as *mut ArenaSink).cast(),
            write: write_sink,
        };
        unsafe { YogReply::send(&reply, &[7, 7]) };
        unsafe { YogReply::send(&reply, &[8]) };
        assert_eq!((sink.len, sink.spilled), (3, false));
        assert_eq!(slice(base(), at, 3), Some(&[7u8, 7, 8][..]));
        unsafe { YogReply::send(&reply, &vec![1; BYTES]) };
        assert!(sink.spilled);
        // What was in the arena moved aside with the rest.
        assert_eq!(
            SPILL.with(|s| (s.borrow().len(), s.borrow()[..3].to_vec())),
            (BYTES + 3, vec![7, 7, 8])
        );
    }
}
