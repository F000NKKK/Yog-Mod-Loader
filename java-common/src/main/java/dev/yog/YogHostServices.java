package dev.yog;

import dev.yog.bus.YogServices;
import dev.yog.data.YogData;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.util.ArrayList;
import java.util.List;

/**
 * The host's actions on the bus ({@code docs/design/bus.md}, stage 3): every
 * action in {@link YogServices#HOSTED} answered by the host method its record
 * names ({@code HOST}: a {@link NativeBridge} method, or {@code Class.method}
 * of another {@code dev.yog} class) taking as many arguments as the request
 * spreads into — one provider for all of them.
 * The request's components are laid out in order (a nested record spreads
 * into its own), converted to the method's parameter types, and its result
 * is the reply: {@code NaN}, {@link Long#MIN_VALUE}, {@link Integer#MIN_VALUE}
 * and {@code null} read as none, a {@code void} method as done.
 */
public final class YogHostServices {
    private YogHostServices() {}

    /** Provides every hosted action; before the mods register, so they can call them. */
    public static void provideAll() {
        for (Class<? extends Record> action : YogServices.HOSTED) {
            try {
                provide(action);
            } catch (ReflectiveOperationException | RuntimeException e) {
                YogLog.error("host action " + action.getName() + " isn't provided", e);
            }
        }
    }

    private static void provide(Class<? extends Record> action) throws ReflectiveOperationException {
        String host = (String) action.getField("HOST").get(null);
        String address = (String) action.getField("ADDRESS").get(null);
        Method method = hostMethod(host, arity(action));
        MethodHandle handle = MethodHandles.publicLookup().unreflect(method);
        Method fromBytes = action.getMethod("fromBytes", byte[].class);
        Class<?>[] types = method.getParameterTypes();
        boolean done = method.getReturnType() == void.class;
        YogBus.provide(YogBus.resolve(address), payload -> {
            try {
                List<Object> args = new ArrayList<>();
                spread(fromBytes.invoke(null, (Object) payload), args);
                if (args.size() != types.length) {
                    YogLog.warn(address + ": " + args.size() + " arguments for " + host + "/" + types.length);
                    return YogBus.Reply.of(YogBus.BAD_PAYLOAD);
                }
                Object[] call = new Object[types.length];
                for (int i = 0; i < types.length; i++) call[i] = convert(args.get(i), types[i]);
                Object out = handle.invokeWithArguments(call);
                return YogBus.Reply.ok(YogData.encode(done ? Boolean.TRUE : reply(out)));
            } catch (Throwable t) {
                YogLog.error(address + " failed", t);
                return YogBus.Reply.of(YogBus.FAILED);
            }
        });
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

    /** How many arguments a request of record class {@code type} spreads into. */
    private static int arity(Class<?> type) {
        if (!type.isRecord()) return 1;
        int n = 0;
        for (RecordComponent c : type.getRecordComponents()) n += arity(c.getType());
        return n;
    }

    /** {@code value}'s arguments: a record's components, each spread; anything else itself. */
    private static void spread(Object value, List<Object> out) throws ReflectiveOperationException {
        if (value instanceof Record r) {
            for (RecordComponent c : r.getClass().getRecordComponents()) spread(c.getAccessor().invoke(r), out);
        } else {
            out.add(value);
        }
    }

    /** {@code value} as a parameter of type {@code type}: numbers to the parameter's width. */
    private static Object convert(Object value, Class<?> type) {
        if (!(value instanceof Number n)) return value;
        if (type == int.class || type == Integer.class) return n.intValue();
        if (type == long.class || type == Long.class) return n.longValue();
        if (type == float.class || type == Float.class) return n.floatValue();
        if (type == double.class || type == Double.class) return n.doubleValue();
        if (type == short.class || type == Short.class) return n.shortValue();
        if (type == byte.class || type == Byte.class) return n.byteValue();
        return value;
    }

    /** A host method's result as the reply: its "none" values as null. */
    private static Object reply(Object out) {
        if (out instanceof Double d && d.isNaN()) return null;
        if (out instanceof Float f && f.isNaN()) return null;
        if (out instanceof Long l && l == Long.MIN_VALUE) return null;
        if (out instanceof Integer i && i == Integer.MIN_VALUE) return null;
        return out;
    }
}
