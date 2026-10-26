package dev.yog.script.threading;

import dev.yog.script.Func2;
import dev.yog.script.TypedObject;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import java.util.Objects;

/**
 * Yog Script's {@code System.Threading.Atomic<T>}: a value read and changed
 * by many threads without a lock — each access at its {@link MemoryOrder}
 * (a {@code VarHandle} access mode). A whole number's arithmetic keeps its
 * width; a compare is by value, as a number's or a {@code bool}'s is.
 */
public final class Atomic<T> extends TypedObject {
    private static final VarHandle VALUE;

    static {
        try {
            VALUE = MethodHandles.lookup().findVarHandle(Atomic.class, "value", Object.class);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    @SuppressWarnings("unused") // Reached through VALUE.
    private volatile Object value;

    public Atomic(T value) {
        this.value = value;
    }

    public T Load() {
        return Load(MemoryOrder.SeqCst);
    }

    @SuppressWarnings("unchecked")
    public T Load(MemoryOrder order) {
        if (order == MemoryOrder.Relaxed) return (T) VALUE.getOpaque(this);
        if (order == MemoryOrder.Acquire) return (T) VALUE.getAcquire(this);
        return (T) VALUE.getVolatile(this);
    }

    public void Store(T value) {
        Store(value, MemoryOrder.SeqCst);
    }

    public void Store(T value, MemoryOrder order) {
        if (order == MemoryOrder.Relaxed) VALUE.setOpaque(this, value);
        else if (order == MemoryOrder.Release) VALUE.setRelease(this, value);
        else VALUE.setVolatile(this, value);
    }

    public T Exchange(T value) {
        return Exchange(value, MemoryOrder.SeqCst);
    }

    @SuppressWarnings("unchecked")
    public T Exchange(T value, MemoryOrder order) {
        if (order == MemoryOrder.Acquire) return (T) VALUE.getAndSetAcquire(this, value);
        if (order == MemoryOrder.Release) return (T) VALUE.getAndSetRelease(this, value);
        return (T) VALUE.getAndSet(this, value);
    }

    public T CompareExchange(T expected, T value) {
        return CompareExchange(expected, value, MemoryOrder.SeqCst);
    }

    /** By value: the object read is the one swapped, so a boxed number's identity doesn't matter. */
    @SuppressWarnings("unchecked")
    public T CompareExchange(T expected, T value, MemoryOrder order) {
        while (true) {
            Object current = Load(order);
            if (!Objects.equals(current, expected)) {
                return (T) current;
            }
            if (VALUE.compareAndSet(this, current, value)) {
                return (T) current;
            }
        }
    }

    public T Add(T delta) {
        return Add(delta, MemoryOrder.SeqCst);
    }

    public T Add(T delta, MemoryOrder order) {
        return Update(current -> sum(current, delta));
    }

    public T Increment() {
        return Increment(MemoryOrder.SeqCst);
    }

    public T Increment(MemoryOrder order) {
        return Update(current -> sum(current, one(current, 1)));
    }

    public T Decrement() {
        return Decrement(MemoryOrder.SeqCst);
    }

    public T Decrement(MemoryOrder order) {
        return Update(current -> sum(current, one(current, -1)));
    }

    @SuppressWarnings("unchecked")
    public T Update(Func2<T, T> change) {
        while (true) {
            Object current = VALUE.getVolatile(this);
            T next = change.Invoke((T) current);
            if (VALUE.compareAndSet(this, current, next)) {
                return next;
            }
        }
    }

    /** {@code a + b}, of {@code a}'s width. */
    @SuppressWarnings("unchecked")
    private static <T> T sum(T a, T b) {
        if (a instanceof Integer x) return (T) Integer.valueOf(x + (Integer) b);
        if (a instanceof Long x) return (T) Long.valueOf(x + (Long) b);
        if (a instanceof Short x) return (T) Short.valueOf((short) (x + (Short) b));
        if (a instanceof Byte x) return (T) Byte.valueOf((byte) (x + (Byte) b));
        if (a instanceof Character x) return (T) Character.valueOf((char) (x + (Character) b));
        throw new IllegalArgumentException("an Atomic of " + a.getClass().getSimpleName() + " isn't added to");
    }

    /** {@code n} of {@code like}'s width. */
    @SuppressWarnings("unchecked")
    private static <T> T one(T like, int n) {
        if (like instanceof Long) return (T) Long.valueOf(n);
        if (like instanceof Short) return (T) Short.valueOf((short) n);
        if (like instanceof Byte) return (T) Byte.valueOf((byte) n);
        if (like instanceof Character) return (T) Character.valueOf((char) n);
        return (T) Integer.valueOf(n);
    }
}
