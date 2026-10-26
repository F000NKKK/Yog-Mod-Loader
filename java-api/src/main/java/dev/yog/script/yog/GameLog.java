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

    public static boolean IsEnabled(LogLevel level, String category) {
        Level at = levelOf(level);
        return at != null && of(category).isEnabledForLevel(at);
    }

    public static void Write(LogLevel level, String category, String message, dev.yog.script.Error error) {
        Level at = levelOf(level);
        if (at == null) {
            return;
        }
        String text = error == null ? message : message + System.lineSeparator() + error.Message;
        of(category).atLevel(at).log(text);
    }
}
