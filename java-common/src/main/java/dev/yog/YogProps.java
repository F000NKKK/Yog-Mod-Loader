package dev.yog;

import java.util.HashMap;
import java.util.Map;

/**
 * Parsing helpers for the `key=value`-per-tab-separated-field lines the
 * native runtime emits for block/item defs (`nativeBlockDefs`,
 * `nativeItemDefs`, ...). Pure Java, no Minecraft types — identical under
 * Yarn and Mojang mappings, so every `YogHost` (Fabric/Forge/NeoForge ×
 * 1.20.1/1.21.1) shares this instead of keeping 6 copies in sync by hand.
 */
public final class YogProps {
    private YogProps() {}

    /** Parse `id\tkey=value\tkey=value...` into a map. The id itself (index 0) is not included. */
    public static Map<String, String> parse(String line) {
        String[] parts = line.split("\t", -1);
        Map<String, String> props = new HashMap<>();
        for (int i = 1; i < parts.length; i++) {
            int eq = parts[i].indexOf('=');
            if (eq > 0) props.put(parts[i].substring(0, eq), parts[i].substring(eq + 1));
        }
        return props;
    }

    public static int parseInt(Map<String, String> p, String key, int def) {
        String v = p.get(key);
        if (v == null) return def;
        try {
            return Integer.parseInt(v);
        } catch (NumberFormatException e) {
            return def;
        }
    }

    public static float parseFloat(Map<String, String> p, String key, float def) {
        String v = p.get(key);
        if (v == null) return def;
        try {
            return Float.parseFloat(v);
        } catch (NumberFormatException e) {
            return def;
        }
    }
}
