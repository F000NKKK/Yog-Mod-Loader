package dev.yog.script;

/**
 * {@code System.Result<T, TError>}: a value, or why it isn't there — Rust's
 * {@code Result}. Compiled code makes it with {@code new Result.Ok<>(v)} /
 * {@code new Result.Err<>(e)} and takes it apart with {@code instanceof}.
 */
public sealed interface Result<T, TError> permits Result.Ok, Result.Err {
    record Ok<T, TError>(T Value) implements Result<T, TError> {}

    record Err<T, TError>(TError Error) implements Result<T, TError> {}

    default boolean IsOk() {
        return this instanceof Ok;
    }

    default boolean IsErr() {
        return this instanceof Err;
    }

    /** The value; a panic when it's an {@code Err}. */
    default T Unwrap() {
        return Expect("`Unwrap()` on an error");
    }

    /** The value; a panic with {@code message} when it's an {@code Err}. */
    default T Expect(String message) {
        if (this instanceof Ok<T, TError> ok) {
            return ok.Value();
        }
        Err<T, TError> err = (Err<T, TError>) this;
        throw new YsPanic(message + ": " + err.Error(), "");
    }

    /** The value, or {@code fallback} when it's an {@code Err}. */
    default T UnwrapOr(T fallback) {
        return this instanceof Ok<T, TError> ok ? ok.Value() : fallback;
    }
}
