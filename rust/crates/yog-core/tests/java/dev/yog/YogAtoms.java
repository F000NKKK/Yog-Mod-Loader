package dev.yog;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/**
 * The frame test's stand-in for the host's {@code YogAtoms}: no runtime in
 * this JVM, so the table is the Rust test's, one name a line in the file
 * {@code -Dyog.atoms} names (line n: atom n).
 */
public final class YogAtoms {
    private YogAtoms() {}

    private static final List<String> NAMES = load();

    private static List<String> load() {
        try {
            return Files.readAllLines(Path.of(System.getProperty("yog.atoms")));
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
    }

    public static int number(String name) {
        return NAMES.indexOf(name);
    }

    public static String name(int n) {
        return n >= 0 && n < NAMES.size() ? NAMES.get(n) : null;
    }
}
