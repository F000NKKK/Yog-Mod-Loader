package dev.yog.util;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;
import java.util.stream.Stream;

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
        cleanupStaleNativeExtractions(Path.of(System.getProperty("java.io.tmpdir")));
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

    private static void cleanupStaleNativeExtractions(Path tempDir) {
        if (!Files.isDirectory(tempDir)) {
            return;
        }
        try (Stream<Path> files = Files.list(tempDir)) {
            for (Path file : files.toList()) {
                Long pid = extractionOwnerPid(file.getFileName().toString());
                if (pid != null && !ProcessHandle.of(pid).map(ProcessHandle::isAlive).orElse(false)) {
                    Files.deleteIfExists(file);
                }
            }
        } catch (IOException e) {
            throw new RuntimeException("failed to clean stale Yog native extractions", e);
        }
    }

    private static Long extractionOwnerPid(String fileName) {
        if (!fileName.startsWith("yog-")) {
            return null;
        }
        String baseName;
        if (fileName.endsWith(".so")) {
            baseName = fileName.substring(0, fileName.length() - 3);
        } else if (fileName.endsWith(".dll")) {
            baseName = fileName.substring(0, fileName.length() - 4);
        } else if (fileName.endsWith(".dylib")) {
            baseName = fileName.substring(0, fileName.length() - 6);
        } else {
            return null;
        }

        int counterSeparator = baseName.lastIndexOf('-');
        int pidSeparator = counterSeparator > 0 ? baseName.lastIndexOf('-', counterSeparator - 1) : -1;
        if (pidSeparator <= "yog-".length()
                || !isDigits(baseName, pidSeparator + 1, counterSeparator)
                || !isDigits(baseName, counterSeparator + 1, baseName.length())) {
            return null;
        }
        try {
            long pid = Long.parseLong(baseName.substring(pidSeparator + 1, counterSeparator));
            return pid > 0 ? pid : null;
        } catch (NumberFormatException e) {
            return null;
        }
    }

    private static boolean isDigits(String value, int start, int end) {
        if (start >= end) {
            return false;
        }
        for (int i = start; i < end; i++) {
            char digit = value.charAt(i);
            if (digit < '0' || digit > '9') {
                return false;
            }
        }
        return true;
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
