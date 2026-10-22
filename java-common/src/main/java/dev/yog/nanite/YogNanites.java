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
import org.objectweb.asm.Type;

/**
 * Mods' injections made Mixin classes before the game's classes load
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

    private static List<String> make() {
        try {
            Path game = YogEarly.gameDir();
            YogNativeRuntime.loadEmbedded();
            byte[] bytes = nativeDeclarations(YogNativeRuntime.modsDir(game));
            List<YogTypes.Inject> points = points(YogTypes.Nanites.fromBytes(bytes).injects());
            if (points.isEmpty()) return List.of();
            Path dir = clean(game.resolve(DIR));
            List<String> names = new ArrayList<>();
            List<String> keys = new ArrayList<>();
            List<Type> returns = new ArrayList<>();
            for (int i = 0; i < points.size(); i++) {
                YogTypes.Inject inject = points.get(i);
                Path file = dir.resolve(YogNaniteGen.className(i) + CLASS_EXT);
                Files.createDirectories(file.getParent());
                Files.write(file, YogNaniteGen.mixin(i, inject));
                names.add(YogNaniteGen.CLASS_PREFIX + i);
                keys.add(inject.point());
                returns.add(Type.getReturnType(YogMappings.desc(inject.desc())));
            }
            YogInjections.points(keys, returns);
            YogEarly.addClasses(dir);
            YogLog.info("nanites: " + names.size() + " injection point(s) made");
            return names;
        } catch (IOException | RuntimeException | LinkageError e) {
            YogLog.error("nanites: none made", e);
            return List.of();
        }
    }

    /** One injection a point (the first declaration of it; cancellable if any is). */
    private static List<YogTypes.Inject> points(List<YogTypes.Inject> all) {
        Map<String, YogTypes.Inject> byPoint = new LinkedHashMap<>();
        for (YogTypes.Inject i : all) {
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
