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

    /** A {@code switch} no arm of which takes {@code value}. */
    public static YsPanic noArm(Object value, String at) {
        return new YsPanic("no arm of this `switch` takes `" + value + "`", at);
    }

    /** {@code s.Substring(from, length)}, by characters. */
    public static String substring(String s, int from, int length) {
        return s.substring(from, from + length);
    }

    /** {@code list[i]}: a panic, not an exception, when it's out of range. */
    public static <T> T at(List<T> list, int i, String at) {
        if (i < 0 || i >= list.size()) {
            throw new YsPanic("index " + i + " out of range (" + list.size() + " items)", at);
        }
        return list.get(i);
    }
}
