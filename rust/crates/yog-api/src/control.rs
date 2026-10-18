//! Doing what a player does, for automation — tours, cut scenes, bots:
//! moving (its own keys held), looking, attacking, using, mining, placing,
//! picking a slot, clicking a container slot, chatting. No-ops on a
//! dedicated server and without a player.
//!
//! ```ignore
//! use yog_api::{client::player, control};
//! if let Some(me) = player::state() {
//!     control::look(me.yaw + 90.0, 0.0);
//!     control::drive(control::Drive { forward: 1.0, ..Default::default() });
//! }
//! ```

use serde::Serialize;
use serde_json::{Value, json};
use yog_ids::client_op as op;

use crate::client::op::{ask, call};

/// Movement keys held for the player, till [`release`].
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct Drive {
    /// -1 (back) to 1 (forward).
    pub forward: f32,
    /// -1 (right) to 1 (left), as the game's input has it.
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

/// Holds `keys` for the player each tick, till [`release`].
pub fn drive(keys: Drive) {
    call(op::DRIVE, json!(keys));
}

/// Gives the movement keys back to the player.
pub fn release() {
    call(op::RELEASE, Value::Null);
}

/// Turns the player to `yaw`, `pitch` (degrees).
pub fn look(yaw: f32, pitch: f32) {
    call(op::LOOK, json!({ "yaw": yaw, "pitch": pitch }));
}

/// Attacks entity `uuid`, or (`None`) what the crosshair is on.
pub fn attack(uuid: Option<&str>) {
    call(op::ATTACK, json!({ "entity": uuid }));
}

/// Uses the held item, the main hand's or the off hand's.
pub fn use_item(off_hand: bool) {
    call(op::USE, json!({ "off_hand": off_hand }));
}

/// A block's side: the game's `Direction`.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

/// Keeps breaking the block at `pos` from `face` (call each tick); whether it's still there.
pub fn mine(pos: [i32; 3], face: Face) -> bool {
    ask(op::MINE, json!({ "x": pos[0], "y": pos[1], "z": pos[2], "face": face })).unwrap_or(false)
}

/// Stops breaking a block.
pub fn stop_mining() {
    call(op::MINE, json!({ "stop": true }));
}

/// Places the held block against the block at `pos`, on `face`; whether the game took it.
pub fn place(pos: [i32; 3], face: Face) -> bool {
    ask(op::PLACE, json!({ "x": pos[0], "y": pos[1], "z": pos[2], "face": face })).unwrap_or(false)
}

/// Selects hotbar slot `slot` (0–8).
pub fn select_slot(slot: u8) {
    call(op::SLOT, json!({ "slot": slot }));
}

/// Clicks `slot` of the open container (`button` and `mode` as the game's `ClickType`).
pub fn click(slot: i32, button: i32, mode: &str) {
    call(op::CLICK, json!({ "slot": slot, "button": button, "mode": mode }));
}

/// Sends `text` to the chat — a command when it starts with `/`.
pub fn chat(text: &str) {
    call(op::CHAT, json!({ "text": text }));
}
