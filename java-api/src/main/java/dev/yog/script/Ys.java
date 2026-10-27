package dev.yog.script;

import dev.yog.YogLog;
import java.util.List;
import java.util.Objects;

/**
 * The built-in macros and the library's helpers as compiled code calls
 * them; {@code at} is the call's place in its source ({@code file:line}).
 */
public final class Ys {
    private Ys() {}

    /** {@code panic!}, {@code todo!}, {@code unreachable!}: never returns. */
    public static <T> T panic(String message, String at) {
        throw new YsPanic(message, at);
    }

    /** {@code assert!(cond, message)}. */
    public static void check(boolean cond, String message, String at) {
        if (!cond) {
            throw new YsPanic(message, at);
        }
    }

    /** {@code assert_eq!(a, b)}. */
    public static void checkEq(Object a, Object b, String at) {
        if (!Objects.equals(a, b)) {
            throw new YsPanic("assertion failed: `" + a + "` isn't `" + b + "`", at);
        }
    }

    /** {@code dbg!(x)}: logged with where, given back. */
    public static <T> T dbg(T value, String what, String at) {
        YogLog.info("[" + at + "] " + what + " = " + value);
        return value;
    }

    /**
     * {@code was}, {@code then} evaluated after it: a type's own {@code x++}'s
     * value ({@code x} as it was, before its step's assignment), a number's
     * {@code GetType()} (its declared type, the value still read).
     */
    public static <T> T before(T was, Object then) {
        return was;
    }

    /**
     * A pattern's name bound inside a condition ({@code x is Mode.Charging(let r)}):
     * its assignment the argument, the condition going on — always true.
     */
    public static boolean bound(Object assigned) {
        return true;
    }

    /** A {@code switch} no arm of which takes {@code value}. */
    public static YsPanic noArm(Object value, String at) {
        return new YsPanic("no arm of this `switch` takes `" + value + "`", at);
    }

    /** {@code list[i]}: a panic, not an exception, when it's out of range. */
    public static <T> T at(List<T> list, int i, String at) {
        if (i < 0 || i >= list.size()) {
            throw new YsPanic("index " + i + " out of range (" + list.size() + " items)", at);
        }
        return list.get(i);
    }

    /** How many items {@code items} has (an array, a string, a list): what {@code ^n} counts back from. */
    public static int lengthOf(Object items) {
        if (items instanceof CharSequence text) {
            return text.length();
        }
        if (items instanceof java.util.Collection<?> list) {
            return list.size();
        }
        if (items != null && items.getClass().isArray()) {
            return java.lang.reflect.Array.getLength(items);
        }
        throw new YsPanic("`" + items + "` has no length to count back from", "");
    }

    /**
     * {@code items[from..to]} ({@code inclusive}) or {@code items[from..<to]}:
     * its slice, of its own kind — an array's, a string's, a list's.
     */
    public static Object slice(Object items, int from, int to, boolean inclusive) {
        int end = inclusive ? to + 1 : to;
        int length = lengthOf(items);
        if (from < 0 || end > length || from > end) {
            throw new YsPanic("range " + from + ".." + to + " out of " + length + " items", "");
        }
        if (items instanceof String text) {
            return text.substring(from, end);
        }
        if (items instanceof dev.yog.script.collections.generic.List<?> list) {
            return list.GetRange(from, end - from);
        }
        Object made = java.lang.reflect.Array.newInstance(items.getClass().getComponentType(), end - from);
        System.arraycopy(items, from, made, 0, end - from);
        return made;
    }
}
