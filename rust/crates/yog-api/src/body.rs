//! Bodies, camera and movement (`yog-body`, see its docs): pose and dress
//! entities, disguise and morph them, move the camera, replace the local
//! player's travel step. Register handlers with [`Registry::on_body`],
//! [`Registry::on_camera`] and [`Registry::on_travel`]; say which entities
//! to ask about with [`track`] and [`track_type`].
//!
//! [`Registry::on_body`]: crate::Registry::on_body
//! [`Registry::on_camera`]: crate::Registry::on_camera
//! [`Registry::on_travel`]: crate::Registry::on_travel

pub use yog_body::*;
use yog_abi::YogStr;

use crate::registry::global_api;

/// Asks the body handlers about entity `uuid` each frame (`on`), or stops.
pub fn track(uuid: &str, on: bool) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    unsafe { ((*api).client_body_track)((*api).ctx, YogStr::from_str(uuid), on) }
}

/// Asks the body handlers about every entity of `entity_type` (`on`), or stops.
pub fn track_type(entity_type: &str, on: bool) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    unsafe { ((*api).client_body_track_type)((*api).ctx, YogStr::from_str(entity_type), on) }
}
