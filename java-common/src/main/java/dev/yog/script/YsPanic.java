package dev.yog.script;

/**
 * A panic of Yog Script code: a bug, with where it was. It unwinds to the
 * handler's boundary (an event handler, a task), which reports it; the
 * game goes on ({@code ys.md} §4.5).
 */
public final class YsPanic extends RuntimeException {
    /** Where in its source the code panicked: {@code file:line}. */
    private final String at;

    public YsPanic(String message, String at) {
        super(message + " (" + at + ")");
        this.at = at;
    }

    public String at() {
        return at;
    }
}
