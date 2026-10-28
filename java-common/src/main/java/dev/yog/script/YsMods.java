package dev.yog.script;

import dev.yog.YogLog;
import dev.yog.YogNames;
import java.io.IOException;
import java.io.InputStream;
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
    private static final Map<String, List<ModLoader>> LOADERS = new ConcurrentHashMap<>();
    /** The scripts' folder in a {@code .yog} for this game version. */
    private static volatile String folder;

    /** Every {@code .yog} in {@code modsDir}: its scripts for game version {@code mcVersion} loaded. */
    public static void load(Path modsDir, String mcVersion) {
        // Each side's ticks drive its tasks (what an `await` goes on on).
        dev.yog.script.threading.tasks.Scheduler.listen();
        // What a reload carries waits for its owners only while the world runs.
        dev.yog.bus.YogEvents.Server.Life.Stopping.on(0, e -> {
            dev.yog.script.yog.Session.forgetCarried();
            return null;
        });
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
     * {@code .yog}; the runtime's hot reload, {@code yog:script/reload}),
     * on the server's thread between its ticks: the old generation's saved
     * members written and its handlers unsubscribed, the new classes
     * loaded, everything live carried into them (Session: objects,
     * statics, async methods mid-way), its queued work theirs. Whether
     * they loaded.
     */
    public static boolean reload(String modId, String path) {
        boolean[] loaded = {false};
        dev.yog.script.threading.tasks.Scheduler.onServerThread(() -> loaded[0] = reloadNow(modId, Path.of(path)));
        YogLog.info("Yog Script: " + modId + (loaded[0] ? " reloaded" : " didn't reload"));
        return loaded[0];
    }

    private static boolean reloadNow(String modId, Path path) {
        List<ModLoader> old = LOADERS.remove(modId);
        if (old == null) {
            return folder != null && loadMod(modId, path);
        }
        java.util.Set<ClassLoader> was = new java.util.HashSet<>(old);
        List<Class<?>> classes = new java.util.ArrayList<>();
        for (ModLoader loader : old) {
            dev.yog.script.yog.State.retire(loader);
            dev.yog.script.yog.SyncedState.retire(loader);
            dev.yog.Generations.retire(loader);
            classes.addAll(loader.loaded());
        }
        dev.yog.script.yog.BlockObject.retire(was);
        dev.yog.script.yog.EntityObject.retire(was);
        dev.yog.script.yog.ItemObject.retire(was);
        Map<String, Object> live = dev.yog.script.yog.Session.retire(was);
        boolean loaded = folder != null && loadMod(modId, path);
        List<ModLoader> next = LOADERS.getOrDefault(modId, List.of());
        if (loaded && !next.isEmpty()) {
            dev.yog.script.yog.Session.carry(was, live, classes, new java.util.ArrayList<>(next));
            old.forEach(l -> dev.yog.Generations.succeed(l, next.get(0)));
        } else {
            old.forEach(dev.yog.Generations::gone);
            dev.yog.script.yog.Session.forgetCarried();
        }
        old.forEach(YsMods::close);
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
    private static ModLoader start(Path jar, Path mod) throws IOException {
        ModLoader loader = new ModLoader(jar, YsMods.class.getClassLoader());
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
