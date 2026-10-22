package dev.yog;

import dev.yog.bus.YogServices;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.Map;

/**
 * The host's actions on the bus ({@code docs/design/bus.md}, stage 3): every
 * action in {@link YogServices#HOSTED} answered by its generated
 * {@code serve} — the request read out of its frame, the host method its
 * record names ({@code HOST}: a {@link NativeBridge} method, or
 * {@code Class.method} of another {@code dev.yog} class) called through a
 * {@code static final} handle with the request's components spread (a
 * nested record into its own), the result packed as the reply:
 * {@code NaN}, {@link Long#MIN_VALUE}, {@link Integer#MIN_VALUE} and
 * {@code null} as none, a {@code void} method as done.
 */
public final class YogHostServices {
    private YogHostServices() {}

    /**
     * Provides every hosted action; before the mods register, so they can
     * call them. A dedicated server leaves the client's out: their host
     * classes are the client's, never loaded there (a call finds no one).
     */
    public static void provideAll() {
        boolean client = YogPlatform.isClient();
        for (Map.Entry<String, YogBus.Handler> action : YogServices.HOSTED) {
            if (!client && action.getKey().startsWith(YogNames.ServiceClient.DOMAIN)) continue;
            YogBus.provide(YogBus.resolve(action.getKey()), action.getValue());
        }
        YogHostMirrors.provide();
    }

    /**
     * The host method {@code host} names, taking as many arguments as
     * {@code type} has, adapted to it once: each argument cast to the
     * parameter's type (narrowing too, as a Java cast), the result an
     * {@code Object} — a {@code void} method's {@link Boolean#TRUE}, done.
     * Null (logged) when there's no such method.
     */
    public static MethodHandle handle(String host, MethodType type) {
        try {
            Method method = hostMethod(host, type.parameterCount());
            MethodHandle mh = MethodHandles.publicLookup().unreflect(method);
            if (method.getReturnType() == void.class) {
                mh = MethodHandles.filterReturnValue(mh, MethodHandles.constant(Object.class, Boolean.TRUE));
            }
            return MethodHandles.explicitCastArguments(mh, type);
        } catch (ReflectiveOperationException | RuntimeException e) {
            YogLog.error("host action " + host + " isn't there", e);
            return null;
        }
    }

    /** The public static method {@code host} names, taking {@code arity} arguments: exactly one. */
    private static Method hostMethod(String host, int arity) throws ReflectiveOperationException {
        int dot = host.lastIndexOf('.');
        Class<?> owner = dot < 0 ? NativeBridge.class : Class.forName(NativeBridge.class.getPackageName() + "." + host.substring(0, dot));
        String name = host.substring(dot + 1);
        Method found = null;
        for (Method m : owner.getMethods()) {
            if (!m.getName().equals(name) || m.getParameterCount() != arity || !Modifier.isStatic(m.getModifiers())) continue;
            if (found != null) throw new NoSuchMethodException(host + "/" + arity + " is overloaded");
            found = m;
        }
        if (found == null) throw new NoSuchMethodException(host + "/" + arity);
        return found;
    }

    /** A host method's result as the reply: its "none" values as null. */
    public static Object none(Object out) {
        if (out instanceof Double d && d.isNaN()) return null;
        if (out instanceof Float f && f.isNaN()) return null;
        if (out instanceof Long l && l == Long.MIN_VALUE) return null;
        if (out instanceof Integer i && i == Integer.MIN_VALUE) return null;
        return out;
    }

    /** The status of action {@code address} that threw {@code t}. */
    public static int failed(String address, Throwable t) {
        YogLog.error(address + " failed", t);
        return YogBus.FAILED;
    }
}
