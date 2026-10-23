package dev.yog.client;

import dev.yog.YogServiceHost;

import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;

/** This player's own chat: lines only it sees (a client bot's replies). */
public final class YogClientChat {
    private YogClientChat() {}

    @YogServiceHost
    public static boolean message(String text) {
        Minecraft mc = Minecraft.getInstance();
        if (mc.player == null) return false;
        mc.player.sendSystemMessage(Component.literal(text));
        return true;
    }
}
