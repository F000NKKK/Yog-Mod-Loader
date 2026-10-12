package dev.yog.util;

import java.util.Map;

/**
 * Datapack directory layout differences between Minecraft versions.
 *
 * <p>Minecraft 1.21 (data pack format 48, snapshot 24w21a) renamed a set of
 * datapack directories from plural to singular. Vanilla silently ignores the
 * old names, so a {@code .yog} mod written against 1.20.1 would lose its loot
 * tables, tags, recipes, ... on 1.21 and vice versa. The pack providers run
 * every {@code data/} entry through {@link #remapDataPath} so mods can use
 * either spelling and work on every supported version.
 */
public final class YogDataLayout {
    private YogDataLayout() {
    }

    /** Top-level {@code data/<ns>/<dir>/} renames (plural, pre-1.21 → singular, 1.21+). */
    private static final Map<String, String> TOP_LEVEL = Map.of(
            "advancements", "advancement",
            "functions", "function",
            "item_modifiers", "item_modifier",
            "loot_tables", "loot_table",
            "predicates", "predicate",
            "recipes", "recipe",
            "structures", "structure");

    /** {@code data/<ns>/tags/<dir>/} renames (plural, pre-1.21 → singular, 1.21+). */
    private static final Map<String, String> TAGS = Map.of(
            "blocks", "block",
            "entity_types", "entity_type",
            "fluids", "fluid",
            "functions", "function",
            "game_events", "game_event",
            "items", "item");

    private static final Map<String, String> TOP_LEVEL_REV = invert(TOP_LEVEL);
    private static final Map<String, String> TAGS_REV = invert(TAGS);

    /**
     * Rewrite a pack-relative path ({@code data/<ns>/...}) to the directory
     * names the running game expects. Non-{@code data/} paths and directories
     * that weren't renamed are returned unchanged.
     *
     * @param path         pack-relative path with {@code /} separators
     * @param singularDirs {@code true} for MC 1.21+ (singular names),
     *                     {@code false} for 1.20.x (plural names)
     */
    public static String remapDataPath(String path, boolean singularDirs) {
        if (!path.startsWith("data/")) {
            return path;
        }
        // data / <ns> / <dir> / <rest...>
        String[] parts = path.split("/", 5);
        if (parts.length < 4) {
            return path;
        }
        Map<String, String> top = singularDirs ? TOP_LEVEL : TOP_LEVEL_REV;
        String mapped = top.get(parts[2]);
        if (mapped != null) {
            parts[2] = mapped;
            return String.join("/", parts);
        }
        if (parts[2].equals("tags") && parts.length == 5) {
            Map<String, String> tags = singularDirs ? TAGS : TAGS_REV;
            String tag = tags.get(parts[3]);
            if (tag != null) {
                parts[3] = tag;
                return String.join("/", parts);
            }
        }
        return path;
    }

    /** Name of the recipe directory for the running game version. */
    public static String recipeDir(boolean singularDirs) {
        return singularDirs ? "recipe" : "recipes";
    }

    private static Map<String, String> invert(Map<String, String> m) {
        java.util.HashMap<String, String> out = new java.util.HashMap<>();
        m.forEach((k, v) -> out.put(v, k));
        return Map.copyOf(out);
    }
}
