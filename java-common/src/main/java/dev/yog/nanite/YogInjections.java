package dev.yog.nanite;

import dev.yog.YogBus;
import dev.yog.YogNames;
import dev.yog.bus.YogEvents;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import org.objectweb.asm.Type;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Where every generated nanite lands ({@link YogNaniteGen}): its point is
 * emitted on the bus ({@code yog:event/nanite/inject}, keyed by the point)
 * for the mods' handlers, and what they made of it applied — an inject
 * cancelled or returning, a value modified, a call answered (or made as it
 * was). Nothing is packed when no one listens.
 */
public final class YogInjections {
    private YogInjections() {}

    /** What {@link #redirect} hands back when no handler answered: make the call. */
    public static final Object NOT_ANSWERED = new Object();

    /** Each generated nanite's point, the type of its value, the call a redirect stands in for. */
    private static volatile String[] points = new String[0];
    private static volatile Type[] values = new Type[0];
    private static volatile YogNaniteGen.Call[] calls = new YogNaniteGen.Call[0];
    private static volatile List<List<YogNaniteLocals.Local>> locals = List.of();
    private static volatile int[] addresses = new int[0];

    /** What the generated nanites are numbered by. */
    static void points(List<String> keys, List<YogNaniteGen.Made> made) {
        known = true;
        points = keys.toArray(String[]::new);
        values = made.stream().map(YogNaniteGen.Made::value).toArray(Type[]::new);
        calls = made.stream().map(YogNaniteGen.Made::call).toArray(YogNaniteGen.Call[]::new);
        locals = made.stream().map(YogNaniteGen.Made::locals).toList();
        addresses = new int[keys.size()];
    }

    /** Whether this copy of the class knows the points (set by a test, or learnt here). */
    private static volatile boolean known;

    /**
     * The points, learnt the first time a nanite fires: the generated code
     * reaches this class through the game's class loader, which may hold
     * another copy of it than the one the Mixin plugin ran in.
     */
    private static void know() {
        if (known) return;
        synchronized (YogInjections.class) {
            if (known) return;
            try {
                YogNanites.Made all = YogNanites.made();
                points(all.points().stream().map(dev.yog.bus.YogTypes.Inject::point).toList(), all.made());
                dev.yog.YogLog.info("nanites: " + points.length + " point(s) known where the game runs");
            } catch (RuntimeException | LinkageError e) {
                dev.yog.YogLog.error("nanites: their points can't be known here", e);
                known = true;
            }
        }
    }

    private static boolean heard(int n) {
        know();
        if (n < 0 || n >= points.length) return false;
        int at = addresses[n];
        if (at == 0) {
            at = YogBus.keyed(YogNames.EventNanite.INJECT, points[n]);
            addresses[n] = at;
            dev.yog.YogLog.info("nanites: point " + n + " reached first (" + points[n] + "), handlers: " + YogBus.heard(at));
        }
        return YogBus.heard(at);
    }

    /** What a point lends its handlers besides the receiver and arguments. */
    private record Lent(Object value, Object original, Object[] locals) {}

    /** No values: no arguments, no locals. */
    private static final Object[] NONE = new Object[0];

    /** Point {@code n} emitted (its objects lent for the call): what the handlers made of it. */
    private static YogEvents.Nanite.Injected emit(int n, Object self, Object[] args, Object value, Object original) {
        return emit(n, self, args, new Lent(value, original, NONE));
    }

    private static YogEvents.Nanite.Injected emit(int n, Object self, Object[] args, Lent lent) {
        return YogObjects.borrowing(() -> {
            Object originalRef = lent.original() == null ? null : YogObjects.hold(lent.original());
            return new YogEvents.Nanite.Injected(points[n], YogReflect.toData(self), data(args),
                    YogReflect.toData(lent.value()), false, false, data(lent.locals()), originalRef).emitChanged();
        });
    }

    private static List<Object> data(Object[] values) {
        List<Object> out = new ArrayList<>(values.length);
        for (Object a : values) out.add(YogReflect.toData(a));
        return out;
    }

    /**
     * Inject {@code n} reached in {@code self} (null: static) with
     * {@code args} and the locals it asked for: the locals as the handlers
     * left them (null when no one listened), for the handler to write back.
     */
    public static Object[] fire(int n, Object self, Object[] args, CallbackInfo ci, Object[] localValues) {
        if (!heard(n)) return null;
        Object value = ci instanceof CallbackInfoReturnable<?> r ? r.getReturnValue() : null;
        YogEvents.Nanite.Injected out = emit(n, self, args, new Lent(value, null, localValues));
        if (out == null) return null;
        if (out.cancelled() && ci.isCancellable()) {
            if (out.returned() && ci instanceof CallbackInfoReturnable<?> r) {
                returning(r, YogReflect.toJava(out.value(), boxed(values[n])));
            } else {
                ci.cancel();
            }
        }
        return locals(n, out.locals());
    }

    /** What the handlers left of inject {@code n}'s locals, each its local's type. */
    private static Object[] locals(int n, List<Object> data) {
        List<YogNaniteLocals.Local> asked = locals.get(n);
        Object[] out = new Object[asked.size()];
        for (int i = 0; i < out.length && i < data.size(); i++) out[i] = YogReflect.toJava(data.get(i), boxed(asked.get(i).type()));
        return out;
    }

