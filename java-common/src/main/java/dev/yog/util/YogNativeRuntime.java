package dev.yog.util;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;

/**
 * Locating and loading the embedded native Yog runtime. Pure Java — shared
 * by every loader's {@code NativeBridge}; only the game directory lookup is
 * loader specific.
 */
public final class YogNativeRuntime {
    private YogNativeRuntime() {
    }

    /** Directory players drop {@code .yog} mods into, relative to the game directory. */
    public static final String MODS_DIR = "yog-mods";

    /** Jar resource root holding one runtime per {@code <os>-<arch>} platform tag. */
    private static final String NATIVES_ROOT = "/natives/";
    private static final String LIB_BASE_NAME = "yog_runtime";

    /**
     * Extract the platform's runtime native from inside this jar and load it, so
     * players never deal with a loose .so/.dll. The jar bundles every supported
     * platform under {@code /natives/<os>-<arch>/}.
     */
    public static void loadEmbedded() {
        String resource = NATIVES_ROOT + platformTag() + "/" + libName();
        try (InputStream in = YogNativeRuntime.class.getResourceAsStream(resource)) {
            if (in == null) {
                throw new IllegalStateException("embedded Yog runtime not found: " + resource);
            }
            Path tmp = Files.createTempFile(LIB_BASE_NAME, "-" + libName());
            try {
                Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
                System.load(tmp.toAbsolutePath().toString());
            } catch (IOException | RuntimeException | Error e) {
                try {
                    Files.deleteIfExists(tmp);
                } catch (IOException cleanup) {
                    e.addSuppressed(cleanup);
                }
                throw e;
            }
            tmp.toFile().deleteOnExit();
        } catch (IOException e) {
            throw new RuntimeException("failed to load the Yog native runtime", e);
        }
    }

    /** {@code <gameDir>/yog-mods}, created if missing; absolute path string for {@code nativeInit}. */
    public static String modsDir(Path gameDir) {
        Path dir = gameDir.resolve(MODS_DIR);
        try {
            Files.createDirectories(dir);
        } catch (IOException ignored) {
            // best effort; the runtime tolerates a missing directory
        }
        return dir.toAbsolutePath().toString();
    }

    /** e.g. {@code linux-x86_64} — must match the Rust runtime's platform tag. */
    private static String platformTag() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        String osTag = os.contains("win") ? "windows" : os.contains("mac") ? "macos" : "linux";
        String archTag = switch (arch) {
            case "amd64", "x86_64" -> "x86_64";
            case "aarch64", "arm64" -> "aarch64";
            default -> arch;
        };
        return osTag + "-" + archTag;
    }

    private static String libName() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("win")) {
            return LIB_BASE_NAME + ".dll";
        }
        return os.contains("mac") ? "lib" + LIB_BASE_NAME + ".dylib" : "lib" + LIB_BASE_NAME + ".so";
    }
}
