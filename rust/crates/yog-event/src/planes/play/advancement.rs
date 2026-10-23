//! Advancements: the host's actions.

use yog_ids as ids;

actions! {
    /// Grants advancement `id` to a player.
    Grant { uuid: String, id: String } -> bool = ids::service_advancement::GRANT, "YogAdvancementActions.advancementGrant";
    /// Whether a player has advancement `id`.
    Done { uuid: String, id: String } -> bool = ids::service_advancement::DONE, "YogAdvancementActions.advancementDone";
}
