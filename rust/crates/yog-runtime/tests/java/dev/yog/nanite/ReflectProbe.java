package dev.yog.nanite;

import dev.yog.data.YogData;
import java.util.List;

/** Reflection as a nanite uses it, on a class of its own, for the runtime's JVM test. */
public final class ReflectProbe {
    private int secret = 7;

    private String twice(String s, int n) {
        return s.repeat(n);
    }

    /** What each step gave, joined. */
    public static String run() throws Throwable {
        String cls = ReflectProbe.class.getName();
        int make = YogReflect.resolve(cls, "<init>", "()V");
        Object probe = YogReflect.call(make, null, List.of());
        int twice = YogReflect.resolve(cls, "twice", "(Ljava/lang/String;I)Ljava/lang/String;");
        Object out = YogReflect.call(twice, probe, List.of("ab", 3L));
        int secret = YogReflect.resolve(cls, "secret", "I");
        Object before = YogReflect.get(secret, probe);
        YogReflect.set(secret, probe, 42L);
        Object after = YogReflect.get(secret, probe);
        int missing = YogReflect.resolve(cls, "nope", "()V");
        long held = YogObjects.held();
        YogReflect.release(new long[] {Long.parseLong(((YogData.Ref) probe).id())});
        return String.join(",", String.valueOf(out), String.valueOf(before), String.valueOf(after),
                String.valueOf(missing == Integer.MIN_VALUE), String.valueOf(held), String.valueOf(YogObjects.held()),
                String.valueOf(YogReflect.resolve(cls, "twice", "(Ljava/lang/String;I)Ljava/lang/String;") == twice));
    }

    /** The same members by the names a mapping table gives them (`NamedProbe#double`, `#hidden`). */
    public static String runMapped() throws Throwable {
        String cls = "dev.yog.nanite.NamedProbe";
        int make = YogReflect.resolve(cls, "<init>", "()V");
        Object probe = YogReflect.call(make, null, List.of());
        int twice = YogReflect.resolve(cls, "double", "(Ljava/lang/String;I)Ljava/lang/String;");
        int secret = YogReflect.resolve(cls, "hidden", "I");
        return YogReflect.call(twice, probe, List.of("x", 2L)) + "," + YogReflect.get(secret, probe);
    }
}
