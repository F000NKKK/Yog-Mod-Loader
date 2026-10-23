package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogPlatform;
import dev.yog.YogServiceHost;

import net.minecraft.server.level.ServerPlayer;

/** The host's net actions: the methods {@code yog_event::net} names. */
public final class YogNetActions {
    private YogNetActions() {}

    @YogServiceHost
    public static boolean sendToPlayer(String player, String channel, byte[] data) {
        ServerPlayer p = NativeBridge.playerByName(player);
        if (p == null) return false;
        YogPlatform.sendToClient(p, channel, data);
        return true;
    }
}
