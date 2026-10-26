package dev.yog.script.threading.channels;

import dev.yog.script.TypedObject;
import dev.yog.script.threading.tasks.Task;

/** Yog Script's {@code ChannelWriter<T>}: a channel's writing end. */
public final class ChannelWriter<T> extends TypedObject {
    private final Channel1<T> of;

    ChannelWriter(Channel1<T> of) {
        this.of = of;
    }

    public boolean TryWrite(T item) {
        return of.tryWrite(item);
    }

    public Task WriteAsync(T item) {
        return of.writeAsync(item);
    }

    public void Write(T item) {
        of.writeAsync(item).Wait();
    }

    public void Complete() {
        of.complete();
    }
}
