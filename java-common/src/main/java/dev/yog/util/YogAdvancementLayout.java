package dev.yog.util;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import dev.yog.YogNames;
import dev.yog.YogNames.AdvancementField;

/**
 * The mods' advancements as this version reads them. A tab's background is
 * a texture id ({@code minecraft:block/calcite}) since 1.21.5; before, the
 * texture's file ({@code minecraft:textures/block/calcite.png}).
 */
public final class YogAdvancementLayout {
    private YogAdvancementLayout() {}

    /** The first version whose backgrounds are texture ids. */
    private static final int[] TEXTURE_ID_BACKGROUNDS_SINCE = {1, 21, 5};
    private static final String TEXTURES = YogNames.Res.TEXTURES, PNG = YogNames.Res.PNG, NS = YogNames.Sep.NAMESPACE;
    /** An advancement's folder, singular or plural (as {@link YogDataLayout#remapDataPath} left it). */
    private static final String DIR = YogNames.RenamedDataDir.ADVANCEMENT;

    /** Whether this version's backgrounds are files; set by {@link #setVersion}. */
    private static boolean fileBackgrounds;

    /** The game version advancements are written for. */
    public static void setVersion(String mcVersion) {
        fileBackgrounds = !YogDataLayout.atLeast(mcVersion, TEXTURE_ID_BACKGROUNDS_SINCE);
    }

    /** Whether {@code path} ({@code data/<ns>/advancement(s)/….json}) needs rewriting for this version. */
    public static boolean rewrites(String path) {
        if (!fileBackgrounds) return false;
        String[] p = path.split(YogNames.Sep.PATH, 4);
        return p.length == 4 && p[0].equals(YogNames.Pack.DATA) && p[2].startsWith(DIR) && path.endsWith(YogNames.Res.JSON);
    }

    /** {@code json} with its background a file again; anything else as it is. */
    public static String withFileBackground(String json) {
        try {
            JsonElement root = JsonParser.parseString(json);
            if (!root.isJsonObject()) return json;
            JsonObject display = root.getAsJsonObject().getAsJsonObject(AdvancementField.DISPLAY);
            JsonElement bg = display == null ? null : display.get(AdvancementField.BACKGROUND);
            if (bg == null || !bg.isJsonPrimitive()) return json;
            String id = bg.getAsString();
            int at = id.indexOf(NS);
            String ns = at < 0 ? YogNames.Ns.MINECRAFT : id.substring(0, at), path = id.substring(at + 1);
            if (path.startsWith(TEXTURES)) return json;
            display.addProperty(AdvancementField.BACKGROUND, ns + NS + TEXTURES + path + PNG);
            return root.toString();
        } catch (RuntimeException e) {
            return json;
        }
    }
}
