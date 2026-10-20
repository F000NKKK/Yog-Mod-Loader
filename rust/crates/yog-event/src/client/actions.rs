//! This client's own actions: the host's (nothing on a dedicated server).

use yog_core::BlockPos;
use yog_ids as ids;

use crate::pos::Pos;

actions! {
    /// Plays `sound` to this client's player; its handle (to stop it).
    PlaySound { sound: String, volume: f32, pitch: f32, looping: bool } -> Option<i32> = ids::service_client::PLAY_SOUND, "YogClientApi.playSound";
    /// Stops a sound `PlaySound` started.
    StopSound { handle: i32 } -> bool = ids::service_client::STOP_SOUND, "YogClientApi.stopSound";
    /// What this client's player holds: `item\tcount\tsnbt`.
    HeldItem { off_hand: bool } -> Option<String> = ids::service_client::HELD_ITEM, "YogClientApi.heldItem";
    /// Tints the block at `pos` on this client (`0xRRGGBB`; -1 clears it).
    SetBlockTint { dim: String, #[data(with = "Pos")] pos: BlockPos, color: i32 } -> bool = ids::service_client::SET_BLOCK_TINT, "YogClientApi.setBlockTint";
    /// Whether GLFW key `key` is held down.
    KeyDown { key: i32 } -> bool = ids::service_client::KEY_DOWN, "YogClientApi.keyDown";
    /// An attribute's value on this client's player.
    AttributeValue { attribute: String } -> Option<f64> = ids::service_client::ATTRIBUTE_VALUE, "YogClientApi.attributeValue";
    /// Whether this client's player has advancement `id`.
    AdvancementDone { id: String } -> bool = ids::service_client::ADVANCEMENT_DONE, "YogClientApi.advancementDone";
    /// An operation of `yog_ids::client_op` with `args` (JSON); its answer as JSON.
    Op { op: String, args: String } -> Option<String> = ids::service_client::OP, "YogClientOps.call";
    /// Asks the mods about entity `uuid`'s body each frame (`on`) or not.
    BodyTrack { uuid: String, on: bool } -> bool = ids::service_client::BODY_TRACK, "YogClientApi.bodyTrack";
    /// The same for every entity of `entity_type`.
    BodyTrackType { entity_type: String, on: bool } -> bool = ids::service_client::BODY_TRACK_TYPE, "YogClientApi.bodyTrackType";
    /// This client's side of an entity's size (`width` < 0: vanilla's).
    SetDimensions { uuid: String, width: f32, height: f32, eye: f32 } -> bool = ids::service_client::SET_DIMENSIONS, "YogClientApi.setDimensions";
}

/// What this client was told an entity keeps under `key` (the runtime keeps it).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct EntityData {
    pub uuid: String,
    pub key: String,
}
