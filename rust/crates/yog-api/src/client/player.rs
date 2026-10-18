//! The local player (client): where it is, how it fares, what it holds.
//! `None` (or empty) on a dedicated server and without a player.
//!
//! ```ignore
//! use yog_api::client::player;
//! if let Some(me) = player::state() {
//!     yog_api::info!("at {} {} {}", me.x, me.y, me.z);
//! }
//! ```

use serde::Deserialize;
use serde_json::Value;
use yog_ids::client_op as op;

use super::op::ask;

/// The local player: where it is, where it looks, how it moves and fares.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct State {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Degrees: yaw 0 looks south (+z), 90 west; pitch 90 looks down.
    pub yaw: f32,
    pub pitch: f32,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub on_ground: bool,
    pub in_water: bool,
    pub eye_height: f64,
    pub health: f32,
    pub food: i32,
    pub dimension: String,
    /// The selected hotbar slot, 0–8.
    pub slot: i32,
    /// How charged the next attack is: 1 does full damage.
    pub attack_strength: f32,
}

/// Where the local player is and how it fares; `None` without one.
pub fn state() -> Option<State> {
    ask(op::STATE, Value::Null)
}

/// A stack in the player's inventory.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Stack {
    /// 0–8 the hotbar, 9–35 the rest, 36–39 armor (feet first), 40 the off hand.
    pub slot: i32,
    pub item: String,
    pub count: i32,
}

/// The player's inventory, stacks only.
pub fn inventory() -> Vec<Stack> {
    ask(op::INVENTORY, Value::Null).unwrap_or_default()
}

