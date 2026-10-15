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

    /**
     * A recipe in Yog's own format — what {@code .yrecipe} files say:
     * {@code shaped}, {@code shapeless}, {@code smelting}, {@code blasting},
     * {@code smoking}, {@code campfire_cooking}, {@code stonecutting},
     * {@code smithing}; an ingredient is an item id, a {@code #tag} or an
     * array of those (any of them) — as the vanilla recipe JSON of this
     * Minecraft version ({@code mc121}: 1.21's {@code "id"} results). JSON of
     * another mod's type ({@code create:mixing}) is passed on (upgraded to
     * 1.21 there); the mod's own kind ({@code <namespace>:…}) is its data,
     * not the game's: {@code null}, nothing to write.
     */
    public static String yogRecipeToVanilla(String json, String namespace, boolean mc121) {
        JsonObject in;
        try {
            in = JsonParser.parseString(json).getAsJsonObject();
        } catch (Exception e) {
            return json;
        }
        return yogRecipeToVanilla(in, namespace, mc121);
    }

    /** An item a recipe names that this version of the game doesn't have (under any name). */
    private static final class Missing extends RuntimeException {
        Missing(String id) {
            super(id, null, false, false);
        }
    }

    /** {@code id} as this version calls it ({@link dev.yog.data.YogVersions}), or {@link Missing}. */
    private static String item(String id) {
        String here = dev.yog.data.YogVersions.item(id);
        if (here == null) throw new Missing(id);
        return here;
    }

    /** {@link #yogRecipeToVanilla(String, String, boolean)} of a recipe already a JSON tree. */
    public static String yogRecipeToVanilla(JsonObject in, String namespace, boolean mc121) {
        try {
            return toVanilla(in, namespace, mc121);
        } catch (Missing m) {
            // Content of another version: this one hasn't got it, so neither the recipe.
            System.err.println("[yog] recipe of " + namespace + " left out: " + m.getMessage() + " isn't in this version of Minecraft");
            return null;
        }
    }

    private static String toVanilla(JsonObject in, String namespace, boolean mc121) {
        String type = in.has("type") ? in.get("type").getAsString() : "";
        if (type.contains(":")) {
            // The mod's own kind of recipe (`mymod:infusion`): its data, for
            // its machines and books — not the game's.
            if (type.startsWith(namespace + ":")) return null;
            // Another mod's (a Java mod's `create:mixing`): as written.
            String json = in.toString();
            return mc121 ? upgradeRecipeJson(json) : json;
        }
        JsonObject out = new JsonObject();
        String result = in.has("result") ? in.get("result").getAsString() : "minecraft:air";
        int count = in.has("count") ? in.get("count").getAsInt() : 1;
        switch (type) {
            case "shaped" -> {
                out.addProperty("type", "minecraft:crafting_shaped");
                out.add("pattern", in.get("pattern"));
                JsonObject key = new JsonObject();
                for (Map.Entry<String, JsonElement> e : in.getAsJsonObject("key").entrySet()) {
                    key.add(e.getKey(), ingredient(e.getValue()));
                }
                out.add("key", key);
                out.add("result", stack(result, count, mc121));
            }
            case "shapeless" -> {
                out.addProperty("type", "minecraft:crafting_shapeless");
                com.google.gson.JsonArray list = new com.google.gson.JsonArray();
                for (JsonElement e : in.getAsJsonArray("ingredients")) list.add(ingredient(e));
                out.add("ingredients", list);
                out.add("result", stack(result, count, mc121));
            }
            case "smelting", "blasting", "smoking", "campfire_cooking" -> {
                out.addProperty("type", "minecraft:" + type);
                out.add("ingredient", ingredient(in.get("ingredient")));
                if (mc121) out.add("result", stack(result, count, true));
                else out.addProperty("result", result);
                out.addProperty("experience", in.has("experience") ? in.get("experience").getAsFloat() : 0.1f);
                out.addProperty("cookingtime", in.has("cook_time") ? in.get("cook_time").getAsInt() : 200);
            }
            case "stonecutting" -> {
                out.addProperty("type", "minecraft:stonecutting");
                out.add("ingredient", ingredient(in.get("ingredient")));
                if (mc121) {
                    out.add("result", stack(result, count, true));
                } else {
                    out.addProperty("result", result);
                    out.addProperty("count", count);
                }
            }
            case "smithing" -> {
                out.addProperty("type", "minecraft:smithing_transform");
                out.add("template", ingredient(in.get("template")));
                out.add("base", ingredient(in.get("base")));
                out.add("addition", ingredient(in.get("addition")));
                out.add("result", stack(result, count, mc121));
            }
            default -> {
                return in.toString();
            }
        }
        if (in.has("group")) out.addProperty("group", in.get("group").getAsString());
        if (in.has("category")) out.addProperty("category", in.get("category").getAsString());
        return out.toString();
    }

    /** An item id, a {@code #tag}, or an array of those (any of them), as a vanilla ingredient. */
    private static JsonElement ingredient(JsonElement v) {
        if (v.isJsonArray()) {
            // Alternatives: those this version has; the recipe needs one.
            com.google.gson.JsonArray any = new com.google.gson.JsonArray();
            Missing missing = null;
            for (JsonElement e : v.getAsJsonArray()) {
                try {
                    any.add(ingredient(e));
                } catch (Missing m) {
                    missing = m;
                }
            }
            if (any.isEmpty() && missing != null) throw missing;
            return any.size() == 1 ? any.get(0) : any;
        }
        String item = v.getAsString();
        JsonObject o = new JsonObject();
        if (item.startsWith("#")) {
            String tag = item.substring(1);
            if (!dev.yog.data.YogTags.has("item", tag)) throw new Missing("#" + tag);
            o.addProperty("tag", tag);
        } else {
            o.addProperty("item", item(item));
        }
        return o;
    }

    /** A result stack: {@code {"item": …}} before 1.21, {@code {"id": …}} since. */
    private static JsonObject stack(String id, int count, boolean mc121) {
        JsonObject o = new JsonObject();
        o.addProperty(mc121 ? "id" : "item", item(id));
        if (count != 1) o.addProperty("count", count);
        return o;
    }
}
