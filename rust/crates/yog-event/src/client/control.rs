//! This client's player doing what a player does — its own keys held, its
//! clicks — for tours, cut scenes, bots: the host's actions (nothing on a
//! dedicated server, nor without a player).

use yog_core::{BlockPos, ClickMode, Face};
use yog_ids as ids;

actions! {
    /// Holds the movement keys each tick till [`release`]: `forward` -1
    /// (back) to 1, `strafe` -1 (right) to 1 (left); `use_item` raises a
    /// shield, draws a bow, eats.
    Drive { forward: f32, strafe: f32, jump: bool, sneak: bool, sprint: bool, use_item: bool } -> bool = ids::service_client_control::DRIVE, "YogClientOps.drive";
    /// Gives the movement keys back to the player.
    Release {  } -> bool = ids::service_client_control::RELEASE, "YogClientOps.release";
    /// Turns the player to `yaw`, `pitch` (degrees).
    Look { yaw: f32, pitch: f32 } -> bool = ids::service_client_control::LOOK, "YogClientOps.look";
    /// Attacks entity `entity` (its uuid), or (`None`) what the crosshair is on; whether something was hit.
    Attack { entity: Option<String> } -> bool = ids::service_client_control::ATTACK, "YogClientOps.attack";
    /// Uses the held item, the main hand's or the off hand's.
    UseItem { off_hand: bool } -> bool = ids::service_client_control::USE_ITEM, "YogClientOps.useItem";
    /// Keeps breaking the block at `pos` from `face` (call each tick); whether it's still there.
    Mine { pos: BlockPos, face: Face } -> bool = ids::service_client_control::MINE, "YogClientOps.mine";
    /// Stops breaking a block.
    StopMining {  } -> bool = ids::service_client_control::STOP_MINING, "YogClientOps.stopMining";
    /// Places the held block against the block at `pos`, on `face`; whether the game took it.
    Place { pos: BlockPos, face: Face } -> bool = ids::service_client_control::PLACE, "YogClientOps.place";
    /// Selects hotbar slot `slot` (0–8).
    SelectSlot { slot: u32 } -> bool = ids::service_client_control::SELECT_SLOT, "YogClientOps.selectSlot";
    /// Clicks `slot` of the open container (`button` as the game's).
    Click { slot: i32, button: i32, mode: ClickMode } -> bool = ids::service_client_control::CLICK, "YogClientOps.click";
    /// Sends `text` to the chat — a command when it starts with `/`.
    Chat { text: String } -> bool = ids::service_client_control::CHAT, "YogClientOps.chat";
}
