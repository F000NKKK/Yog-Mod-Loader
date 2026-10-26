package dev.yog.script;

import java.util.Objects;

/**
 * {@code System.Result<T, TError>}: a value, or why it isn't there — Rust's
 * {@code Result}, an enum as every enum is: its cases ({@code Ok}, {@code Err})
 * numbered, each carrying its data. Compiled code makes one with
 * {@code new Result.Ok<>(v)} / {@code new Result.Err<>(e)} and takes it
 * apart with {@code instanceof}.
 */
public sealed class Result<T, TError> extends Enum permits Result.Ok, Result.Err {
    /** Its cases' numbers, in order (as any enum's, from 0). */
    private static final long OK = 0;
    private static final long ERR = 1;
    private static final Table<Result> TABLE =
            new Table<>(Result.class, Result::new, new String[] {"Ok", "Err"}, new long[] {OK, ERR}).Carrying(0, 1);

    protected Result(long value) {
        super(value);
    }

    @Override
    protected Table<?> table() {
        return TABLE;
    }

    /** The value. */
    public static final class Ok<T, TError> extends Result<T, TError> {
        public final T Value;

        public Ok(T Value) {
            super(OK);
            this.Value = Value;
        }

        public T Value() {
            return Value;
        }

        @Override
        public boolean equals(Object other) {
            return other instanceof Ok<?, ?> that && Objects.equals(Value, that.Value);
        }

        @Override
        public int hashCode() {
            return Objects.hash(OK, Value);
        }

        @Override
        public String toString() {
            return "Ok(" + dev.yog.script.host.Object.ToString(Value) + ")";
        }
    }

    /** Why there's no value. */
    public static final class Err<T, TError> extends Result<T, TError> {
        public final TError Error;

        public Err(TError Error) {
            super(ERR);
            this.Error = Error;
        }

        public TError Error() {
            return Error;
        }

        @Override
        public boolean equals(Object other) {
            return other instanceof Err<?, ?> that && Objects.equals(Error, that.Error);
        }

        @Override
        public int hashCode() {
            return Objects.hash(ERR, Error);
        }

        @Override
        public String toString() {
            return "Err(" + dev.yog.script.host.Object.ToString(Error) + ")";
        }
    }

    public boolean IsOk() {
        return this instanceof Ok;
    }

    public boolean IsErr() {
        return this instanceof Err;
    }

    /** The value; a panic when it's an {@code Err}. */
    public T Unwrap() {
        return Expect("`Unwrap()` on an error");
    }

    /** The value; a panic with {@code message} when it's an {@code Err}. */
    public T Expect(String message) {
        if (this instanceof Ok<T, TError> ok) {
            return ok.Value;
        }
        throw new YsPanic(message + ": " + this, "");
    }

    /** The value, or {@code fallback} when it's an {@code Err}. */
    public T UnwrapOr(T fallback) {
        return this instanceof Ok<T, TError> ok ? ok.Value : fallback;
    }
}
