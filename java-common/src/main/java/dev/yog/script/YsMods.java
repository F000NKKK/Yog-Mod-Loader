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
import java.util.ServiceLoader;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * The game's Yog Script mods: each {@code .yog}'s scripts compiled for this
 * game version ({@code scripts/<version>/*.jar}, its Java's) loaded in a
 * class loader of their own, its {@link YsMod}s made. A mod that fails to load is reported; the others
 * load.
 */
public final class YsMods {
    private YsMods() {}

    /** What a mod's file and a jar are named with. */
    private static final String MOD_EXT = ".yog";
    private static final String JAR_EXT = ".jar";
    /** What separates an archive's folders. */
    private static final String ZIP_SEP = "/";

    /** Every {@code .yog} in {@code modsDir}: its scripts for game version {@code mcVersion} loaded. */
    public static void load(Path modsDir, String mcVersion) {
        // Each side's ticks drive its tasks (what an `await` goes on on).
        dev.yog.script.threading.tasks.Scheduler.listen();
        String folder = YogNames.Archive.SCRIPTS + ZIP_SEP + mcVersion + ZIP_SEP;
        try (DirectoryStream<Path> mods = Files.newDirectoryStream(modsDir, "*" + MOD_EXT)) {
            for (Path mod : mods) {
                loadMod(mod, folder);
            }
        } catch (IOException e) {
            YogLog.warn("Yog Script: " + modsDir + " can't be read: " + e.getMessage());
        }
    }

    private static void loadMod(Path mod, String folder) {
        try (ZipFile zip = new ZipFile(mod.toFile())) {
            var entries = zip.entries();
            while (entries.hasMoreElements()) {
                ZipEntry e = entries.nextElement();
                String name = e.getName();
                if (name.startsWith(folder) && name.endsWith(JAR_EXT)) {
                    start(extracted(zip, e), mod);
                }
            }
        } catch (IOException | RuntimeException e) {
            YogLog.error("Yog Script: " + mod.getFileName() + " didn't load: " + e);
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

    /** The jar's mods made: each one's constructor is its loading. */
    private static void start(Path jar, Path mod) throws IOException {
        URLClassLoader loader = new URLClassLoader(new URL[] {jar.toUri().toURL()}, YsMods.class.getClassLoader());
        for (ServiceLoader.Provider<YsMod> m : ServiceLoader.load(YsMod.class, loader).stream().toList()) {
            try {
                m.get();
                YogLog.info("Yog Script: loaded " + m.type().getName() + " from " + mod.getFileName());
            } catch (YsPanic | java.util.ServiceConfigurationError e) {
                YogLog.error("Yog Script: " + m.type().getName() + " panicked loading: " + e.getMessage());
            }
        }
    }
}
