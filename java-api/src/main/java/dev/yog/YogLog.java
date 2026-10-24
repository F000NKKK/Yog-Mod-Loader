package dev.yog;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Yog's log: the game's own (SLF4J), so the hosts' records and the native
 * side's — the runtime's and every mod's, sent here by
 * {@code yog_logging}'s sink — land in {@code latest.log} with the game's.
 */
public final class YogLog {
    private YogLog() {}

    private static final Logger LOG = LoggerFactory.getLogger("Yog");

    /** {@code yog_logging::Level} as the native side numbers it. */
    private static final int INFO = 0;
    private static final int WARN = 1;

    /** A native record (the runtime's {@code host_log}). */
    public static void log(int level, String message) {
        switch (level) {
            case INFO -> LOG.info(message);
            case WARN -> LOG.warn(message);
            default -> LOG.error(message);
        }
    }

    public static void info(String message) {
        LOG.info(message);
    }

    public static void warn(String message) {
        LOG.warn(message);
    }

    public static void error(String message) {
        LOG.error(message);
    }

    public static void error(String message, Throwable cause) {
        LOG.error(message, cause);
    }
}
