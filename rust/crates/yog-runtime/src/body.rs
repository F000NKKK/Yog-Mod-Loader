//! Bodies (`yog-body`): what the client tracks and how big an entity is.
//! The bodies, camera and travel step themselves are bus events.

use std::os::raw::c_void;

use yog_abi::YogStr;

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

pub(crate) unsafe extern "C" fn api_client_set_dimensions(_ctx: *mut c_void, uuid: YogStr, w: f32, h: f32, eye: f32) {
    unsafe {
        let _: Option<()> = host!("dev/yog/YogClientApi", "setDimensions", [S(uuid), F(w), F(h), F(eye)]);
    }
}
