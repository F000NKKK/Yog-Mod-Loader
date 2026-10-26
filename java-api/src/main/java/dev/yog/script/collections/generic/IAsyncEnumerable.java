package dev.yog.script.collections.generic;

/** Yog Script's {@code IAsyncEnumerable<T>}: items as they're ready, gone through by {@code await foreach}. */
public interface IAsyncEnumerable<T> {
    IAsyncEnumerator<T> GetAsyncEnumerator();
}