    /** Modify {@code n}'s value {@code value}: as a handler made it, else as it was. */
    public static Object modify(int n, Object self, Object value) {
        if (!heard(n)) return value;
        YogEvents.Nanite.Injected out = emit(n, self, NONE, value, null);
        if (out == null || !out.returned()) return value;
        return YogReflect.toJava(out.value(), boxed(values[n]));
    }

    /** Redirect {@code n}'s call with {@code args} (the receiver first): a handler's answer, else {@link #NOT_ANSWERED}. */
    public static Object redirect(int n, Object self, Object[] args) {
        if (!heard(n)) return NOT_ANSWERED;
        YogEvents.Nanite.Injected out = emit(n, self, args, null, original(n));
        if (out == null || !out.returned()) return NOT_ANSWERED;
        return YogReflect.toJava(out.value(), boxed(values[n]));
    }

    /** Added method {@code n} called on {@code self} with {@code args}: a handler's answer, else its type's zero. */
    public static Object call(int n, Object self, Object[] args) {
        Class<?> type = boxed(values[n]);
        YogEvents.Nanite.Injected out = heard(n) ? emit(n, self, args, null, null) : null;
        if (out != null && out.returned()) return YogReflect.toJava(out.value(), type);
        return zero(type);
    }

    /** A type's zero, boxed (null for a reference). */
    private static Object zero(Class<?> type) {
        if (type == Boolean.class) return false;
        if (type == Character.class) return (char) 0;
        if (type == Object.class) return null;
        return YogReflect.toJava(0L, type);
    }

    /** Redirect {@code n}'s call as a function of its arguments (the receiver first), for handlers to make. */
    private static Function<List<Object>, Object> original(int n) {
        YogNaniteGen.Call call = calls[n];
        if (call.isField()) return field(call);
        return args -> {
            try {
                Class<?> owner = Class.forName(call.owner().replace('/', '.'), false, YogInjections.class.getClassLoader());
                MethodType type = MethodType.fromMethodDescriptorString(call.desc(), owner.getClassLoader());
                Method m = method(owner, call.name(), type.parameterArray());
                m.setAccessible(true);
                int first = call.isStatic() ? 0 : 1;
                Object receiver = call.isStatic() ? null : YogReflect.toJava(args.get(0), owner);
                Object[] in = new Object[type.parameterCount()];
                for (int i = 0; i < in.length; i++) in[i] = YogReflect.toJava(args.get(first + i), type.parameterType(i));
                return YogReflect.toData(m.invoke(receiver, in));
            } catch (ReflectiveOperationException | RuntimeException e) {
                dev.yog.YogLog.warn("nanite: the original call failed (" + e + ")");
                return null;
            }
        };
    }

    /** A redirected field's read or write as a function of its operands (the receiver first, unless static). */
    private static Function<List<Object>, Object> field(YogNaniteGen.Call call) {
        return args -> {
            try {
                Class<?> owner = Class.forName(call.owner().replace('/', '.'), false, YogInjections.class.getClassLoader());
                java.lang.reflect.Field f = fieldOf(owner, call.name());
                f.setAccessible(true);
                int first = call.isStatic() ? 0 : 1;
                Object receiver = call.isStatic() ? null : YogReflect.toJava(args.get(0), owner);
                if (!call.write()) return YogReflect.toData(f.get(receiver));
                f.set(receiver, YogReflect.toJava(args.get(first), f.getType()));
                return null;
            } catch (ReflectiveOperationException | RuntimeException e) {
                dev.yog.YogLog.warn("nanite: the original field access failed (" + e + ")");
                return null;
            }
        };
    }

    private static java.lang.reflect.Field fieldOf(Class<?> owner, String name) throws NoSuchFieldException {
        for (Class<?> at = owner; at != null; at = at.getSuperclass()) {
            try {
                return at.getDeclaredField(name);
            } catch (NoSuchFieldException e) {
                // Declared further up.
            }
        }
        throw new NoSuchFieldException(owner.getName() + "." + name);
    }

    private static Method method(Class<?> owner, String name, Class<?>[] params) throws NoSuchMethodException {
        for (Class<?> at = owner; at != null; at = at.getSuperclass()) {
            try {
                return at.getDeclaredMethod(name, params);
            } catch (NoSuchMethodException e) {
                // Declared further up.
            }
        }
        return owner.getMethod(name, params);
    }

    /** Sets {@code r}'s value (erased: the handler's own type checks it at run time). */
    private static void returning(CallbackInfoReturnable<?> r, Object value) {
        ((CallbackInfoReturnable) r).setReturnValue(value);
    }

    /** The class a value of type {@code t} is converted to. */
    private static Class<?> boxed(Type t) {
        return switch (t.getSort()) {
            case Type.BOOLEAN -> Boolean.class;
            case Type.BYTE -> Byte.class;
            case Type.CHAR -> Character.class;
            case Type.SHORT -> Short.class;
            case Type.INT -> Integer.class;
            case Type.LONG -> Long.class;
            case Type.FLOAT -> Float.class;
            case Type.DOUBLE -> Double.class;
            default -> Object.class;
        };
    }
}
