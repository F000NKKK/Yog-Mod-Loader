package dev.yog.script;

/**
 * Yog Script's {@code System.Units}: the game's units of time as numbers,
 * a quantity of seconds made one of ticks where ticks are wanted.
 */
public final class Units {
    /** How many ticks the game runs a second. */
    public static final double TicksPerSecond = 20.0;

    private Units() {}

    public static double ToTicks(double seconds) {
        return seconds * TicksPerSecond;
    }
}
