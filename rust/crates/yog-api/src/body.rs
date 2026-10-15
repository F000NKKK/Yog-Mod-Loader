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

/// This client's side of [`yog_core::Server::entity_set_dimensions`]: the
/// entity's own hitbox (`Some((width, height, eye height))`, blocks) or
/// vanilla's again. The mod sends who has which (its own packet); the local
/// player moves and collides with it.
pub fn set_dimensions(uuid: &str, size: Option<(f32, f32, f32)>) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    let (w, h, e) = size.unwrap_or((-1.0, 0.0, 0.0));
    unsafe { ((*api).client_set_dimensions)((*api).ctx, YogStr::from_str(uuid), w, h, e) }
}

/// Asks the body handlers about every entity of `entity_type` (`on`), or stops.
pub fn track_type(entity_type: &str, on: bool) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    unsafe { ((*api).client_body_track_type)((*api).ctx, YogStr::from_str(entity_type), on) }
}
