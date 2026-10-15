package dev.yog.util;

import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Entities' own hitboxes ({@code entity_set_dimensions}): width, height and
 * eye height in blocks, kept apart for the server and the client (one JVM in
 * single player), applied whenever the game works out an entity's size.
 */
public final class YogDimensions {
    private YogDimensions() {
    }

    private static final Map<UUID, float[]> SERVER = new ConcurrentHashMap<>();
    private static final Map<UUID, float[]> CLIENT = new ConcurrentHashMap<>();

    /** Sets (width ≥ 0) or clears an entity's size on one side. */
    public static void set(boolean client, UUID id, float width, float height, float eye) {
        Map<UUID, float[]> m = client ? CLIENT : SERVER;
        if (width < 0) m.remove(id); else m.put(id, new float[] {width, height, eye});
    }

    /** {width, height, eye} of the entity on that side, or null for vanilla's. */
    public static float[] get(boolean client, UUID id) {
        Map<UUID, float[]> m = client ? CLIENT : SERVER;
        return m.isEmpty() ? null : m.get(id);
    }
}
