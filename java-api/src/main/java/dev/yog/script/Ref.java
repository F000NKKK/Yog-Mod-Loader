package dev.yog.script;

/**
 * A {@code ref} or {@code out} parameter's cell: the method reads and writes
 * the caller's variable through {@link #value}, the caller takes it back
 * when the call ends.
 */
public final class Ref<T> {
    public T value;

    /** An {@code out} argument's: no value until the method gives one. */
    public Ref() {
    }

    /** A {@code ref} argument's: the caller's value. */
    public Ref(T value) {
        this.value = value;
    }

    /**
     * A reflection call's {@code ref} or {@code out} argument {@code i} of
     * {@code args}: its cell, kept in {@code cells} to give its value back.
     */
    // The method takes the type its description says, which Invoke checked args[i] against.
    @SuppressWarnings("unchecked")
    public static <T> Ref<T> of(java.lang.Object[] args, int i, Ref<?>[] cells) {
        Ref<T> cell = new Ref<>((T) args[i]);
        cells[i] = cell;
        return cell;
    }

    /** Each cell's value back into {@code args}, as .NET's {@code Invoke} gives a {@code ref} or {@code out} argument's. */
    public static void back(java.lang.Object[] args, Ref<?>[] cells) {
        for (int i = 0; i < cells.length; i++) {
            if (cells[i] != null) {
                args[i] = cells[i].value;
            }
        }
    }
}
