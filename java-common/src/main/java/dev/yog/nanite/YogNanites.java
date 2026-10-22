package dev.yog.nanite;

import dev.yog.YogLog;
import dev.yog.bus.YogTypes;
import dev.yog.util.YogNativeRuntime;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

/**
 * Mods' nanites made Mixin classes before the game's classes load
 * ({@code docs/design/nanites.md}): the runtime asks every mod for its
 * injections (their libraries opened on their own), one mixin is made a
 * point (mods at one point share it), written under the game's
 * {@code .yog/nanites/} and put where Mixin finds them; the host's nanite
 * config ({@link YogNanitePlugin}) lists them.
 */
public final class YogNanites {
    private YogNanites() {}

    /** Where the generated classes go, under the game's directory. */
    private static final String DIR = ".yog/nanites";
    private static final String CLASS_EXT = ".class";

    private static List<String> mixins;

    /** The generated mixins' simple names (made the first time asked). */
    static synchronized List<String> mixins() {
        if (mixins == null) mixins = make();
        return mixins;
    }

    /** This game's nanites: their points and what each was made into. */
    record Made(List<YogTypes.Inject> points, List<YogNaniteGen.Made> made) {}

    /**
     * Every mod's nanites made (one a point) — the same each time a process
     * asks: the plugin to write the mixins, {@link YogInjections} (perhaps
     * another class loader's copy of it) to know what they are.
     */
    static Made made() {
        Path game = YogEarly.gameDir();
        YogNativeRuntime.loadEmbedded();
        byte[] bytes = nativeDeclarations(YogNativeRuntime.modsDir(game));
        List<YogTypes.Inject> points = points(YogTypes.Nanites.fromBytes(bytes).injects());
        List<YogNaniteGen.Made> made = new ArrayList<>();
        for (int i = 0; i < points.size(); i++) made.add(YogNaniteGen.mixin(i, points.get(i)));
        return new Made(points, made);
    }

    private static List<String> make() {
        try {
            Made all = made();
            if (all.points().isEmpty()) return List.of();
            Path dir = clean(YogEarly.gameDir().resolve(DIR));
            List<String> names = new ArrayList<>();
            for (int i = 0; i < all.points().size(); i++) {
                Path file = dir.resolve(YogNaniteGen.className(i) + CLASS_EXT);
                Files.createDirectories(file.getParent());
                Files.write(file, all.made().get(i).bytes());
                names.add(YogNaniteGen.CLASS_PREFIX + i);
            }
            YogEarly.addClasses(dir);
            YogLog.info("nanites: " + names.size() + " injection point(s) made");
            return names;
        } catch (IOException | RuntimeException | LinkageError e) {
            YogLog.error("nanites: none made", e);
            return List.of();
        }
    }

    /** Where the game's client-only classes are (Mojang's names): none of them on a dedicated server. */
    private static final String[] CLIENT_PACKAGES = {"net.minecraft.client.", "com.mojang.blaze3d."};

    /** Whether nanite {@code i} goes into a class this game hasn't (the client's, on a dedicated server). */
    private static boolean absent(YogTypes.Inject i, boolean client) {
        if (client) return false;
        for (String p : CLIENT_PACKAGES) {
            if (i.class_().startsWith(p)) return true;
        }
        return false;
    }

    /** One injection a point (the first declaration of it; cancellable if any is), those this game has. */
    private static List<YogTypes.Inject> points(List<YogTypes.Inject> all) {
        boolean client = YogEarly.isClient();
        Map<String, YogTypes.Inject> byPoint = new LinkedHashMap<>();
        for (YogTypes.Inject i : all) {
            if (absent(i, client)) continue;
            YogTypes.Inject known = byPoint.get(i.point());
            if (known == null || (!known.cancellable() && i.cancellable())) byPoint.put(i.point(), i);
        }
        return new ArrayList<>(byPoint.values());
    }

    /** {@code dir}, emptied of a previous launch's classes. */
    private static Path clean(Path dir) throws IOException {
        if (Files.isDirectory(dir)) {
            try (Stream<Path> old = Files.walk(dir)) {
                for (Path p : old.sorted(Comparator.reverseOrder()).toList()) Files.delete(p);
            }
        }
        Files.createDirectories(dir);
        return dir;
    }

    private static native byte[] nativeDeclarations(String modsDir);
}
