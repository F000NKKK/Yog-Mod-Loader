//! Advancements: a mod's are content (`.ydef` `kind: "advancement"`); one
//! granted by the mod's code says so (`criteria: [{ granted: true }]`) and
//! is awarded here. What a player has is asked here too, and on the client
//! by [`crate::client::advancement_done`].

use yog_abi::YogStr;

/// Awards advancement `id` to the player `uuid`; `false` without such a
/// player or advancement.
pub fn grant(uuid: &str, id: &str) -> bool {
    crate::registry::server_table().is_some_and(|s| unsafe { (s.advancement_grant)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(id)) })
}

/// Whether the player `uuid` has advancement `id`.
pub fn done(uuid: &str, id: &str) -> bool {
    crate::registry::server_table().is_some_and(|s| unsafe { (s.advancement_done)(s.ctx, YogStr::from_str(uuid), YogStr::from_str(id)) })
}
