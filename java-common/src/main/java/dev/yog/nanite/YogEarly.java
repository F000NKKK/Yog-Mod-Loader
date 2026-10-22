package dev.yog.nanite;

import dev.yog.YogLog;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;

/**
 * What nanites need of the loader before the game's classes load, reached
 * by reflection so this code is every loader's: the game's directory, and
 * a directory of classes put where Mixin looks for mixins — Fabric's class
 * path, or (ModLauncher, which asks the thread's context class loader for
 * what its own loader hasn't) a class loader in front of the context one.
 */
final class YogEarly {
    private YogEarly() {}

    private static final String FABRIC_LOADER = "net.fabricmc.loader.api.FabricLoader";
    private static final String FABRIC_LAUNCHER = "net.fabricmc.loader.impl.launch.FabricLauncherBase";
    private static final String FML_PATHS = "net.minecraftforge.fml.loading.FMLPaths";
    private static final String NEO_FML_PATHS = "net.neoforged.fml.loading.FMLPaths";
    private static final String GAME_DIR = "GAMEDIR";

    /** The game's directory. */
    static Path gameDir() {
        try {
            Object loader = Class.forName(FABRIC_LOADER).getMethod("getInstance").invoke(null);
            return (Path) loader.getClass().getMethod("getGameDir").invoke(loader);
        } catch (ReflectiveOperationException | LinkageError e) {
            for (String paths : new String[] {NEO_FML_PATHS, FML_PATHS}) {
                try {
                    Object dir = Class.forName(paths).getField(GAME_DIR).get(null);
                    return (Path) dir.getClass().getMethod("get").invoke(dir);
                } catch (ReflectiveOperationException | LinkageError ignored) {
                    // Not this loader: the next.
                }
            }
        }
        return Path.of("").toAbsolutePath();
    }

    /** Whether this game is a client (the loader's platform says). */
    static boolean isClient() {
        return dev.yog.YogPlatform.isClient();
    }

    /** Puts {@code dir}'s classes where Mixin finds them. */
    static void addClasses(Path dir) {
        try {
            Object launcher = Class.forName(FABRIC_LAUNCHER).getMethod("getLauncher").invoke(null);
            Method add = launcher.getClass().getMethod("addToClassPath", Path.class, String[].class);
            add.invoke(launcher, dir, new String[0]);
            return;
        } catch (ReflectiveOperationException | LinkageError e) {
            // Not Fabric: ModLauncher.
        }
        try {
            Thread t = Thread.currentThread();
            URL[] urls = {dir.toUri().toURL()};
            t.setContextClassLoader(new URLClassLoader(urls, t.getContextClassLoader()));
        } catch (java.net.MalformedURLException e) {
            YogLog.error("nanites: " + dir + " can't be a class path", e);
        }
    }
}
