package dev.yog;

import java.util.Locale;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.GameType;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/** The host's player actions: the methods {@code yog_event::player} names. */
public final class YogPlayerActions {
    private YogPlayerActions() {}

    /** Give `count` of `itemId` to the named online player. */
    @YogServiceHost
    public static boolean giveItem(String player, String itemId, int count) {
        ServerPlayer p = NativeBridge.playerByName(player);
        Identifier id = Identifier.tryParse(itemId);
        if (p == null) { YogLog.warn("giveItem: player not found: " + player); return false; }
        if (id == null) { YogLog.warn("giveItem: bad id: " + itemId); return false; }
        if (count <= 0) { YogLog.warn("giveItem: bad count: " + count); return false; }
        if (!BuiltInRegistries.ITEM.containsKey(id)) {
            YogLog.info("giveItem: item not registered: " + itemId);
            // List what IS registered for debugging
            if (itemId.startsWith(YogNames.Ns.YOG + YogNames.Sep.NAMESPACE)) {
                YogLog.warn("known yog: items: " +
                    BuiltInRegistries.ITEM.keySet().stream()
                        .filter(i -> i.getNamespace().equals(YogNames.Ns.YOG))
                        .map(Identifier::toString).toList());
            }
            return false;
        }
        Item item = BuiltInRegistries.ITEM.getValue(id);
        p.addItem(new ItemStack(item, count));
        return true;
    }

    /** Teleport the named online player within their current world. */
    @YogServiceHost
    public static boolean teleport(String player, double x, double y, double z) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) {
            return false;
        }
        p.teleportTo(p.level(), x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        return true;
    }

    @YogServiceHost
    public static boolean kickPlayer(String playerName, String reason) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        p.connection.disconnect(Component.literal(reason));
        return true;
    }

    @YogServiceHost
    public static boolean setGamemode(String playerName, String gamemode) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        GameType mode = switch (gamemode.toLowerCase(Locale.ROOT)) {
            case "survival", "s", "0" -> GameType.SURVIVAL;
            case "creative", "c", "1" -> GameType.CREATIVE;
            case "adventure", "a", "2" -> GameType.ADVENTURE;
            case "spectator", "sp", "3" -> GameType.SPECTATOR;
            default -> null;
        };
        if (mode == null) return false;
        p.setGameMode(mode);
        return true;
    }

    @YogServiceHost
    public static boolean playerSetSlot(String playerName, int slot, String itemId, int count) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, net.minecraft.world.item.ItemStack.EMPTY);
            return true;
        }
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        inv.setItem(slot, new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.getValue(id), count));
        return true;
    }

    @YogServiceHost
    public static boolean teleportToDim(String playerName, String dimension, double x, double y, double z) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (p == null) { YogLog.warn("teleportToDim: player not found: " + playerName); return false; }
        if (w == null) { YogLog.warn("teleportToDim: no dimension " + dimension); return false; }
        p.teleportTo(w, x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        return true;
    }

    // ── held item NBT (ABI minor 11) ─────────────────────────────────────────

    /** SNBT of the item in the player's main hand, or null if offline / holding air. */
    @YogServiceHost
    public static String getHeldItemNbt(String playerName) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return null;
        net.minecraft.world.item.ItemStack stack = p.getMainHandItem();
        if (stack.isEmpty()) return null;
        return stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
    }

    /** Merge snbt into the NBT of the item in the player's main hand.
     *  Returns false if the player is offline or holding air. */
    @YogServiceHost
    public static boolean setHeldItemNbt(String playerName, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        net.minecraft.world.item.ItemStack stack = p.getMainHandItem();
        if (stack.isEmpty()) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                    net.minecraft.world.item.component.CustomData.of(nbt));
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    // ── item stack query (ABI minor 12) ──────────────────────────────────────

    /** SNBT of the item in the player's off hand, or null if offline / holding air. */
    @YogServiceHost
    public static String getOffhandItemNbt(String playerName) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return null;
        net.minecraft.world.item.ItemStack stack = p.getOffhandItem();
        if (stack.isEmpty()) return null;
        return stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
    }

    /** Merge snbt into the NBT of the player's off-hand item. */
    @YogServiceHost
    public static boolean setOffhandItemNbt(String playerName, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        net.minecraft.world.item.ItemStack stack = p.getOffhandItem();
        if (stack.isEmpty()) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                    net.minecraft.world.item.component.CustomData.of(nbt));
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** Replace inventory slot; snbt merged into new item's NBT (pass "" for no NBT). */
    @YogServiceHost
    public static boolean setSlotItem(String playerName, int slot, String itemId, int count, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, net.minecraft.world.item.ItemStack.EMPTY);
            return true;
        }
        net.minecraft.world.item.ItemStack stack = NativeBridge.buildStack(itemId, count, snbt);
        if (stack == null) return false;
        inv.setItem(slot, stack);
        return true;
    }

    /** Place the BlockItem from the player's inventory slot at a replaceable position. */
    @YogServiceHost
    public static boolean placeBlockFromSlot(String player, int slot, String dim, int x, int y, int z) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            ServerLevel w = NativeBridge.worldFor(dim);
            if (p == null || w == null || p.level() != w) return false;
            Inventory inv = p.getInventory();
            if (slot < 0 || slot >= inv.getContainerSize()) return false;
            ItemStack stack = inv.getItem(slot);
            if (stack.isEmpty() || !(stack.getItem() instanceof BlockItem bi)) return false;
            BlockPos pos = new BlockPos(x, y, z);
            if (!w.getBlockState(pos).canBeReplaced()) return false;
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.UP, pos, false);
            InteractionResult r = bi.place(new BlockPlaceContext(p, InteractionHand.MAIN_HAND, stack, hit));
            return r.consumesAction();
        } catch (Throwable t) {
            return false;
        }
    }

    @YogServiceHost
    public static boolean playerAddXp(String player, int amount) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            if (p == null) return false;
            p.giveExperiencePoints(amount);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Grant / revoke flight; revoking leaves creative / spectator players untouched. */
    @YogServiceHost
    public static boolean playerSetMayFly(String player, boolean mayFly) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            if (p == null) return false;
            GameType gm = p.gameMode.getGameModeForPlayer();
            if (!mayFly && (gm.isCreative() || gm == GameType.SPECTATOR)) return true;
            p.getAbilities().mayfly = mayFly;
            if (!mayFly) p.getAbilities().flying = false;
            p.onUpdateAbilities();
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    // ── ABI minor 3 ──────────────────────────────────────────────────────────

    /** Puts {@code itemId} on cooldown for {@code ticks} for an online player. */
    @YogServiceHost
    public static boolean setItemCooldown(String player, String itemId, int ticks) {
        ServerPlayer p = NativeBridge.playerByName(player);
        Identifier id = Identifier.tryParse(itemId);
        if (p == null || id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        p.getCooldowns().addCooldown(BuiltInRegistries.ITEM.getValue(id).getDefaultInstance(), ticks);
        return true;
    }
}
