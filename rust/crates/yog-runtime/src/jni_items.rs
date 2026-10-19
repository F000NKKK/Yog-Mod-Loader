//! JNI: item insertion.

use super::*;

// ── item insert JNI entry points ─────────────────────────────────────────────

/// A hopper or dropper offers an item to a block that accepts items:
/// `simulate` asks whether it would take it (Pre), else it takes it (Post).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemInsert<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    item_id: JString<'l>,
    count: jint,
    nbt: JString<'l>,
    simulate: jni::sys::jboolean,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.item_insert.is_empty() {
        return 0;
    }
    let (b, d, i, n) = (jstr!(env, block, 0), jstr!(env, dimension, 0), jstr!(env, item_id, 0), jstr!(env, nbt, 0));
    let ev = YogItemInsertEvent {
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        item_id: YogStr::from_str(&i),
        count: count.max(0) as u32,
        nbt: YogStr::from_str(&n),
    };
    let srv = srv_ptr();
    if simulate != 0 {
        // Some handler takes it.
        !allowed(&h.item_insert, |ud, f| unsafe { !f(ud, srv, &ev, 0) }) as jni::sys::jboolean
    } else {
        let mut taken = false;
        guard("on_item_insert", || {
            each(&h.item_insert, |ud, f| taken |= unsafe { f(ud, srv, &ev, 1) });
        });
        taken as jni::sys::jboolean
    }
}
