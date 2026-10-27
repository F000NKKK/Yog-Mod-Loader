//! Values kept on a stack — the Rust side of a Yog Script item's `saved`
//! and `synced` members (`docs/design/storage.md`): bytes by key in the
//! custom data of what a player holds, so they travel with the stack and
//! reach its clients with it.

use std::marker::PhantomData;

use yog_data::Schema;

/// A value kept in the data of the stack a player holds, at its key (the
/// mod's own: `<mod>:<key>`); its default where the stack has none yet.
///
/// ```ignore
/// static CHARGES: ItemData<u32> = ItemData::new("charges");
/// CHARGES.update(&use_item.player, use_item.off_hand, |n| *n = n.saturating_sub(1));
/// ```
pub struct ItemData<T> {
    key: &'static str,
    value: PhantomData<fn() -> T>,
}

impl<T: Schema + Default> ItemData<T> {
    pub const fn new(key: &'static str) -> Self {
        ItemData {
            key,
            value: PhantomData,
        }
    }

    /// The value on the stack `player` holds (`off_hand`: in its other
    /// hand); its default when the stack has none (or there's no stack).
    pub fn get(&self, player: &str, off_hand: bool) -> T {
        crate::event::player::held_item_data(player, off_hand, self.stack_key())
            .and_then(|r| yog_data::read_bytes(&r.bytes).ok())
            .unwrap_or_default()
    }

    /// The value on the stack this client's player holds (`off_hand`: in
    /// its other hand), as the server last sent the stack — the game sends
    /// stacks, so every `ItemData` is synced; its default when the stack has
    /// none (or there's no stack).
    pub fn held_here(&self, off_hand: bool) -> T {
        crate::event::client::held_item_data(off_hand, self.stack_key())
            .and_then(|r| yog_data::read_bytes(&r.bytes).ok())
            .unwrap_or_default()
    }

    /// Puts `value` on the stack `player` holds; false when it holds nothing.
    pub fn set(&self, player: &str, off_hand: bool, value: &T) -> bool {
        crate::event::player::set_held_item_data(
            player,
            off_hand,
            self.stack_key(),
            yog_data::to_bytes(value),
        )
    }

    /// The value on the stack `player` holds changed by `change`, and put
    /// back; what `change` returns, none when it holds nothing.
    pub fn update<R>(
        &self,
        player: &str,
        off_hand: bool,
        change: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let mut value = self.get(player, off_hand);
        let out = change(&mut value);
        self.set(player, off_hand, &value).then_some(out)
    }

    /// The key in a stack's data: the mod's, then its own (as Yog Script's are).
    fn stack_key(&self) -> String {
        format!("{}{}{}", crate::mod_id(), yog_ids::sep::NAMESPACE, self.key)
    }
}
