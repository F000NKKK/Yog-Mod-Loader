package dev.yog.script.threading.channels;

import dev.yog.script.TypedObject;
import dev.yog.script.YsPanic;
import dev.yog.script.collections.generic.IAsyncEnumerable;
import dev.yog.script.collections.generic.IAsyncEnumerator;
import dev.yog.script.threading.tasks.Task;
import dev.yog.script.threading.tasks.Task1;

/** Yog Script's {@code ChannelReader<T>}: a channel's reading end. */
public final class ChannelReader<T> extends TypedObject {
    /** Where a read past the end says it panicked. */
    private static final String READ_AT = "ChannelReader.ReadAsync";

    private final Channel1<T> of;

    ChannelReader(Channel1<T> of) {
        this.of = of;
    }

    public T TryRead() {
        return of.tryRead();
    }

    public Task1<Boolean> WaitToReadAsync() {
        return of.waitToRead();
    }

    /** The next item, waited for: an item waited for, tried, waited for again if another reader took it. */
    public Task1<T> ReadAsync() {
        Task1<T> out = new Task1<>();
        next(out);
        return out;
    }

    private void next(Task1<T> out) {
        T now = of.tryRead();
        if (now != null) {
            out.finish(now);
            return;
        }
        Task1<Boolean> more = of.waitToRead();
        more.onDoneUncaptured(() -> {
            if (Boolean.TRUE.equals(more.Result())) {
                next(out);
            } else {
                out.fail(new YsPanic("the channel is completed and read", READ_AT));
            }
        });
    }

    public T Read() {
        return ReadAsync().Result();
    }

    public IAsyncEnumerable<T> ReadAllAsync() {
        return () -> new IAsyncEnumerator<T>() {
            private T current;

            @Override
            public T Current() {
                return current;
            }

            @Override
            public Task1<Boolean> MoveNextAsync() {
                Task1<Boolean> moved = new Task1<>();
                step(moved);
                return moved;
            }

            private void step(Task1<Boolean> moved) {
                T now = of.tryRead();
                if (now != null) {
                    current = now;
                    moved.finish(true);
                    return;
                }
                Task1<Boolean> more = of.waitToRead();
                more.onDoneUncaptured(() -> {
                    if (Boolean.TRUE.equals(more.Result())) {
                        step(moved);
                    } else {
                        moved.finish(false);
                    }
                });
            }
        };
    }

    public Task Completion() {
        return of.completion();
    }

    public int Count() {
        return of.count();
    }
}
