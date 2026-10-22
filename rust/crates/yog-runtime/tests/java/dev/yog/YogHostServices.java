package dev.yog;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodType;

/** A stand-in for the host's actions (the tests' JVM has no game): none answer. */
public final class YogHostServices {
    private YogHostServices() {}

    public static MethodHandle handle(String host, MethodType type) {
        return null;
    }

    public static Object none(Object out) {
        return out;
    }

    public static int failed(String address, Throwable t) {
        return YogBus.FAILED;
    }
}
