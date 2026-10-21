package dev.yog;

import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerPlayer;

/** The host's net actions: the methods {@code yog_event::net} names. */
public final class YogNetActions {
    private YogNetActions() {}

    /** Send a raw-byte packet to a player on a channel (server -> client). */
    @YogServiceHost
    public static boolean sendToPlayer(String player, String channel, byte[] data) {
        ServerPlayer p = NativeBridge.playerByName(player);
        Identifier id = Identifier.tryParse(channel);
        if (p == null || id == null) {
            return false;
        }
        YogPlatform.sendToClient(p, channel, data);
        return true;
    }
}
