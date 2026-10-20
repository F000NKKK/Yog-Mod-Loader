//! What the runtime asks this client itself (for books): an item tag's
//! items, and whether the player has an advancement. Each goes to
//! `dev.yog.YogClientApi`; on a dedicated server it answers nothing.

use yog_abi::YogStr;

use crate::jcall::JArg;

/// The items of item tag `id`, as this client has them.
pub(crate) fn client_tag_items(id: &str) -> Vec<String> {
    let items: Option<String> = unsafe { host!("dev/yog/YogClientApi", "tagItems", [JArg::S(YogStr::from_str(id))]) };
    items.map(|s| s.split(yog_ids::sep::FIELD).filter(|i| !i.is_empty()).map(str::to_string).collect()).unwrap_or_default()
}

/// Whether this client's player has advancement `id` — what books ask of
/// their locked entries.
pub(crate) fn client_advancement_done(id: &str) -> bool {
    unsafe { host!("dev/yog/YogClientApi", "advancementDone", [JArg::S(YogStr::from_str(id))]).unwrap_or(false) }
}
