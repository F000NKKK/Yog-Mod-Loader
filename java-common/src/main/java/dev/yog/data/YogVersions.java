package dev.yog.data;

import dev.yog.YogLog;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Predicate;

/**
 * Vanilla content across Minecraft versions: a mod names it as its
 * datapack version does ({@code [mod] datapack}); on a version where it's
 * called otherwise ({@code short_grass} is {@code grass} on 1.20.1) the
 * other name is used — {@code yog/renames.txt}, the same table
 * {@code yog build} checks with.
 */
public final class YogVersions {
    private YogVersions() {}

    /** Whether an item id is in the game — set by the host. */
    public static volatile Predicate<String> itemExists = id -> true;

    /** {@code minecraft:<a name>} → {@code minecraft:<the other>}. */
    private static final Map<String, String> RENAMED = load();

    private static Map<String, String> load() {
        Map<String, String> out = new HashMap<>();
        try (InputStream in = YogVersions.class.getResourceAsStream("/yog/renames.txt")) {
            if (in == null) return out;
            BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
            for (String line; (line = r.readLine()) != null; ) {
                if (line.startsWith("#") || line.isBlank()) continue;
                String[] p = line.trim().split("\\s+");
                if (p.length < 3) continue;
                out.put("minecraft:" + p[1], "minecraft:" + p[2]);
                out.put("minecraft:" + p[2], "minecraft:" + p[1]);
            }
        } catch (Exception e) {
            YogLog.warn("yog/renames.txt: " + e);
        }
        return out;
    }

    /** What item {@code id} is on this version: itself, its other name, or null when there's none. */
    public static String item(String id) {
        if (itemExists.test(id)) return id;
        String other = RENAMED.get(id);
        return other != null && itemExists.test(other) ? other : null;
    }
}
