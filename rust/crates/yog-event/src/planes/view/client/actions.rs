//! This client's own actions: the host's (nothing on a dedicated server).

use yog_core::{BlockPos, ScreenSlot, Stack};
use yog_ids as ids;

actions! {
    /// Plays `sound` to this client's player, a streamed one from `from`
    /// seconds; its handle (to stop it).
    PlaySound { sound: String, volume: f32, pitch: f32, looping: bool, from: f32 } -> Option<i32> = ids::service_client::PLAY_SOUND, "client.YogClientApi.playSound";
    /// Stops a sound `PlaySound` started.
    StopSound { handle: i32 } -> bool = ids::service_client::STOP_SOUND, "client.YogClientApi.stopSound";
    /// What this client's player holds.
    HeldItem { off_hand: bool } -> Option<Stack> = ids::service_client::HELD_ITEM, "client.YogClientApi.heldItem";
    /// Tints the block at `pos` on this client (`0xRRGGBB`; -1 clears it).
    SetBlockTint { dim: String, pos: BlockPos, color: i32 } -> bool = ids::service_client::SET_BLOCK_TINT, "client.YogClientApi.setBlockTint";
    /// The slots of the inventory screen this client has open (none when it has none).
    ScreenSlots {  } -> Vec<ScreenSlot> = ids::service_client::SCREEN_SLOTS, "actions.YogQueries.screenSlots";
    /// Whether GLFW key `key` is held down.
    KeyDown { key: i32 } -> bool = ids::service_client::KEY_DOWN, "client.YogClientApi.keyDown";
    /// The key `binding` is bound to, as the controls show it: a game's
    /// binding by its name (`sprint`), a mod's by its id (`mymod:xray`).
    KeyName { binding: String } -> Option<String> = ids::service_client::KEY_NAME, "client.YogClientApi.keyName";
    /// An attribute's value on this client's player.
    AttributeValue { attribute: String } -> Option<f64> = ids::service_client::ATTRIBUTE_VALUE, "client.YogClientApi.attributeValue";
    /// Whether this client's player has advancement `id`.
    AdvancementDone { id: String } -> bool = ids::service_client::ADVANCEMENT_DONE, "client.YogClientApi.advancementDone";
    /// Asks the mods about entity `uuid`'s body each frame (`on`) or not.
    BodyTrack { uuid: String, on: bool } -> bool = ids::service_client::BODY_TRACK, "client.YogClientApi.bodyTrack";
    /// The same for every entity of `entity_type`.
    BodyTrackType { entity_type: String, on: bool } -> bool = ids::service_client::BODY_TRACK_TYPE, "client.YogClientApi.bodyTrackType";
    /// This client's side of an entity's size (`width` < 0: vanilla's).
    SetDimensions { uuid: String, width: f32, height: f32, eye: f32 } -> bool = ids::service_client::SET_DIMENSIONS, "client.YogClientApi.setDimensions";
}

/// What this client was told an entity keeps under `key` (the runtime keeps it).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct EntityData {
    pub uuid: String,
    pub key: String,
}
