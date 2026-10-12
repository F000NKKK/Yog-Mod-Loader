package dev.yog.util;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
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

    /** {@code true} for {@code data/<ns>/recipe/**.json} (after {@link #remapDataPath}). */
    public static boolean isRecipePath(String path) {
        String[] parts = path.split("/", 4);
        return parts.length == 4 && parts[0].equals("data")
                && (parts[2].equals("recipe") || parts[2].equals("recipes"))
                && path.endsWith(".json");
    }

    /**
     * Upgrade a 1.20-style recipe JSON to the 1.20.5+ item stack shape that
     * 1.21 requires: {@code "result":{"item":X,"count":N}} becomes
     * {@code {"id":X,"count":N}}, and a bare {@code "result":"X"} (smelting,
     * stonecutting — with its top-level {@code "count"}) becomes an object.
     * Ingredients keep {@code "item"} — that changed only in 1.21.2.
     * Anything already in the new shape or unparseable is returned as is.
     */
    public static String upgradeRecipeJson(String json) {
        JsonObject root;
        try {
            JsonElement parsed = JsonParser.parseString(json);
            if (!parsed.isJsonObject()) {
                return json;
            }
            root = parsed.getAsJsonObject();
        } catch (RuntimeException e) {
            return json;
        }
        JsonElement result = root.get("result");
        if (result == null) {
            return json;
        }
        if (result.isJsonPrimitive() && result.getAsJsonPrimitive().isString()) {
            JsonObject stack = new JsonObject();
            stack.addProperty("id", result.getAsString());
            JsonElement count = root.remove("count");
            if (count != null) {
                stack.add("count", count);
            }
            root.add("result", stack);
        } else if (result.isJsonObject()) {
            JsonObject stack = result.getAsJsonObject();
            if (stack.has("id") || !stack.has("item")) {
                return json;
            }
            stack.add("id", stack.remove("item"));
        } else {
            return json;
        }
        return root.toString();
    }

    private static Map<String, String> invert(Map<String, String> m) {
        java.util.HashMap<String, String> out = new java.util.HashMap<>();
        m.forEach((k, v) -> out.put(v, k));
        return Map.copyOf(out);
    }
}
