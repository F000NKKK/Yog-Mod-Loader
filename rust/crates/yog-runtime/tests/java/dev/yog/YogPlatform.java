package dev.yog;

import java.nio.file.Path;

/** A stand-in for a loader's platform (the tests' JVM is none): a client, here. */
public final class YogPlatform {
    private YogPlatform() {}

    public static boolean isClient() {
        return true;
    }

    public static Path gameDir() {
        return Path.of("").toAbsolutePath();
    }
}
