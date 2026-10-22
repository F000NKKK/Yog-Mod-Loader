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
    private static volatile int[] addresses = new int[0];

    /** What the generated nanites are numbered by. */
    static void points(List<String> keys, List<YogNaniteGen.Made> made) {
        points = keys.toArray(String[]::new);
        values = made.stream().map(YogNaniteGen.Made::value).toArray(Type[]::new);
        calls = made.stream().map(YogNaniteGen.Made::call).toArray(YogNaniteGen.Call[]::new);
        addresses = new int[keys.size()];
    }

    private static boolean heard(int n) {
        if (n < 0 || n >= points.length) return false;
        int at = addresses[n];
        if (at == 0) {
            at = YogBus.keyed(YogNames.EventNanite.INJECT, points[n]);
            addresses[n] = at;
        }
        return YogBus.heard(at);
    }

    /** Point {@code n} emitted (its objects lent for the call): what the handlers made of it. */
    private static YogEvents.Nanite.Injected emit(int n, Object self, Object[] args, Object value, Object original) {
        return YogObjects.borrowing(() -> {
            List<Object> data = new ArrayList<>(args.length);
            for (Object a : args) data.add(YogReflect.toData(a));
            Object originalRef = original == null ? null : YogObjects.hold(original);
            return new YogEvents.Nanite.Injected(points[n], YogReflect.toData(self), data,
                    YogReflect.toData(value), false, false, originalRef).emitChanged();
        });
    }

    /** Inject {@code n} reached in {@code self} (null: static) with {@code args}. */
    public static void fire(int n, Object self, Object[] args, CallbackInfo ci) {
        if (!heard(n)) return;
        Object value = ci instanceof CallbackInfoReturnable<?> r ? r.getReturnValue() : null;
        YogEvents.Nanite.Injected out = emit(n, self, args, value, null);
        if (out == null || !out.cancelled() || !ci.isCancellable()) return;
        if (out.returned() && ci instanceof CallbackInfoReturnable<?> r) {
            returning(r, YogReflect.toJava(out.value(), boxed(values[n])));
        } else {
            ci.cancel();
        }
    }

    /** Modify {@code n}'s value {@code value}: as a handler made it, else as it was. */
    public static Object modify(int n, Object self, Object value) {
        if (!heard(n)) return value;
        YogEvents.Nanite.Injected out = emit(n, self, new Object[0], value, null);
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
