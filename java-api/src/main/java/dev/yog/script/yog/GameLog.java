package dev.yog.script.yog;

import dev.yog.script.logging.LogLevel;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.event.Level;

/** Yog Script's {@code Yog.GameLog}: the game's log (SLF4J), one logger per category. */
public final class GameLog {
    private GameLog() {}

    private static final Map<String, Logger> LOGGERS = new ConcurrentHashMap<>();

    private static Logger of(String category) {
        return LOGGERS.computeIfAbsent(category, LoggerFactory::getLogger);
    }

    /** The game's level of {@code level} ({@code null}: {@code None}, nothing written). */
    private static Level levelOf(LogLevel level) {
        if (level == LogLevel.Trace) {
            return Level.TRACE;
        }
        if (level == LogLevel.Debug) {
            return Level.DEBUG;
        }
        if (level == LogLevel.Information) {
            return Level.INFO;
        }
        if (level == LogLevel.Warning) {
            return Level.WARN;
        }
        return level == LogLevel.None ? null : Level.ERROR;
    }

    /**
     * Whether {@code category}'s logger writes {@code level}: by SLF4J 1's
     * methods (the game's on 1.19, before 2's {@code isEnabledForLevel}).
     */
    public static boolean IsEnabled(LogLevel level, String category) {
        Level at = levelOf(level);
        if (at == null) {
            return false;
        }
        Logger log = of(category);
        return switch (at) {
            case TRACE -> log.isTraceEnabled();
            case DEBUG -> log.isDebugEnabled();
            case INFO -> log.isInfoEnabled();
            case WARN -> log.isWarnEnabled();
            case ERROR -> log.isErrorEnabled();
        };
    }

    public static void Write(LogLevel level, String category, String message, dev.yog.script.Error error) {
        Level at = levelOf(level);
        if (at == null) {
            return;
        }
        String text = error == null ? message : message + System.lineSeparator() + error.Message;
        Logger log = of(category);
        // SLF4J 1's methods: its 2's `atLevel` isn't the game's on 1.19.
        switch (at) {
            case TRACE -> log.trace(text);
            case DEBUG -> log.debug(text);
            case INFO -> log.info(text);
            case WARN -> log.warn(text);
            case ERROR -> log.error(text);
        }
    }
}
