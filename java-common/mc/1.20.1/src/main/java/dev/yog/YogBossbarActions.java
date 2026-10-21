package dev.yog;

import java.util.Locale;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.network.chat.Component;

/** The host's bossbar actions: the methods {@code yog_event::bossbar} names. */
public final class YogBossbarActions {
    private YogBossbarActions() {}

    @YogServiceHost
    public static boolean bossbarCreate(String id, String title, String color, String style) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        ResourceLocation bid = ResourceLocation.tryParse(id);
        if (bid == null) return false;
        net.minecraft.server.bossevents.CustomBossEvents mgr = s.getCustomBossEvents();
        if (mgr.get(bid) != null) return false;
        net.minecraft.server.bossevents.CustomBossEvent bar = mgr.create(bid, Component.literal(title));
        bar.setColor(parseBossBarColor(color));
        bar.setOverlay(parseBossBarStyle(style));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarRemove(String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        ResourceLocation bid = ResourceLocation.tryParse(id);
        if (bid == null) return false;
        net.minecraft.server.bossevents.CustomBossEvents mgr = s.getCustomBossEvents();
        net.minecraft.server.bossevents.CustomBossEvent bar = mgr.get(bid);
        if (bar == null) return false;
        mgr.remove(bar);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetTitle(String id, String title) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setName(Component.literal(title));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetProgress(String id, float progress) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setProgress(Math.max(0f, Math.min(1f, progress)));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetColor(String id, String color) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setColor(parseBossBarColor(color));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarAddPlayer(String id, String playerName) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.addPlayer(p);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarRemovePlayer(String id, String playerName) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.removePlayer(p);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetVisible(String id, boolean visible) {
        net.minecraft.server.bossevents.CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setVisible(visible);
        return true;
    }

    private static net.minecraft.server.bossevents.CustomBossEvent getBossBar(String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return null;
        ResourceLocation bid = ResourceLocation.tryParse(id);
        return bid == null ? null : s.getCustomBossEvents().get(bid);
    }

    private static net.minecraft.world.BossEvent.BossBarColor parseBossBarColor(String color) {
        return switch (color.toLowerCase(Locale.ROOT)) {
            case "pink"   -> net.minecraft.world.BossEvent.BossBarColor.PINK;
            case "blue"   -> net.minecraft.world.BossEvent.BossBarColor.BLUE;
            case "red"    -> net.minecraft.world.BossEvent.BossBarColor.RED;
            case "green"  -> net.minecraft.world.BossEvent.BossBarColor.GREEN;
            case "yellow" -> net.minecraft.world.BossEvent.BossBarColor.YELLOW;
            case "purple" -> net.minecraft.world.BossEvent.BossBarColor.PURPLE;
            default       -> net.minecraft.world.BossEvent.BossBarColor.WHITE;
        };
    }

    private static net.minecraft.world.BossEvent.BossBarOverlay parseBossBarStyle(String style) {
        return switch (style.toLowerCase(Locale.ROOT)) {
            case "notched_6"  -> net.minecraft.world.BossEvent.BossBarOverlay.NOTCHED_6;
            case "notched_10" -> net.minecraft.world.BossEvent.BossBarOverlay.NOTCHED_10;
            case "notched_12" -> net.minecraft.world.BossEvent.BossBarOverlay.NOTCHED_12;
            case "notched_20" -> net.minecraft.world.BossEvent.BossBarOverlay.NOTCHED_20;
            default           -> net.minecraft.world.BossEvent.BossBarOverlay.PROGRESS;
        };
    }
}
