//! JNI: ABI minor 9 entry points.

use super::*;

// ── ABI minor 9 JNI entry points ─────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpenPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.container_open.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::EMPTY,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpen<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    container_type: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.container_open.is_empty() {
        return;
    }
    let (p, pu, ct, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, container_type),
        jstr!(env, dimension),
    );
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::from_str(&ct),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_container_open", || {
        each(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

