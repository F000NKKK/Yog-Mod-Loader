package dev.yog;

import java.util.Locale;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Item;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.network.chat.Component;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.GameType;
import net.minecraft.core.Direction;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/** The host's player actions: the methods {@code yog_event::player} names. */
public final class YogPlayerActions {
    private YogPlayerActions() {}

    @YogServiceHost
    public static boolean giveItem(String player, String itemId, int count) {
        ServerPlayer p = NativeBridge.playerByName(player);
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (p == null || id == null || count <= 0) return false;
        if (!BuiltInRegistries.ITEM.containsKey(id)) return false;
        Item item = BuiltInRegistries.ITEM.get(id);
        p.addItem(new ItemStack(item, count));
        return true;
    }

    @YogServiceHost
    public static boolean teleport(String player, double x, double y, double z) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return false;
        p.teleportTo(p.serverLevel(), x, y, z, p.getYRot(), p.getXRot());
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
        if (slot < 0 || slot >= inv.items.size()) return false;
        if (count <= 0) { inv.items.set(slot, ItemStack.EMPTY); return true; }
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        inv.items.set(slot, new ItemStack(BuiltInRegistries.ITEM.get(id), count));
        return true;
    }

    @YogServiceHost
    public static boolean teleportToDim(String playerName, String dimension, double x, double y, double z) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (p == null) { YogLog.warn("teleportToDim: player not found: " + playerName); return false; }
        if (w == null) { YogLog.warn("teleportToDim: no dimension " + dimension); return false; }
        p.teleportTo(w, x, y, z, p.getYRot(), p.getXRot());
        return true;
    }

    /** Place the BlockItem from the player's inventory slot at a replaceable position. */
    @YogServiceHost
    public static boolean placeBlockFromSlot(String player, int slot, String dim, int x, int y, int z) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            ServerLevel w = NativeBridge.worldFor(dim);
            if (p == null || w == null || p.serverLevel() != w) return false;
            Inventory inv = p.getInventory();
            if (slot < 0 || slot >= inv.items.size()) return false;
            ItemStack stack = inv.items.get(slot);
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

    // ── held / slot item NBT ────────────────────────────────────────────────

    /** SNBT of the item in the player's main hand, or null if offline / holding air. */
    @YogServiceHost
    public static String getHeldItemNbt(String playerName) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return null;
        ItemStack stack = p.getMainHandItem();
        if (stack.isEmpty()) return null;
        return NativeBridge.stackSnbt(stack);
    }

    /** Replace the NBT tag of the item in the player's main hand with snbt. */
    @YogServiceHost
    public static boolean setHeldItemNbt(String playerName, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        return setStackSnbt(p.getMainHandItem(), snbt);
    }

    /** SNBT of the item in the player's off hand, or null if offline / holding air. */
    @YogServiceHost
    public static String getOffhandItemNbt(String playerName) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return null;
        ItemStack stack = p.getOffhandItem();
        if (stack.isEmpty()) return null;
        return NativeBridge.stackSnbt(stack);
    }

    /** Replace the NBT tag of the player's off-hand item with snbt. */
    @YogServiceHost
    public static boolean setOffhandItemNbt(String playerName, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        return setStackSnbt(p.getOffhandItem(), snbt);
    }

    private static boolean setStackSnbt(ItemStack stack, String snbt) {
        if (stack.isEmpty()) return false;
        try {
            CompoundTag nbt = TagParser.parseTag(snbt);
            stack.setTag(nbt);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** Replace inventory slot; snbt becomes the new item's NBT tag (pass "" for none). */
    @YogServiceHost
    public static boolean setSlotItem(String playerName, int slot, String itemId, int count, String snbt) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, ItemStack.EMPTY);
            return true;
        }
        ItemStack stack = NativeBridge.buildStack(itemId, count, snbt);
        if (stack == null) return false;
        inv.setItem(slot, stack);
        return true;
    }

    // ── ABI minor 3 ──────────────────────────────────────────────────────────

    /** Puts {@code itemId} on cooldown for {@code ticks} for an online player. */
    @YogServiceHost
    public static boolean setItemCooldown(String player, String itemId, int ticks) {
        ServerPlayer p = NativeBridge.playerByName(player);
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (p == null || id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        p.getCooldowns().addCooldown(BuiltInRegistries.ITEM.get(id), ticks);
        return true;
    }
}
