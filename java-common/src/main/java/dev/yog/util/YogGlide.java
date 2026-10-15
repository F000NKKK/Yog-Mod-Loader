package dev.yog.util;

import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Players that may glide as if wearing an elytra ({@code entity_set_glider}).
 * The server and the client keep their own sets — in single player both
 * live in one JVM — and the mod tells the client ({@code set_glider}).
 */
public final class YogGlide {
    private YogGlide() {
    }

    /** Server side: who may glide. */
    private static final Set<UUID> SERVER = ConcurrentHashMap.newKeySet();
    /** Client side: who may glide, with their wings' texture ("" for none). */
    private static final Map<UUID, String> CLIENT = new ConcurrentHashMap<>();

    public static void setServer(UUID player, boolean glide) {
        if (glide) SERVER.add(player); else SERVER.remove(player);
    }

    public static void setClient(UUID player, boolean glide, String wings) {
        if (glide) CLIENT.put(player, wings == null ? "" : wings); else CLIENT.remove(player);
    }

    /** Whether {@code player} may glide, on the side {@code client} says. */
    public static boolean can(UUID player, boolean client) {
        return client ? CLIENT.containsKey(player) : SERVER.contains(player);
    }

    /** The texture of the player's wings on the client, or null to draw none. */
    public static String wings(UUID player) {
        String w = CLIENT.get(player);
        return w == null || w.isEmpty() ? null : w;
    }
}
