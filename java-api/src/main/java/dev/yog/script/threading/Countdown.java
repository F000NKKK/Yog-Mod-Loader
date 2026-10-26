package dev.yog.script.threading;

import dev.yog.script.TimeSpan;
import dev.yog.script.YsPanic;
import dev.yog.script.threading.tasks.Task;

/** Yog Script's {@code System.Threading.Countdown} ({@code CountdownEvent}): set once signalled so many times. */
public final class Countdown {
    private int count;
    private final Task done = new Task();

    public Countdown(int initialCount) {
        this.count = initialCount;
        if (initialCount == 0) {
            done.finish();
        }
    }

    public boolean Signal() {
        return Signal(1);
    }

    public boolean Signal(int signalCount) {
        boolean now;
        synchronized (this) {
            if (signalCount > count) {
                throw new YsPanic("signalled more than its count", "Countdown.Signal");
            }
            count -= signalCount;
            now = count == 0;
        }
        if (now) {
            done.finish();
        }
        return now;
    }

    public void AddCount() {
        AddCount(1);
    }

    public synchronized void AddCount(int signalCount) {
        if (count == 0) {
            throw new YsPanic("a countdown that's set isn't counted again", "Countdown.AddCount");
        }
        count += signalCount;
    }

    public void Wait() {
        done.Wait();
    }

    public boolean Wait(TimeSpan timeout) {
        return done.Wait(timeout);
    }

    public Task WaitAsync() {
        return done;
    }

    public synchronized int CurrentCount() {
        return count;
    }

    public boolean IsSet() {
        return done.IsCompleted();
    }
}
