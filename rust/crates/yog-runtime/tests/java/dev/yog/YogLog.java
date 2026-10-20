package dev.yog;

/** A stand-in for the host's logger (which needs SLF4J): what the tests' Java writes goes to stderr. */
public final class YogLog {
    private YogLog() {}

    public static void error(String message, Throwable t) {
        System.err.println(message + ": " + t);
    }

    public static void warn(String message) {
        System.err.println(message);
    }
}
