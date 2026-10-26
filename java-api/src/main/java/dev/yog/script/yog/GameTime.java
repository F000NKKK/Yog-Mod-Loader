package dev.yog.script.yog;

import dev.yog.script.Action;
import dev.yog.script.Func1;
import dev.yog.script.threading.tasks.Scheduler;
import dev.yog.script.threading.tasks.Task;

/** Yog Script's {@code Yog.GameTime}: the game's time, its side's ticks. */
public final class GameTime {
    private GameTime() {}

    public static Task Delay(double delay) {
        Task t = new Task();
        Scheduler.here().afterTicks(Math.round(delay), t::finish);
        return t;
    }

    public static Task NextTick() {
        return Delay(1);
    }

    public static Task Until(Func1<Boolean> condition) {
        Task t = new Task();
        Scheduler.here().until(condition::Invoke, t::finish);
        return t;
    }

    public static Task Every(double period, Action action) {
        Task t = new Task();
        Scheduler.here().every(Math.max(1, Math.round(period)), action::Invoke, t::fail);
        return t;
    }
}
