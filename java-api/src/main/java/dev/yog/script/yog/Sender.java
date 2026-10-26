package dev.yog.script.yog;

/** Yog Script's {@code System.Yog.Sender}: who ran a command, and what it's told. */
public final class Sender {
    private final String name;
    private final String uuid;
    private final String dimension;
    private String reply;

    Sender(String name, String uuid, String dimension) {
        this.name = name;
        this.uuid = uuid;
        this.dimension = dimension;
    }

    public String Name() {
        return name;
    }

    /** The console runs commands as no entity: it has no uuid. */
    public boolean IsConsole() {
        return uuid.isEmpty();
    }

    /** A player is its handle, its name; the console isn't one. */
    public String Player() {
        return IsConsole() ? null : name;
    }

    public String Dimension() {
        return dimension;
    }

    public void Reply(String text) {
        reply = text;
    }

    /** What it's told: what it was replied, else nothing. */
    String reply() {
        return reply == null ? "" : reply;
    }
}
