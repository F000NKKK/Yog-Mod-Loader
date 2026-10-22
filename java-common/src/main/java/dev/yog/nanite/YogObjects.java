package dev.yog.nanite;

import dev.yog.YogNames;
import dev.yog.data.YogData;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * The objects the host holds for Yog mods (what reflection hands back that
 * isn't a number, text, a list or an entity), each by a number in a
 * {@link YogData.Ref} of kind {@link YogNames.NaniteRef#OBJECT}, until the
 * mod lets it go.
 */
public final class YogObjects {
    private YogObjects() {}

    private static final Map<Long, Object> HELD = new ConcurrentHashMap<>();
    private static final AtomicLong NEXT = new AtomicLong(1);

    /** {@code o} held: its reference. */
    public static YogData.Ref hold(Object o) {
        long n = NEXT.getAndIncrement();
        HELD.put(n, o);
        return new YogData.Ref(YogNames.NaniteRef.OBJECT, Long.toString(n));
    }

    /** The object {@code ref} names, or null when it isn't one held. */
    public static Object of(YogData.Ref ref) {
        if (!ref.kind().equals(YogNames.NaniteRef.OBJECT)) return null;
        try {
            return HELD.get(Long.parseLong(ref.id()));
        } catch (NumberFormatException e) {
            return null;
        }
    }

    /** Lets object {@code n} go. */
    public static void release(long n) {
        HELD.remove(n);
    }

    /** How many are held (tests, leaks). */
    public static int held() {
        return HELD.size();
    }
}
