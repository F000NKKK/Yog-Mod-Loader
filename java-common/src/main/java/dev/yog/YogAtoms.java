package dev.yog;

import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Atoms ({@code yog_data::atom}): names that repeat across messages, sent on
 * the bus as their number. The table is the runtime's, one a process; Java
 * keeps one {@code String} an atom, asking the runtime only the first time.
 */
public final class YogAtoms {
    private YogAtoms() {}

    private static final int START = 1024;
    private static final Map<String, Integer> NUMBERS = new ConcurrentHashMap<>();
    /** Names by number, as far as they're known here. */
    private static volatile String[] names = new String[START];

    /** {@code name}'s number. */
    public static int number(String name) {
        Integer n = NUMBERS.get(name);
        if (n != null) return n;
        int made = nativeAtom(name);
        remember(made, name);
        return made;
    }

    /** Atom {@code n}'s name; null when there's none. */
    public static String name(int n) {
        String[] known = names;
        if (n >= 0 && n < known.length && known[n] != null) return known[n];
        String name = nativeAtomName(n);
        if (name != null) remember(n, name);
        return name;
    }

    private static synchronized void remember(int n, String name) {
        if (n < 0) return;
        String[] known = names;
        if (n >= known.length) known = Arrays.copyOf(known, Math.max(known.length * 2, n + 1));
        // The one String this name has: the one already kept, if any.
        String kept = known[n] != null ? known[n] : name;
        known[n] = kept;
        names = known;
        NUMBERS.putIfAbsent(kept, n);
    }

    private static native int nativeAtom(String name);

    private static native String nativeAtomName(int n);
}
