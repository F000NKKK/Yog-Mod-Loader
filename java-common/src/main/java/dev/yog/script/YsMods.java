package dev.yog.script;

import dev.yog.YogLog;
import dev.yog.YogNames;
import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Map;
import java.util.ServiceLoader;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * The game's Yog Script mods: each {@code .yog}'s scripts compiled for this
 * game version ({@code scripts/<version>/*.jar}, its Java's) loaded in a
 * class loader of their own, its {@link YsMod}s made. A mod that fails to load is reported; the others
 * load. A mod rebuilt while the game runs is loaded again ({@link #reload}):
 * its old classes' state written out and their handlers retired, its new
 * classes loaded in loaders of their own (docs/design/storage.md, S8).
 */
public final class YsMods {
    private YsMods() {}

    /** What a mod's file and a jar are named with. */
    private static final String MOD_EXT = ".yog";
    private static final String JAR_EXT = ".jar";
    /** What separates an archive's folders. */
    private static final String ZIP_SEP = "/";

    /** Each mod's class loaders now (its file's name without {@code .yog}). */
    private static final Map<String, List<URLClassLoader>> LOADERS = new ConcurrentHashMap<>();
    /** The scripts' folder in a {@code .yog} for this game version. */
    private static volatile String folder;

    /** Every {@code .yog} in {@code modsDir}: its scripts for game version {@code mcVersion} loaded. */
    public static void load(Path modsDir, String mcVersion) {
        // Each side's ticks drive its tasks (what an `await` goes on on).
        dev.yog.script.threading.tasks.Scheduler.listen();
        folder = YogNames.Archive.SCRIPTS + ZIP_SEP + mcVersion + ZIP_SEP;
        try (DirectoryStream<Path> mods = Files.newDirectoryStream(modsDir, "*" + MOD_EXT)) {
            for (Path mod : mods) {
                loadMod(modId(mod), mod);
            }
        } catch (IOException e) {
            YogLog.warn("Yog Script: " + modsDir + " can't be read: " + e.getMessage());
        }
    }

    /**
     * Mod {@code modId}'s scripts again from {@code path} (its rebuilt
     * {@code .yog}; the runtime's hot reload, {@code yog:script/reload}):
     * its live objects' saved state written and dropped, its old classes'
     * handlers and tasks retired, the new classes loaded. Whether they loaded.
     */
    public static boolean reload(String modId, String path) {
        List<URLClassLoader> old = LOADERS.remove(modId);
        if (old != null) {
            for (URLClassLoader loader : old) {
                dev.yog.script.yog.State.retire(loader);
                dev.yog.script.yog.SyncedState.retire(loader);
                dev.yog.Generations.retire(loader);
            }
        }
        boolean loaded = folder != null && loadMod(modId, Path.of(path));
        if (old != null) {
            old.forEach(YsMods::close);
        }
        YogLog.info("Yog Script: " + modId + (loaded ? " reloaded" : " didn't reload"));
        return loaded;
    }

    private static void close(URLClassLoader loader) {
        try {
            loader.close();
        } catch (IOException e) {
            YogLog.warn("Yog Script: an old class loader didn't close: " + e.getMessage());
        }
    }

    /** A mod's id as its file names it: {@code hexcasting.yog} → {@code hexcasting}. */
    private static String modId(Path mod) {
        String name = mod.getFileName().toString();
        return name.endsWith(MOD_EXT) ? name.substring(0, name.length() - MOD_EXT.length()) : name;
    }

    /** {@code mod}'s scripts for this game version loaded; whether it loaded. */
    private static boolean loadMod(String modId, Path mod) {
        try (ZipFile zip = new ZipFile(mod.toFile())) {
            var entries = zip.entries();
            while (entries.hasMoreElements()) {
                ZipEntry e = entries.nextElement();
                String name = e.getName();
                if (name.startsWith(folder) && name.endsWith(JAR_EXT)) {
                    LOADERS.computeIfAbsent(modId, k -> new CopyOnWriteArrayList<>()).add(start(extracted(zip, e), mod));
                }
            }
            return true;
        } catch (IOException | RuntimeException e) {
            YogLog.error("Yog Script: " + mod.getFileName() + " didn't load: " + e);
            return false;
        }
    }

    /** The jar out of the {@code .yog}, as a file a class loader reads. */
    private static Path extracted(ZipFile zip, ZipEntry e) throws IOException {
        Path jar = Files.createTempFile("yog-script-", JAR_EXT);
        jar.toFile().deleteOnExit();
        try (InputStream in = zip.getInputStream(e)) {
            Files.copy(in, jar, StandardCopyOption.REPLACE_EXISTING);
        }
        return jar;
    }

    /** The jar's mods made, as its loader's code ({@link dev.yog.Generations}):
     * each one's constructor is its loading. */
    private static URLClassLoader start(Path jar, Path mod) throws IOException {
        URLClassLoader loader = new URLClassLoader(new URL[] {jar.toUri().toURL()}, YsMods.class.getClassLoader());
        for (ServiceLoader.Provider<YsMod> m : ServiceLoader.load(YsMod.class, loader).stream().toList()) {
            try {
                dev.yog.Generations.within(loader, m::get);
                YogLog.info("Yog Script: loaded " + m.type().getName() + " from " + mod.getFileName());
            } catch (YsPanic | java.util.ServiceConfigurationError e) {
                YogLog.error("Yog Script: " + m.type().getName() + " panicked loading: " + e.getMessage());
            }
        }
        return loader;
    }
}
