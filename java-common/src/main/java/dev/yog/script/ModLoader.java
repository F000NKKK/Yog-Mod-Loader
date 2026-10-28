package dev.yog.script;

import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.zip.ZipFile;

/**
 * One generation of a Yog Script mod's classes: its jar's, in a loader of
 * their own (a hot reload makes the next one), knowing which it loaded.
 */
final class ModLoader extends URLClassLoader {
    /** What a class file ends with, and the separators of its name. */
    private static final String CLASS_EXT = ".class";
    private static final char PATH_SEP = '/';
    private static final char NAME_SEP = '.';

    private final Path jar;

    ModLoader(Path jar, ClassLoader parent) throws IOException {
        super(new URL[] {jar.toUri().toURL()}, parent);
        this.jar = jar;
    }

    /** The classes of its jar it has loaded so far (not loading any). */
    List<Class<?>> loaded() {
        List<Class<?>> out = new ArrayList<>();
        try (ZipFile zip = new ZipFile(jar.toFile())) {
            zip.stream()
                    .map(e -> e.getName())
                    .filter(n -> n.endsWith(CLASS_EXT))
                    .map(n -> n.substring(0, n.length() - CLASS_EXT.length()).replace(PATH_SEP, NAME_SEP))
                    .map(this::findLoadedClass)
                    .filter(c -> c != null)
                    .forEach(out::add);
        } catch (IOException e) {
            dev.yog.YogLog.warn("Yog Script: " + jar + " can't be listed: " + e.getMessage());
        }
        return out;
    }
}
