package dev.yog.script;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.LongFunction;

/**
 * Every enum's base, as .NET's {@code System.Enum}: a value is its number
 * (its underlying integer, kept as a {@code long}: a {@code u64}'s bits
 * too) and — a case declaring some ({@code Charging(i32 Rate)}) — its
 * data, a subclass's fields. A value made of a number ({@code (Mode)5},
 * {@code A | B}) carries none: one object per number ({@link Table#of}),
 * so {@code ==} compares them.
 */
public abstract class Enum implements Comparable<Enum> {
    /** What separates a flags enum's names in its text ({@code "A, B"}). */
    private static final String FLAGS_SEP = ", ";
    /** A table its class's initializer just made, until {@link #TABLES} takes it. */
    private static final Map<Class<?>, Table<?>> MADE = new ConcurrentHashMap<>();
    /** Each enum type's table, kept with its class (gone with a reloaded mod's). */
    private static final ClassValue<Table<?>> TABLES = new ClassValue<>() {
        @Override
        protected Table<?> computeValue(Class<?> type) {
            try {
                Class.forName(type.getName(), true, type.getClassLoader());
            } catch (ClassNotFoundException e) {
                throw new IllegalStateException(e);
            }
            return MADE.remove(type);
        }
    };

    /** Asked for while its class is still being made (a case's own
     * initializer): the table as made so far, not kept as the class's. */
    private static Table<?> stillMaking(Class<?> type) {
        TABLES.remove(type);
        return MADE.get(type);
    }

    private final long value;

    protected Enum(long value) {
        this.value = value;
    }

    /** Its underlying integer. */
    public final long value() {
        return value;
    }

    /** Its type's table: its names, its width. */
    protected abstract Table<?> table();

    /** Whether every bit of {@code flag} is set in it. */
    public boolean HasFlag(Enum flag) {
        return (value & flag.value) == flag.value;
    }

    /** {@code a} before {@code b}: by their values, an unsigned type's as unsigned. */
    public static int compare(Enum a, Enum b) {
        return a.table().signed ? Long.compare(a.value, b.value) : Long.compareUnsigned(a.value, b.value);
    }

    @Override
    public int compareTo(Enum other) {
        return compare(this, other);
    }

    public int CompareTo(Object target) {
        return compare(this, (Enum) target);
    }

    /** Its name; a flags enum's names of its bits ({@code "A, B"}); else its number. */
    @Override
    public String toString() {
        return table().written(value);
    }

    public String ToString() {
        return toString();
    }

    // Its type's statics, its class handed first ([Reified]); what they give
    // made the call's own type by the compiler.

    /** Its named values, in order: an array of {@code type}. */
    public static Object GetValues(Class<?> type) {
        List<? extends Enum> named = tableOf(type).named();
        Object out = java.lang.reflect.Array.newInstance(type, named.size());
        for (int i = 0; i < named.size(); i++) {
            java.lang.reflect.Array.set(out, i, named.get(i));
        }
        return out;
    }

    public static String[] GetNames(Class<?> type) {
        return tableOf(type).names.clone();
    }

    public static Enum Parse(Class<?> type, String value) {
        Enum parsed = TryParse(type, value);
        if (parsed == null) {
            throw new IllegalArgumentException("`" + value + "` isn't a `" + type.getSimpleName() + "`");
        }
        return parsed;
    }

    /** {@code value} by its name (a flags enum's names joined by commas), or its number; {@code null} for neither. */
    public static Enum TryParse(Class<?> type, String value) {
        return tableOf(type).parsed(value);
    }

    public static boolean IsDefined(Class<?> type, Enum value) {
        return tableOf(type).index(value.value) >= 0;
    }

    /** {@code type}'s table, its class made ready first (its table is made with it). */
    private static Table<?> tableOf(Class<?> type) {
        Table<?> t = TABLES.get(type);
        return t != null ? t : stillMaking(type);
    }

    /** An enum type's cases: their names and numbers, which carry data, its width, whether it's of flags; each value without data made once. */
    public static final class Table<E extends Enum> {
        private static final int LONG_BITS = 64;

        private final LongFunction<E> make;
        private final String[] names;
        private final long[] values;
        private final Map<Long, E> made = new ConcurrentHashMap<>();
        private long mask = -1L;
        private boolean signed = true;
        private boolean flags;
        /** The cases carrying data, by index: no number makes one. */
        private final java.util.Set<Integer> carrying = new java.util.HashSet<>();

        public Table(Class<E> type, LongFunction<E> make, String[] names, long[] values) {
            this.make = make;
            this.names = names;
            this.values = values;
            MADE.put(type, this);
        }

        /** Its underlying type: {@code bits} wide, {@code signed} or not ({@code : u8} is 8, unsigned). */
        public Table<E> Width(int bits, boolean signed) {
            this.mask = bits >= LONG_BITS ? -1L : (1L << bits) - 1;
            this.signed = signed;
            return this;
        }

        /** The cases at {@code indexes} carry data: their values are their classes' objects. */
        public Table<E> Carrying(int... indexes) {
            for (int i : indexes) {
                carrying.add(i);
            }
            return this;
        }

        /** A {@code [Flags]} enum's: its text names its bits. */
        public Table<E> Flags() {
            this.flags = true;
            return this;
        }

        /** The value {@code value} (cut to its width): the one object of it; a panic for a case carrying data, which a number can't make. */
        public E of(long value) {
            long cut = value & mask;
            if (signed && mask != -1L && (cut & ~(mask >>> 1)) != 0) {
                cut |= ~mask;
            }
            int named = index(cut);
            if (named >= 0 && carrying.contains(named)) {
                throw new IllegalStateException("`" + names[named] + "` carries data: its number alone doesn't make one");
            }
            return made.computeIfAbsent(cut, make::apply);
        }

        int index(long value) {
            for (int i = 0; i < values.length; i++) {
                if (values[i] == value) {
                    return i;
                }
            }
            return -1;
        }

        /** The cases without data, as values. */
        List<E> named() {
            List<E> out = new ArrayList<>();
            for (int i = 0; i < values.length; i++) {
                if (!carrying.contains(i)) {
                    out.add(of(values[i]));
                }
            }
            return out;
        }

        String written(long value) {
            int i = index(value);
            if (i >= 0) {
                return names[i];
            }
            if (flags && value != 0) {
                StringBuilder out = new StringBuilder();
                long left = value;
                for (int k = 0; k < values.length; k++) {
                    if (!carrying.contains(k) && values[k] != 0 && (left & values[k]) == values[k]) {
                        out.append(out.length() == 0 ? "" : FLAGS_SEP).append(names[k]);
                        left &= ~values[k];
                    }
                }
                if (left == 0) {
                    return out.toString();
                }
            }
            return signed ? Long.toString(value) : Long.toUnsignedString(value);
        }

        E parsed(String text) {
            long value = 0;
            for (String part : text.split(",")) {
                String name = part.trim();
                int i = java.util.Arrays.asList(names).indexOf(name);
                if (i >= 0) {
                    value |= values[i];
                    continue;
                }
                try {
                    value |= Long.parseLong(name);
                } catch (NumberFormatException e) {
                    return null;
                }
            }
            return of(value);
        }
    }
}
