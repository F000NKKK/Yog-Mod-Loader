//! Tags: the host's actions.

use yog_ids as ids;

actions! {
    /// Whether `item` is in item tag `tag`.
    HasItem { item: String, tag: String } -> bool = ids::service_tag::HAS_ITEM, "hasItemTag";
    /// Whether `block` is in block tag `tag`.
    HasBlock { block: String, tag: String } -> bool = ids::service_tag::HAS_BLOCK, "hasBlockTag";
    /// Whether `entity_type` is in entity type tag `tag`.
    HasEntity { entity_type: String, tag: String } -> bool = ids::service_tag::HAS_ENTITY, "hasEntityTag";
    /// The items of item tag `tag`.
    Items { tag: String } -> Vec<String> = ids::service_tag::ITEMS, "YogClientApi.tagItems";
}
