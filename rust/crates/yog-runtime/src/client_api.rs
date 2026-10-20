//! What the runtime asks this client itself (for books): an item tag's
//! items, and whether the player has an advancement — through the bus, as
//! any mod would; on a dedicated server nothing answers.

use yog_event::{client, tag};

/// The items of item tag `id`, as this client has them.
pub(crate) fn client_tag_items(id: &str) -> Vec<String> {
    crate::bus::call(&tag::Items { tag: id.to_owned() }).unwrap_or_default()
}

/// Whether this client's player has advancement `id` — what books ask of
/// their locked entries.
pub(crate) fn client_advancement_done(id: &str) -> bool {
    crate::bus::call(&client::AdvancementDone { id: id.to_owned() }).unwrap_or(false)
}
