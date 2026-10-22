package dev.yog.nanite;

import dev.yog.YogBus;
import dev.yog.YogNames;
import dev.yog.bus.YogEvents;
import java.util.ArrayList;
import java.util.List;
import org.objectweb.asm.Type;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Where every generated injection lands ({@link YogNaniteGen}): the point
 * reached is emitted on the bus ({@code yog:event/nanite/inject}, keyed by
 * the point) for the mods' handlers, and what they made of it applied —
 * cancelled, a return value set. Nothing is packed when no one listens.
 */
public final class YogInjections {
    private YogInjections() {}

    /** Each generated injection's point and its method's return type, by number. */
    private static volatile String[] points = new String[0];
    private static volatile Type[] returns = new Type[0];
    private static volatile int[] addresses = new int[0];

    /** The points the generated injections are numbered by. */
    static void points(List<String> keys, List<Type> returnTypes) {
        points = keys.toArray(String[]::new);
        returns = returnTypes.toArray(Type[]::new);
        addresses = new int[keys.size()];
    }

    private static int address(int n) {
        int at = addresses[n];
        if (at == 0) {
            at = YogBus.keyed(YogNames.EventNanite.INJECT, points[n]);
            addresses[n] = at;
        }
        return at;
    }

    /** Injection {@code n} reached in {@code self} (null: static) with {@code args}. */
    public static void fire(int n, Object self, Object[] args, CallbackInfo ci) {
        if (n < 0 || n >= points.length || !YogBus.heard(address(n))) return;
        YogEvents.Nanite.Injected out = YogObjects.borrowing(() -> {
            List<Object> values = new ArrayList<>(args.length);
            for (Object a : args) values.add(YogReflect.toData(a));
            Object value = ci instanceof CallbackInfoReturnable<?> r ? YogReflect.toData(r.getReturnValue()) : null;
            return new YogEvents.Nanite.Injected(points[n], YogReflect.toData(self), values, value, false, false)
                    .emitChanged();
        });
        if (out == null || !out.cancelled() || !ci.isCancellable()) return;
        if (out.returned() && ci instanceof CallbackInfoReturnable<?> r) {
            returning(r, YogReflect.toJava(out.value(), boxed(returns[n])));
        } else {
            ci.cancel();
        }
    }

    /** Sets {@code r}'s value (erased: the handler's own type checks it at run time). */
    private static void returning(CallbackInfoReturnable<?> r, Object value) {
        ((CallbackInfoReturnable) r).setReturnValue(value);
    }

    /** The class a return value of type {@code t} is converted to. */
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
