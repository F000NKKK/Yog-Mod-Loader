package dev.yog.script.threading.tasks;

import dev.yog.YogLog;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.PriorityQueue;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BooleanSupplier;
import java.util.function.Consumer;

/**
 * What a side's tasks go on on — its synchronization context, as .NET's:
 * each thread its own, the server's ticks driving the server thread's and
 * the client's the client's, so what awaits goes on on the side it started
 * on. Each tick runs what's due — by the game's ticks or by real time — in
 * the order it came due.
 */
public final class Scheduler {
    private static final Map<Thread, Scheduler> OF_THREAD = new ConcurrentHashMap<>();
    private long now;
    private long made;
    /** Due at a tick of the game's. */
    private final PriorityQueue<Timer> ticks = new PriorityQueue<>();
    /** Due at a moment of real time ({@link System#nanoTime}). */
    private final PriorityQueue<Timer> clock = new PriorityQueue<>();
    private List<Waiting> waiting = new ArrayList<>();
    /** The list {@link #waiting} is checked into next: the last tick's, emptied. */
    private List<Waiting> spare = new ArrayList<>();

    private Scheduler() {}

    /** The scheduler of the thread asking. */
    public static Scheduler here() {
        return OF_THREAD.computeIfAbsent(Thread.currentThread(), t -> new Scheduler());
    }

    /**
     * Each side's tick drives its thread's tasks: listened to once, where
     * the game runs (the bus is the game's).
     */
    public static void listen() {
        // What's started while the mods load (a mod's constructor) waits on
        // the loading thread: the client's, where a client runs; else (a
        // dedicated server) the server's ticks drive it.
        Scheduler loading = here();
        boolean[] client = {false};
        dev.yog.bus.YogEvents.Server.Game.Tick.on(0, e -> {
            Scheduler server = here();
            server.tick();
            if (!client[0] && loading != server) {
                loading.tick();
            }
            return null;
        });
        dev.yog.bus.YogEvents.Client.Game.Tick.on(0, e -> {
            client[0] = true;
            here().tick();
            return null;
        });
    }

    /** {@code then} run once {@code count} ticks have passed. */
    public void afterTicks(long count, Runnable then) {
        ticks.add(new Timer(now + Math.max(1, count), made++, then));
    }

    /** {@code then} run at the first tick once {@code nanos} of real time have passed. */
    public void afterNanos(long nanos, Runnable then) {
        clock.add(new Timer(System.nanoTime() + nanos, made++, then));
    }

    /** {@code then} run at the first tick {@code condition} holds. */
    public void until(BooleanSupplier condition, Runnable then) {
        waiting.add(new Waiting(condition, then));
    }

    /** {@code action} run every {@code period} ticks; its panic stops it, given to {@code failed}. */
    public void every(long period, Runnable action, Consumer<Throwable> failed) {
        afterTicks(period, () -> {
            try {
                action.run();
            } catch (RuntimeException e) {
                failed.accept(e);
                return;
            }
            every(period, action, failed);
        });
    }

    /** One tick: the timers due and the conditions that hold run. */
    void tick() {
        now++;
        runDue(ticks, now);
        runDue(clock, System.nanoTime());
        if (waiting.isEmpty()) {
            return;
        }
        // The two lists take turns: nothing allocated a tick.
        List<Waiting> checked = waiting;
        waiting = spare;
        spare = checked;
        for (Waiting w : checked) {
            boolean holds;
            try {
                holds = w.condition().getAsBoolean();
            } catch (RuntimeException e) {
                YogLog.error("Yog Script: a task's condition panicked: " + e.getMessage());
                continue;
            }
            if (holds) {
                run(w.then());
            } else {
                waiting.add(w);
            }
        }
        checked.clear();
    }

    /** What's due in {@code queue} by {@code at}, run. */
    private static void runDue(PriorityQueue<Timer> queue, long at) {
        while (!queue.isEmpty() && queue.peek().due() <= at) {
            run(queue.poll().then());
        }
    }

    /** {@code r} run; its panic logged, the others going on (a task's own fails it). */
    private static void run(Runnable r) {
        try {
            r.run();
        } catch (RuntimeException e) {
            YogLog.error("Yog Script: a task panicked: " + e.getMessage());
        }
    }

    /** What's due at tick {@code due}, in the order it was asked for. */
    private record Timer(long due, long order, Runnable then) implements Comparable<Timer> {
        @Override
        public int compareTo(Timer o) {
            return due != o.due ? Long.compare(due, o.due) : Long.compare(order, o.order);
        }
    }

    private record Waiting(BooleanSupplier condition, Runnable then) {}
}
