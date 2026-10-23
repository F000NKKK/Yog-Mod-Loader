package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;

import net.minecraft.network.protocol.game.ClientboundSetActionBarTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.network.chat.Component;

/** The host's chat actions: the methods {@code yog_event::chat} names. */
public final class YogChatActions {
    private YogChatActions() {}

    /** Broadcast a chat message to all players. */
    @YogServiceHost
    public static void broadcast(String message) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s != null) {
            s.execute(() -> s.getPlayerList().broadcastSystemMessage(Component.literal(message), false));
        }
    }

    @YogServiceHost
    public static boolean sendTitle(String playerName, String title, String subtitle, int fadein, int stay, int fadeout) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        p.connection.send(new ClientboundSetTitlesAnimationPacket(fadein, stay, fadeout));
        if (!title.isEmpty()) p.connection.send(new ClientboundSetTitleTextPacket(Component.literal(title)));
        if (!subtitle.isEmpty()) p.connection.send(new ClientboundSetSubtitleTextPacket(Component.literal(subtitle)));
        return true;
    }

    @YogServiceHost
    public static boolean sendActionbar(String playerName, String message) {
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (p == null) return false;
        p.connection.send(new ClientboundSetActionBarTextPacket(Component.literal(message)));
        return true;
    }

    /** A system chat message from a JSON text component (the /tellraw format). */
    @YogServiceHost
    public static boolean tellraw(String player, String json) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            if (p == null) return false;
            Component text = Component.Serializer.fromJson(json, p.registryAccess());
            if (text == null) return false;
            p.sendSystemMessage(text);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    // ── ABI minor 2 ──────────────────────────────────────────────────────────

    /** An action-bar message from a JSON text component. */
    @YogServiceHost
    public static boolean actionbarJson(String player, String json) {
        try {
            ServerPlayer p = NativeBridge.playerByName(player);
            if (p == null) return false;
            Component text = Component.Serializer.fromJson(json, p.registryAccess());
            if (text == null) return false;
            p.connection.send(new ClientboundSetActionBarTextPacket(text));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
