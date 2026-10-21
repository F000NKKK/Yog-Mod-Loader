package dev.yog.util;

import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.YogNames.GameRecipe;
import dev.yog.YogNames.RecipeField;
import dev.yog.YogNames.RecipeKind;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.util.List;
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
    private static final Map<String, String> TOP_LEVEL = renamed(YogNames.RenamedDataDir.ALL_VALUES);

    /** {@code data/<ns>/tags/<dir>/} renames (plural, pre-1.21 → singular, 1.21+). */
    private static final Map<String, String> TAGS = renamed(YogNames.RenamedTagDir.ALL_VALUES);

    /** Each folder's old name → its name. */
    private static Map<String, String> renamed(java.util.List<String> dirs) {
        java.util.HashMap<String, String> out = new java.util.HashMap<>();
        for (String d : dirs) out.put(d + YogNames.DataLayout.PLURAL, d);
        return Map.copyOf(out);
    }

    /** What a pack's data paths start with: {@code data/}. */
    private static final String DATA_DIR = YogNames.Pack.DATA + YogNames.Sep.PATH;

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
        if (!path.startsWith(DATA_DIR)) {
            return path;
        }
        // data / <ns> / <dir> / <rest...>
        String[] parts = path.split(YogNames.Sep.PATH, 5);
        if (parts.length < 4) {
            return path;
        }
        Map<String, String> top = singularDirs ? TOP_LEVEL : TOP_LEVEL_REV;
        String mapped = top.get(parts[2]);
        if (mapped != null) {
            parts[2] = mapped;
            return String.join(YogNames.Sep.PATH, parts);
        }
        if (parts[2].equals(YogNames.Pack.TAGS) && parts.length == 5) {
            Map<String, String> tags = singularDirs ? TAGS : TAGS_REV;
            String tag = tags.get(parts[3]);
            if (tag != null) {
                parts[3] = tag;
                return String.join(YogNames.Sep.PATH, parts);
            }
        }
        return path;
    }

    /** The recipes' folder's name since 1.21. */
    private static final String RECIPE = YogNames.RenamedDataDir.RECIPE;

    /** Name of the recipe directory for the running game version. */
    public static String recipeDir(boolean singularDirs) {
        return singularDirs ? RECIPE : RECIPE + YogNames.DataLayout.PLURAL;
    }

    /** {@code true} for {@code data/<ns>/recipe/**.json} (after {@link #remapDataPath}). */
    public static boolean isRecipePath(String path) {
        String[] parts = path.split(YogNames.Sep.PATH, 4);
        return parts.length == 4 && parts[0].equals(YogNames.Pack.DATA)
                && (parts[2].equals(RECIPE) || parts[2].equals(RECIPE + YogNames.DataLayout.PLURAL))
                && path.endsWith(YogNames.Res.JSON);
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
        upgradeIngredients(root);
        JsonElement result = root.get(RecipeField.RESULT);
        if (result == null) {
            return root.toString();
        }
        if (result.isJsonPrimitive() && result.getAsJsonPrimitive().isString()) {
            JsonObject stack = new JsonObject();
            stack.addProperty(GameRecipe.ID, result.getAsString());
            JsonElement count = root.remove(RecipeField.COUNT);
            if (count != null) {
                stack.add(RecipeField.COUNT, count);
            }
            root.add(RecipeField.RESULT, stack);
        } else if (result.isJsonObject()) {
            JsonObject stack = result.getAsJsonObject();
            if (stack.has(GameRecipe.ID) || !stack.has(GameRecipe.ITEM)) {
                return root.toString();
            }
            stack.add(GameRecipe.ID, stack.remove(GameRecipe.ITEM));
        }
        return root.toString();
    }

    /** The first version whose ingredients are bare ids and {@code #tags} (not {@code {"item": …}} objects). */
    private static final int[] ID_INGREDIENTS_SINCE = {1, 21, 2};
    /** The fields of a recipe that hold ingredients: a map of them, a list of them, or one. */
    private static final String KEY = RecipeField.KEY, INGREDIENTS = RecipeField.INGREDIENTS;
    private static final java.util.List<String> ONE_INGREDIENT = java.util.List.of(RecipeField.INGREDIENT, RecipeField.TEMPLATE, RecipeField.BASE, RecipeField.ADDITION);
    private static final String ITEM = GameRecipe.ITEM, TAG = GameRecipe.TAG, TAG_MARK = YogNames.Sep.TAG;
    /** What a cooking recipe gives and takes when it doesn't say: the game's furnace's. */
    private static final float COOKING_EXPERIENCE = 0.1f;
    private static final int COOKING_TICKS = 200;

    /** Whether this host's game takes ingredients as bare ids (1.21.2+); set by {@link #setVersion}. */
    private static boolean idIngredients;

    /** The game version recipes are written for ({@code 1.21.1}, {@code 26.2}). */
    public static void setVersion(String mcVersion) {
        idIngredients = atLeast(mcVersion, ID_INGREDIENTS_SINCE);
    }

    /** Whether {@code version} is {@code since} or newer. */
    static boolean atLeast(String version, int[] since) {
        String[] parts = version.split("\\.");
        for (int i = 0; i < since.length; i++) {
            int v;
            try {
                v = i < parts.length ? Integer.parseInt(parts[i]) : 0;
            } catch (NumberFormatException e) {
                return false;
            }
            if (v != since[i]) return v > since[i];
        }
        return true;
    }

    /** A recipe's ingredients as this version writes them: on 1.21.2+, {@code {"item": x}} → {@code "x"}, {@code {"tag": t}} → {@code "#t"}. */
    private static void upgradeIngredients(JsonObject recipe) {
        if (!idIngredients) return;
        if (recipe.get(KEY) instanceof JsonObject key) {
            for (Map.Entry<String, JsonElement> e : key.entrySet()) e.setValue(asId(e.getValue()));
        }
        if (recipe.get(INGREDIENTS) instanceof com.google.gson.JsonArray list) {
            for (int i = 0; i < list.size(); i++) list.set(i, asId(list.get(i)));
        }
        for (String field : ONE_INGREDIENT) {
            if (recipe.has(field)) recipe.add(field, asId(recipe.get(field)));
        }
    }

    /**
     * One ingredient as a bare id or {@code #tag}. Alternatives are a list of
     * ids only (1.21.2+ takes a tag alone, not in a list): with a tag among
     * them the tag stands for them all — Yog's alternatives name an item and
     * the common tag it's in ({@code [copper_ingot, #yog:ingots/copper]}).
     */
    private static JsonElement asId(JsonElement v) {
        if (v instanceof com.google.gson.JsonArray any) {
            com.google.gson.JsonArray out = new com.google.gson.JsonArray();
            JsonElement tag = null;
            for (JsonElement e : any) {
                JsonElement id = asId(e);
                if (id.isJsonPrimitive() && id.getAsString().startsWith(TAG_MARK)) {
                    if (tag == null) tag = id;
                    else if (!tag.equals(id)) YogLog.warn("ingredient " + any + ": only " + tag + " of its tags is kept (a list holds no tags)");
                } else {
                    out.add(id);
                }
            }
            if (tag != null) return tag;
            return out.size() == 1 ? out.get(0) : out;
        }
        if (v instanceof JsonObject o) {
            if (o.has(ITEM)) return o.get(ITEM);
            if (o.has(TAG)) return new com.google.gson.JsonPrimitive(TAG_MARK + o.get(TAG).getAsString());
        }
        return v;
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
            YogLog.warn("recipe of " + namespace + " left out: " + m.getMessage() + " isn't in this version of Minecraft or on this loader");
            return null;
        }
    }

    private static String toVanilla(JsonObject in, String namespace, boolean mc121) {
        String type = in.has(RecipeField.TYPE) ? in.get(RecipeField.TYPE).getAsString() : "";
        if (type.contains(YogNames.Sep.NAMESPACE)) {
            // The mod's own kind of recipe (`mymod:infusion`): its data, for
            // its machines and books — not the game's.
            if (type.startsWith(namespace + YogNames.Sep.NAMESPACE)) return null;
            // Another mod's (a Java mod's `create:mixing`): as written.
            String json = in.toString();
            return mc121 ? upgradeRecipeJson(json) : json;
        }
        String result = in.has(RecipeField.RESULT) ? in.get(RecipeField.RESULT).getAsString() : GameRecipe.AIR;
        int count = in.has(RecipeField.COUNT) ? in.get(RecipeField.COUNT).getAsInt() : 1;
        JsonObject out = game(in, type, result, count, mc121);
        if (out == null) return in.toString();
        upgradeIngredients(out);
        if (in.has(RecipeField.GROUP)) out.addProperty(RecipeField.GROUP, in.get(RecipeField.GROUP).getAsString());
        if (in.has(RecipeField.CATEGORY)) out.addProperty(RecipeField.CATEGORY, in.get(RecipeField.CATEGORY).getAsString());
        return out.toString();
    }

    /** The game's recipe of Yog's kind {@code type}; none for a kind it hasn't got. */
    private static JsonObject game(JsonObject in, String type, String result, int count, boolean mc121) {
        JsonObject out = new JsonObject();
        switch (type) {
            case RecipeKind.SHAPED -> {
                out.addProperty(RecipeField.TYPE, GameRecipe.CRAFTING_SHAPED);
                out.add(RecipeField.PATTERN, in.get(RecipeField.PATTERN));
                JsonObject key = new JsonObject();
                for (Map.Entry<String, JsonElement> e : in.getAsJsonObject(RecipeField.KEY).entrySet()) {
                    key.add(e.getKey(), ingredient(e.getValue()));
                }
                out.add(RecipeField.KEY, key);
                out.add(RecipeField.RESULT, stack(result, count, mc121));
            }
            case RecipeKind.SHAPELESS -> {
                out.addProperty(RecipeField.TYPE, GameRecipe.CRAFTING_SHAPELESS);
                com.google.gson.JsonArray list = new com.google.gson.JsonArray();
                for (JsonElement e : in.getAsJsonArray(RecipeField.INGREDIENTS)) list.add(ingredient(e));
                out.add(RecipeField.INGREDIENTS, list);
                out.add(RecipeField.RESULT, stack(result, count, mc121));
            }
            case RecipeKind.SMELTING, RecipeKind.BLASTING, RecipeKind.SMOKING, RecipeKind.CAMPFIRE_COOKING -> {
                out.addProperty(RecipeField.TYPE, dev.yog.data.YogIds.join(YogNames.Ns.MINECRAFT, type));
                out.add(RecipeField.INGREDIENT, ingredient(in.get(RecipeField.INGREDIENT)));
                if (mc121) out.add(RecipeField.RESULT, stack(result, count, true));
                else out.addProperty(RecipeField.RESULT, result);
                out.addProperty(RecipeField.EXPERIENCE, in.has(RecipeField.EXPERIENCE) ? in.get(RecipeField.EXPERIENCE).getAsFloat() : COOKING_EXPERIENCE);
                out.addProperty(GameRecipe.COOKING_TIME, in.has(RecipeField.COOK_TIME) ? in.get(RecipeField.COOK_TIME).getAsInt() : COOKING_TICKS);
            }
            case RecipeKind.STONECUTTING -> {
                out.addProperty(RecipeField.TYPE, GameRecipe.STONECUTTING);
                out.add(RecipeField.INGREDIENT, ingredient(in.get(RecipeField.INGREDIENT)));
                if (mc121) {
                    out.add(RecipeField.RESULT, stack(result, count, true));
                } else {
                    out.addProperty(RecipeField.RESULT, result);
                    out.addProperty(RecipeField.COUNT, count);
                }
            }
            case RecipeKind.SMITHING -> {
                out.addProperty(RecipeField.TYPE, GameRecipe.SMITHING_TRANSFORM);
                for (String part : List.of(RecipeField.TEMPLATE, RecipeField.BASE, RecipeField.ADDITION)) out.add(part, ingredient(in.get(part)));
                out.add(RecipeField.RESULT, stack(result, count, mc121));
            }
            default -> {
                return null;
            }
        }
        return out;
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
        if (item.startsWith(TAG_MARK)) {
            String tag = item.substring(TAG_MARK.length());
            if (!dev.yog.data.YogTags.has(YogNames.Kind.ITEM, tag)) throw new Missing(TAG_MARK + tag);
            o.addProperty(GameRecipe.TAG, tag);
        } else {
            o.addProperty(GameRecipe.ITEM, item(item));
        }
        return o;
    }

    /** A result stack: {@code {"item": …}} before 1.21, {@code {"id": …}} since. */
    private static JsonObject stack(String id, int count, boolean mc121) {
        JsonObject o = new JsonObject();
        o.addProperty(mc121 ? GameRecipe.ID : GameRecipe.ITEM, item(id));
        if (count != 1) o.addProperty(RecipeField.COUNT, count);
        return o;
    }
}
