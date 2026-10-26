package dev.yog.script.collections.generic;

import dev.yog.script.threading.tasks.Task;
import dev.yog.script.threading.tasks.Task1;

/** Yog Script's {@code IAsyncEnumerator<T>}: {@code MoveNextAsync} waits for the next item, {@code Current} is it. */
public interface IAsyncEnumerator<T> extends dev.yog.script.IAsyncDisposable {
    T Current();

    Task1<Boolean> MoveNextAsync();

    @Override
    default Task DisposeAsync() {
        return Task.CompletedTask();
    }
}
