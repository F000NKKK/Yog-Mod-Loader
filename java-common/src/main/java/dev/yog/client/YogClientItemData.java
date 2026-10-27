package dev.yog.client;

import dev.yog.YogServiceHost;
import dev.yog.actions.YogItemData;
import dev.yog.bus.YogTypes;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.InteractionHand;

/**
 * A held stack's Yog data on this client ({@code saved} and {@code synced}
 * items, docs/design/storage.md): the server's, as the game sent the stack.
 */
public final class YogClientItemData {
    private YogClientItemData() {}

    @YogServiceHost
    public static YogTypes.Record read(boolean offHand, String key) {
        LocalPlayer p = Minecraft.getInstance().player;
        if (p == null) return null;
        return YogItemData.of(p.getItemInHand(offHand ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND), key);
    }
}
