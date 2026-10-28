package dev.yog.script.threading.tasks;

/**
 * A delay's task ({@code Task.Delay}: real time; {@code GameTime.Delay}:
 * a side's ticks), knowing how much of it is left — what a {@code saved
 * async} method waiting on it saves, to wait the rest after a restart
 * (docs/design/storage.md S9).
 */
public final class Delayed extends Task {
    /** Whether it counts its side's ticks (else real time, in nanoseconds). */
    private final boolean ticks;
    /** When it's due: the scheduler's tick, or {@link System#nanoTime}. */
    private final long due;
    /** The scheduler counting its ticks ({@code null} for real time). */
    private final Scheduler on;

    private Delayed(boolean ticks, long due, Scheduler on) {
        this.ticks = ticks;
        this.due = due;
        this.on = on;
    }

    /** Done after {@code count} of {@code on}'s ticks. */
    public static Delayed ticks(Scheduler on, long count) {
        Delayed d = new Delayed(true, on.ticksSoFar() + Math.max(1, count), on);
        on.afterTicks(count, d::finish);
        return d;
    }

    /** Done after {@code nanos} of real time: at {@code on}'s tick, else by the pool's timer. */
    static Delayed nanos(Scheduler on, long nanos) {
        Delayed d = new Delayed(false, System.nanoTime() + nanos, null);
        if (on != null) {
            on.afterNanos(nanos, d::finish);
        } else {
            dev.yog.script.threading.ThreadPool.later(nanos, d::finish);
        }
        return d;
    }

    /** Whether it counts ticks (else real time). */
    public boolean countsTicks() {
        return ticks;
    }

    /** What's left of it: ticks, or nanoseconds of real time; none once it's due. */
    public long left() {
        return Math.max(0, due - (ticks ? on.ticksSoFar() : System.nanoTime()));
    }

    /** A delay of {@code left} (ticks or nanoseconds, as {@code countsTicks}) from now, on this thread's scheduler. */
    public static Delayed again(boolean countsTicks, long left) {
        Scheduler here = countsTicks ? Scheduler.here() : Scheduler.current();
        return countsTicks ? ticks(here, left) : nanos(here, left);
    }
}
