package dev.yog;

import java.util.Locale;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.bossevents.CustomBossEvent;
import net.minecraft.server.bossevents.CustomBossEvents;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.BossEvent;

/** The host's bossbar actions: the methods {@code yog_event::bossbar} names. */
public final class YogBossbarActions {
    private YogBossbarActions() {}

    // ── boss bar ────────────────────────────────────────────────────────────

    @YogServiceHost
    public static boolean bossbarCreate(String id, String title, String color, String style) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        CustomBossEvents mgr = s.getCustomBossEvents();
        if (mgr.get(bid) != null) return false;
        CustomBossEvent bar = mgr.create(s.overworld().getRandom(), bid, Component.literal(title));
        bar.setColor(parseBossBarColor(color));
        bar.setOverlay(parseBossBarStyle(style));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarRemove(String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        CustomBossEvents mgr = s.getCustomBossEvents();
        CustomBossEvent bar = mgr.get(bid);
        if (bar == null) return false;
        mgr.remove(bar);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetTitle(String id, String title) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setName(Component.literal(title));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetProgress(String id, float progress) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setProgress(Math.max(0f, Math.min(1f, progress)));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetColor(String id, String color) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setColor(parseBossBarColor(color));
        return true;
    }

    @YogServiceHost
    public static boolean bossbarAddPlayer(String id, String playerName) {
        CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.addPlayer(p);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarRemovePlayer(String id, String playerName) {
        CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = NativeBridge.playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.removePlayer(p);
        return true;
    }

    @YogServiceHost
    public static boolean bossbarSetVisible(String id, boolean visible) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setVisible(visible);
        return true;
    }

    private static CustomBossEvent getBossBar(String id) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return null;
        Identifier bid = Identifier.tryParse(id);
        return bid == null ? null : s.getCustomBossEvents().get(bid);
    }

    private static BossEvent.BossBarColor parseBossBarColor(String color) {
        return switch (color.toLowerCase(Locale.ROOT)) {
            case "pink"   -> BossEvent.BossBarColor.PINK;
            case "blue"   -> BossEvent.BossBarColor.BLUE;
            case "red"    -> BossEvent.BossBarColor.RED;
            case "green"  -> BossEvent.BossBarColor.GREEN;
            case "yellow" -> BossEvent.BossBarColor.YELLOW;
            case "purple" -> BossEvent.BossBarColor.PURPLE;
            default       -> BossEvent.BossBarColor.WHITE;
        };
    }

    private static BossEvent.BossBarOverlay parseBossBarStyle(String style) {
        return switch (style.toLowerCase(Locale.ROOT)) {
            case "notched_6"  -> BossEvent.BossBarOverlay.NOTCHED_6;
            case "notched_10" -> BossEvent.BossBarOverlay.NOTCHED_10;
            case "notched_12" -> BossEvent.BossBarOverlay.NOTCHED_12;
            case "notched_20" -> BossEvent.BossBarOverlay.NOTCHED_20;
            default           -> BossEvent.BossBarOverlay.PROGRESS;
        };
    }
}
