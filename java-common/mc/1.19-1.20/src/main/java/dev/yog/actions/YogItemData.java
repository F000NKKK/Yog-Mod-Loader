package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogNames;
import dev.yog.YogServiceHost;
import dev.yog.bus.YogTypes;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;

/**
 * A held stack's Yog data ({@code saved} items, docs/design/storage.md):
 * bytes by key in its tag's {@code yog} compound (1.19–1.20: the stack's
 * own tag).
 */
public final class YogItemData {
    private YogItemData() {}

    @YogServiceHost
    public static YogTypes.Record read(String player, boolean offHand, String key) {
        ItemStack stack = held(player, offHand);
        CompoundTag tag = stack == null ? null : stack.getTag();
        if (tag == null || !tag.contains(YogNames.Ns.YOG, Tag.TAG_COMPOUND)) return null;
        CompoundTag yog = tag.getCompound(YogNames.Ns.YOG);
        return yog.contains(key, Tag.TAG_BYTE_ARRAY) ? new YogTypes.Record(yog.getByteArray(key)) : null;
    }

    @YogServiceHost
    public static boolean write(String player, boolean offHand, String key, byte[] bytes) {
        ItemStack stack = held(player, offHand);
        if (stack == null) return false;
        stack.getOrCreateTagElement(YogNames.Ns.YOG).putByteArray(key, bytes);
        return true;
    }

    /** What {@code player} holds ({@code offHand}: in its other hand); none offline or empty. */
    private static ItemStack held(String player, boolean offHand) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return null;
        ItemStack stack = p.getItemInHand(offHand ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND);
        return stack.isEmpty() ? null : stack;
    }
}
