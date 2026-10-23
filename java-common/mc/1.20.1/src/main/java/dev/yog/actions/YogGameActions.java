package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogPlatform;
import dev.yog.YogServiceHost;

import net.minecraft.server.MinecraftServer;

/** The host's game actions: the methods {@code yog_event::game} names. */
public final class YogGameActions {
    private YogGameActions() {}

    @YogServiceHost
    public static String gameDir() {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return YogPlatform.gameDir().toAbsolutePath().toString();
        return s.getServerDirectory().getAbsolutePath();
    }

    /** The directory of the world being played (its save): where mods keep what belongs to it. */
    @YogServiceHost
    public static String worldDir() {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return null;
        return s.getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT).toAbsolutePath().normalize().toString();
    }

    // ── translations (consumed by the runtime's translate ABI) ─────────────

    /**
     * The text of translation key {@code key} in the game's current
     * language (lang files of every resource pack, .yog mods' included), or
     * null when no language defines it. Format specifiers are left for the
     * caller (the runtime fills them in).
     */
    @YogServiceHost
    public static String translate(String key) {
        net.minecraft.locale.Language lang = net.minecraft.locale.Language.getInstance();
        return lang.has(key) ? lang.getOrDefault(key) : null;
    }
}
