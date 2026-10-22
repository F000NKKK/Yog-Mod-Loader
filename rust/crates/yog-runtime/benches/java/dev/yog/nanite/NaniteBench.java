package dev.yog.nanite;

import dev.yog.data.YogJava;
import java.util.List;

/**
 * Java's side of a nanite call against a jvm/call chain and a plain call,
 * each measured in a loop after the JIT warmed it (ns a call).
 */
public final class NaniteBench {
    private NaniteBench() {}

    private static final int WARM = 200_000;
    private static final int RUNS = 2_000_000;

    public static int twice(int x) {
        return x * 2;
    }

    private interface Op {
        Object run(int i) throws Throwable;
    }

    private static double measure(Op op) throws Throwable {
        Object sink = null;
        for (int i = 0; i < WARM; i++) sink = op.run(i);
        long start = System.nanoTime();
        for (int i = 0; i < RUNS; i++) sink = op.run(i);
        long ns = System.nanoTime() - start;
        if (sink == NaniteBench.class) System.out.print("");
        return (double) ns / RUNS;
    }

    /** {@code plain,nanite,chain}: ns a call. */
    public static String run() throws Throwable {
        String cls = NaniteBench.class.getName();
        int member = YogReflect.resolve(cls, "twice", "(I)I");
        double plain = measure(i -> twice(i));
        double nanite = measure(i -> YogReflect.call(member, null, List.of((long) i)));
        double chain = measure(i -> YogJava.invoke(cls + ".twice($0)", List.of((long) i)));
        return String.format(java.util.Locale.ROOT, "%.1f,%.1f,%.1f", plain, nanite, chain);
    }
}
