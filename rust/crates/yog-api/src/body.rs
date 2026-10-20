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
use yog_event::client;
use yog_event::entity::VANILLA_SIZE;

use crate::call;

/// Asks the body handlers about entity `uuid` each frame (`on`), or stops.
pub fn track(uuid: &str, on: bool) {
    call(&client::BodyTrack { uuid: uuid.to_owned(), on });
}

/// What this client was told entity `uuid` keeps under `key`
/// ([`yog_core::Server::set_entity_data`]); body frames carry it too
/// ([`Frame::data`]).
pub fn entity_data(uuid: &str, key: &str) -> Option<Vec<u8>> {
    call(&client::EntityData { uuid: uuid.to_owned(), key: key.to_owned() }).flatten().map(|v| v.bytes)
}

/// This client's side of [`yog_core::Server::entity_set_dimensions`]: the
/// entity's own hitbox (`Some((width, height, eye height))`, blocks) or
/// vanilla's again. The mod sends who has which (its own packet); the local
/// player moves and collides with it.
pub fn set_dimensions(uuid: &str, size: Option<(f32, f32, f32)>) {
    let [width, height, eye] = size.map_or(VANILLA_SIZE, |(w, h, e)| [w, h, e]);
    call(&client::SetDimensions { uuid: uuid.to_owned(), width, height, eye });
}

/// Asks the body handlers about every entity of `entity_type` (`on`), or stops.
pub fn track_type(entity_type: &str, on: bool) {
    call(&client::BodyTrackType { entity_type: entity_type.to_owned(), on });
}
