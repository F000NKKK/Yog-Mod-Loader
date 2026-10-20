//! Advancements: a mod's are content (`.ydef` `kind: "advancement"`); one
//! granted by the mod's code says so (`criteria: [{ granted: true }]`) and
//! is awarded here. What a player has is asked here too, and on the client
//! by [`crate::client::advancement_done`].

use yog_event::advancement;

/// Awards advancement `id` to the player `uuid`; `false` without such a
/// player or advancement.
pub fn grant(uuid: &str, id: &str) -> bool {
    crate::call(&advancement::Grant { uuid: uuid.to_owned(), id: id.to_owned() }).unwrap_or(false)
}

/// Whether the player `uuid` has advancement `id`.
pub fn done(uuid: &str, id: &str) -> bool {
    crate::call(&advancement::Done { uuid: uuid.to_owned(), id: id.to_owned() }).unwrap_or(false)
}
