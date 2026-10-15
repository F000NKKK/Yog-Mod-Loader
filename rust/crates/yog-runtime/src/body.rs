//! Bodies, camera and movement (ABI 3.0, `yog-body`): the hosts ask here
//! each frame about tracked entities' bodies and the camera, and each tick
//! about the local player's travel step; the mods' handlers answer.

use std::os::raw::c_void;

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jbyteArray, jdouble, jfloat, jfloatArray, jint};
use yog_abi::{YogBodyFn, YogCameraFn, YogCameraFrame, YogCameraOut, YogStr, YogTravelFn, YogTravelFrame};

use super::{each, guard, handlers, handlers_mut};

pub(crate) unsafe extern "C" fn api_on_body(ctx: *mut c_void, ud: *mut c_void, f: YogBodyFn) {
    unsafe { handlers_mut(ctx).body.push((super::current_registration(), ud, f)) }
}

pub(crate) unsafe extern "C" fn api_on_camera(ctx: *mut c_void, ud: *mut c_void, f: YogCameraFn) {
    unsafe { handlers_mut(ctx).camera.push((super::current_registration(), ud, f)) }
}

pub(crate) unsafe extern "C" fn api_on_travel(ctx: *mut c_void, ud: *mut c_void, f: YogTravelFn) {
    unsafe { handlers_mut(ctx).travel.push((super::current_registration(), ud, f)) }
}

pub(crate) unsafe extern "C" fn api_client_body_track(_ctx: *mut c_void, uuid: YogStr, on: bool) {
    unsafe {
        let _: Option<()> = host!("dev/yog/YogClientApi", "bodyTrack", [S(uuid), Z(on)]);
    }
}

pub(crate) unsafe extern "C" fn api_client_body_track_type(_ctx: *mut c_void, entity_type: YogStr, on: bool) {
    unsafe {
        let _: Option<()> = host!("dev/yog/YogClientApi", "bodyTrackType", [S(entity_type), Z(on)]);
    }
}

unsafe extern "C" fn append(sink: *mut c_void, data: *const u8, len: usize) {
    // SAFETY: `sink` is the `Vec<u8>` `body_ops` passes; `data` is valid for the call.
    unsafe { (*(sink as *mut Vec<u8>)).extend_from_slice(std::slice::from_raw_parts(data, len)) }
}

/// Every body handler's ops for `frame`, one after another.
fn body_ops(frame: &[u8]) -> Vec<u8> {
    let entries = handlers().body.clone();
    let mut out = Vec::new();
    guard("on_body", || {
        each(&entries, |ud, f| unsafe { f(ud, frame.as_ptr(), frame.len(), &mut out as *mut Vec<u8> as *mut c_void, append) });
    });
    out
}

/// `byte[] nativeBodyPose(byte[] frame)`: the ops for a tracked entity's
/// body this frame (`yog_body::wire`), or null for none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeBodyPose<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    frame: JByteArray<'l>,
) -> jbyteArray {
    let Ok(frame) = env.convert_byte_array(&frame) else { return std::ptr::null_mut() };
    let ops = body_ops(&frame);
    if ops.is_empty() {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&ops).map(|a| a.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// `float[] nativeCamera(...)`: `{right, up, forward, wx, wy, wz, yaw, pitch,
/// roll, fov}` the mods make of this frame's camera, or null when no mod
/// handles the camera.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeCamera<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    uuid: JString<'l>,
    partial: jfloat,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    yaw: jfloat,
    pitch: jfloat,
    detached: jboolean,
    flags: jint,
) -> jfloatArray {
    let entries = handlers().camera.clone();
    if entries.is_empty() {
        return std::ptr::null_mut();
    }
    let uuid = jstr!(env, uuid, std::ptr::null_mut());
    let frame = YogCameraFrame {
        uuid: YogStr::from_str(&uuid),
        partial,
        pos: [x, y, z],
        yaw,
        pitch,
        detached: detached != 0,
        flags: flags as u32,
    };
    let mut out = YogCameraOut { local_offset: [0.0; 3], world_offset: [0.0; 3], yaw: 0.0, pitch: 0.0, roll: 0.0, fov: 1.0 };
    guard("on_camera", || each(&entries, |ud, f| unsafe { f(ud, &frame, &mut out) }));
    let o = &out;
    let values = [
        o.local_offset[0], o.local_offset[1], o.local_offset[2],
        o.world_offset[0], o.world_offset[1], o.world_offset[2],
        o.yaw, o.pitch, o.roll, o.fov,
    ];
    let Ok(arr) = env.new_float_array(values.len() as i32) else { return std::ptr::null_mut() };
    let _ = env.set_float_array_region(&arr, 0, &values);
    arr.into_raw()
}

/// `float[] nativeTravel(...)`: the velocity a mod moves the local player
/// by this tick instead of vanilla's travel, or null for vanilla's.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTravel<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    vx: jfloat,
    vy: jfloat,
    vz: jfloat,
    lx: jfloat,
    ly: jfloat,
    lz: jfloat,
    yaw: jfloat,
    pitch: jfloat,
    forward: jfloat,
    strafe: jfloat,
    jumping: jboolean,
    sneaking: jboolean,
    gravity: jfloat,
    flags: jint,
) -> jfloatArray {
    let entries = handlers().travel.clone();
    if entries.is_empty() {
        return std::ptr::null_mut();
    }
    let frame = YogTravelFrame {
        velocity: [vx, vy, vz],
        look: [lx, ly, lz],
        yaw,
        pitch,
        forward,
        strafe,
        jumping: jumping != 0,
        sneaking: sneaking != 0,
        gravity,
        flags: flags as u32,
    };
    let mut velocity = None;
    guard("on_travel", || {
        super::each_while(&entries, |ud, f| {
            let mut v = [0.0f32; 3];
            if unsafe { f(ud, &frame, &mut v) } {
                velocity = Some(v);
                return false;
            }
            true
        });
    });
    let Some(v) = velocity else { return std::ptr::null_mut() };
    let Ok(arr) = env.new_float_array(3) else { return std::ptr::null_mut() };
    let _ = env.set_float_array_region(&arr, 0, &v);
    arr.into_raw()
}
