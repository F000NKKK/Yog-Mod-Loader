//! Players: the host's actions.

use crate::Point;
use crate::pos::Pos;
use yog_core::BlockPos;
use yog_ids as ids;

actions! {
    /// Gives `player` `count` of `item`.
    GiveItem { player: String, item: String, count: u32 } -> bool = ids::service_player::GIVE_ITEM, "giveItem";
    /// Moves `player` in its dimension.
    Teleport { player: String, pos: Point } -> bool = ids::service_player::TELEPORT, "teleport";
    /// Moves `player` to `pos` in `dim`.
    TeleportToDim { player: String, dim: String, pos: Point } -> bool = ids::service_player::TELEPORT_TO_DIM, "teleportToDim";
    /// Disconnects `player`.
    Kick { player: String, reason: String } -> bool = ids::service_player::KICK, "kickPlayer";
    /// Sets `player`'s game mode (`survival`, `creative`, …).
    SetGamemode { player: String, mode: String } -> bool = ids::service_player::SET_GAMEMODE, "setGamemode";
    /// `player`'s inventory: `slot\titem\tcount` a line.
    Inventory { player: String } -> Option<String> = ids::service_player::INVENTORY, "playerInventory";
    /// Sets inventory `slot` of `player`.
    SetSlot { player: String, slot: u32, item: String, count: u32 } -> bool = ids::service_player::SET_SLOT, "playerSetSlot";
    /// The custom data (SNBT) of what `player` holds.
    HeldItemNbt { player: String } -> Option<String> = ids::service_player::HELD_ITEM_NBT, "getHeldItemNbt";
    /// Sets the custom data of what `player` holds.
    SetHeldItemNbt { player: String, snbt: String } -> bool = ids::service_player::SET_HELD_ITEM_NBT, "setHeldItemNbt";
    /// The custom data (SNBT) of `player`'s offhand stack.
    OffhandItemNbt { player: String } -> Option<String> = ids::service_player::OFFHAND_ITEM_NBT, "getOffhandItemNbt";
    /// Sets the custom data of `player`'s offhand stack.
    SetOffhandItemNbt { player: String, snbt: String } -> bool = ids::service_player::SET_OFFHAND_ITEM_NBT, "setOffhandItemNbt";
    /// The stack in `slot`: `item\tcount\tsnbt`.
    SlotItem { player: String, slot: u32 } -> Option<String> = ids::service_player::SLOT_ITEM, "getSlotItem";
    /// Sets the stack in `slot`, with custom data.
    SetSlotItem { player: String, slot: u32, item: String, count: u32, snbt: String } -> bool = ids::service_player::SET_SLOT_ITEM, "setSlotItem";
    /// Gives `player` experience points.
    AddXp { player: String, amount: i32 } -> bool = ids::service_player::ADD_XP, "playerAddXp";
    /// Lets `player` fly (or not).
    SetMayFly { player: String, may_fly: bool } -> bool = ids::service_player::SET_MAY_FLY, "playerSetMayFly";
    /// `player`'s state (`yog_core::PlayerInfo`'s line).
    Info { player: String } -> Option<String> = ids::service_player::INFO, "playerInfo";
    /// Puts `item` on cooldown for `player`.
    SetItemCooldown { player: String, item: String, ticks: i32 } -> bool = ids::service_player::SET_ITEM_COOLDOWN, "setItemCooldown";
    /// Places the block of `slot`'s stack at `pos`, as `player` would.
    PlaceBlockFromSlot { player: String, slot: u32, dim: String, #[data(with = "Pos")] pos: BlockPos } -> bool = ids::service_player::PLACE_BLOCK_FROM_SLOT, "placeBlockFromSlot";
    /// The online players' names, one a line.
    Online {  } -> Option<String> = ids::service_player::ONLINE, "onlinePlayers";
}
