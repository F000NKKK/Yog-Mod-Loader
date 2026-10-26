package dev.yog.script.threading;

import dev.yog.script.Action;
import dev.yog.script.IDisposable;
import dev.yog.script.YsPanic;

/** Yog Script's {@code System.Threading.CancellationToken}: whether work should stop. */
public final class CancellationToken {
    private static final CancellationToken NONE = new CancellationToken(null);
    /** Where a cancelled token's panic says it was. */
    private static final String CANCELLED_AT = "a cancellation token";

    private final CancellationTokenSource source;

    CancellationToken(CancellationTokenSource source) {
        this.source = source;
    }

    public static CancellationToken None() {
        return NONE;
    }

    public boolean IsCancellationRequested() {
        return source != null && source.IsCancellationRequested();
    }

    public boolean CanBeCanceled() {
        return source != null;
    }

    public IDisposable Register(Action callback) {
        return source == null ? () -> {} : source.register(callback);
    }

    public void ThrowIfCancellationRequested() {
        if (IsCancellationRequested()) {
            throw new YsPanic("the operation was cancelled", CANCELLED_AT);
        }
    }
}
