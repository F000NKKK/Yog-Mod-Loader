//! Players: the host's actions.

use crate::Point;
use yog_core::{BlockPos, PlayerInfo, SlotStack, Stack};
use yog_ids as ids;

actions! {
    /// Gives `player` `count` of `item`.
    GiveItem { player: String, item: String, count: u32 } -> bool = ids::service_player::GIVE_ITEM, "actions.YogPlayerActions.giveItem";
    /// Moves `player` in its dimension.
    Teleport { player: String, pos: Point } -> bool = ids::service_player::TELEPORT, "actions.YogPlayerActions.teleport";
    /// Moves `player` to `pos` in `dim`.
    TeleportToDim { player: String, dim: String, pos: Point } -> bool = ids::service_player::TELEPORT_TO_DIM, "actions.YogPlayerActions.teleportToDim";
    /// Disconnects `player`.
    Kick { player: String, reason: String } -> bool = ids::service_player::KICK, "actions.YogPlayerActions.kickPlayer";
    /// Sets `player`'s game mode (`survival`, `creative`, …).
    SetGamemode { player: String, mode: String } -> bool = ids::service_player::SET_GAMEMODE, "actions.YogPlayerActions.setGamemode";
    /// `player`'s inventory: its stacks, by slot.
    Inventory { player: String } -> Vec<SlotStack> = ids::service_player::INVENTORY, "actions.YogQueries.playerInventory";
    /// Sets inventory `slot` of `player`.
    SetSlot { player: String, slot: u32, item: String, count: u32 } -> bool = ids::service_player::SET_SLOT, "actions.YogPlayerActions.playerSetSlot";
    /// The custom data (SNBT) of what `player` holds.
    HeldItemNbt { player: String } -> Option<String> = ids::service_player::HELD_ITEM_NBT, "actions.YogPlayerActions.getHeldItemNbt";
    /// Sets the custom data of what `player` holds.
    SetHeldItemNbt { player: String, snbt: String } -> bool = ids::service_player::SET_HELD_ITEM_NBT, "actions.YogPlayerActions.setHeldItemNbt";
    /// The custom data (SNBT) of `player`'s offhand stack.
    OffhandItemNbt { player: String } -> Option<String> = ids::service_player::OFFHAND_ITEM_NBT, "actions.YogPlayerActions.getOffhandItemNbt";
    /// Sets the custom data of `player`'s offhand stack.
    SetOffhandItemNbt { player: String, snbt: String } -> bool = ids::service_player::SET_OFFHAND_ITEM_NBT, "actions.YogPlayerActions.setOffhandItemNbt";
    /// The bytes kept under `key` in the custom data of the stack `player`
    /// holds (`off_hand`: in its other hand): a `saved` item's state.
    HeldItemData { player: String, off_hand: bool, key: String } -> Option<crate::storage::Record> = ids::service_player::HELD_ITEM_DATA, "actions.YogItemData.read";
    /// Keeps `bytes` under `key` in the custom data of the stack `player` holds.
    SetHeldItemData { player: String, off_hand: bool, key: String, #[data(with = "yog_data::with::Raw")] bytes: Vec<u8> } -> bool = ids::service_player::SET_HELD_ITEM_DATA, "actions.YogItemData.write";
    /// The stack in `slot`.
    SlotItem { player: String, slot: u32 } -> Option<Stack> = ids::service_player::SLOT_ITEM, "actions.YogQueries.getSlotItem";
    /// Sets the stack in `slot`, with custom data.
    SetSlotItem { player: String, slot: u32, item: String, count: u32, snbt: String } -> bool = ids::service_player::SET_SLOT_ITEM, "actions.YogPlayerActions.setSlotItem";
    /// Gives `player` experience points.
    AddXp { player: String, amount: i32 } -> bool = ids::service_player::ADD_XP, "actions.YogPlayerActions.playerAddXp";
    /// Lets `player` fly (or not).
    SetMayFly { player: String, may_fly: bool } -> bool = ids::service_player::SET_MAY_FLY, "actions.YogPlayerActions.playerSetMayFly";
    /// `player`'s state.
    Info { player: String } -> Option<PlayerInfo> = ids::service_player::INFO, "actions.YogQueries.playerInfo";
    /// Puts `item` on cooldown for `player`.
    SetItemCooldown { player: String, item: String, ticks: i32 } -> bool = ids::service_player::SET_ITEM_COOLDOWN, "actions.YogPlayerActions.setItemCooldown";
    /// Places the block of `slot`'s stack at `pos`, as `player` would.
    PlaceBlockFromSlot { player: String, slot: u32, dim: String, pos: BlockPos } -> bool = ids::service_player::PLACE_BLOCK_FROM_SLOT, "actions.YogPlayerActions.placeBlockFromSlot";
    /// The online players' names.
    Online {  } -> Vec<String> = ids::service_player::ONLINE, "actions.YogQueries.onlinePlayers";
}
