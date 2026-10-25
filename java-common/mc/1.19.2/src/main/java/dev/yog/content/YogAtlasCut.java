package dev.yog.content;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.NativeImage;
import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.util.YogImagePack;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.stream.Stream;

/**
 * The mods' packed pictures cut into their sprites' own files. The build
 * packs an atlas's textures into one picture and says how to cut it (an
 * {@code unstitch} source of {@code assets/<ns>/atlases/<atlas>.json}); this
 * version has no atlas definitions — it stitches every texture a model or a
 * particle names from its own file — so the staged pack gets those files.
 * Client resources only: the server has no pictures.
 */
final class YogAtlasCut {
    private YogAtlasCut() {}

    /**
     * Where an atlas's sprites are under {@code textures/}: the folder this
     * version reads each from (later versions say it in their atlases'
     * definitions). The blocks atlas's sprites name their folder themselves.
     */
    private static final Map<String, String> SPRITE_DIRS = Map.of(
            YogNames.AtlasSource.BLOCKS, "",
            "particles", "particle/",
            "mob_effects", "mob_effect/");

    /** Every packed picture of the pack at {@code root} cut into its sprites' files. */
    static void cut(Path root) {
        Path atlases = root.resolve(YogNames.Pack.ASSETS).resolve(YogNames.Ns.MINECRAFT).resolve(YogNames.Res.ATLASES);
        if (!Files.isDirectory(atlases)) return;
        try (Stream<Path> files = Files.list(atlases)) {
            for (Path atlas : files.toList()) {
                String name = atlas.getFileName().toString().replace(YogNames.Res.JSON, "");
                String dir = SPRITE_DIRS.get(name);
                if (dir != null) cutAtlas(root, atlas, dir);
            }
        } catch (IOException e) {
            YogLog.warn("atlases: " + e);
        }
    }

    private static void cutAtlas(Path root, Path atlas, String dir) throws IOException {
        JsonObject def = new Gson().fromJson(Files.readString(atlas), JsonObject.class);
        if (def == null || !def.has(YogNames.AtlasSource.SOURCES)) return;
        for (var element : def.getAsJsonArray(YogNames.AtlasSource.SOURCES)) {
            JsonObject source = element.getAsJsonObject();
            if (source.has(YogNames.AtlasSource.TYPE)
                    && source.get(YogNames.AtlasSource.TYPE).getAsString().endsWith(YogNames.AtlasSource.UNSTITCH)) {
                unstitch(root, source, dir);
            }
        }
    }

    /** One packed picture's regions, each written as its sprite's file. */
    private static void unstitch(Path root, JsonObject source, String dir) throws IOException {
        Path picture = texture(root, source.get(YogNames.AtlasSource.RESOURCE).getAsString(), "");
        if (!Files.exists(picture)) return;
        try (InputStream in = YogImagePack.png(Files.newInputStream(picture)); NativeImage whole = NativeImage.read(in)) {
            double sx = whole.getWidth() / source.get(YogNames.AtlasSource.DIVISOR_X).getAsDouble();
            double sy = whole.getHeight() / source.get(YogNames.AtlasSource.DIVISOR_Y).getAsDouble();
            for (var element : source.getAsJsonArray(YogNames.AtlasSource.REGIONS)) {
                JsonObject r = element.getAsJsonObject();
                Path out = texture(root, r.get(YogNames.AtlasSource.SPRITE).getAsString(), dir);
                if (Files.exists(out)) continue;
                int x = (int) Math.round(r.get(YogNames.AtlasSource.X).getAsDouble() * sx);
                int y = (int) Math.round(r.get(YogNames.AtlasSource.Y).getAsDouble() * sy);
                int w = (int) Math.round(r.get(YogNames.AtlasSource.WIDTH).getAsDouble() * sx);
                int h = (int) Math.round(r.get(YogNames.AtlasSource.HEIGHT).getAsDouble() * sy);
                Files.createDirectories(out.getParent());
                try (NativeImage sprite = new NativeImage(w, h, false)) {
                    for (int py = 0; py < h; py++) {
                        for (int px = 0; px < w; px++) sprite.setPixelRGBA(px, py, whole.getPixelRGBA(x + px, y + py));
                    }
                    sprite.writeToFile(out);
                }
            }
        }
    }

    /** {@code assets/<ns>/textures/<dir><path>.png} of texture id {@code id}. */
    private static Path texture(Path root, String id, String dir) {
        int colon = id.indexOf(YogNames.Sep.NAMESPACE);
        String ns = id.substring(0, colon), path = id.substring(colon + 1);
        return root.resolve(YogNames.Pack.ASSETS).resolve(ns)
                .resolve(YogNames.Res.TEXTURES + dir + path + YogNames.Res.PNG);
    }
}
