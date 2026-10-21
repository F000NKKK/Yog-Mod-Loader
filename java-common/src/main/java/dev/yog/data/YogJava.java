package dev.yog.data;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

/**
 * Yog mods calling into Java mods' APIs (the {@code jvm/call} action): a chain
 * from a class, by reflection —
 * {@code virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()}.
 * The longest dotted prefix that is a class starts it; then each step is a
 * field, or a method called with {@code $n} (the {@code n}-th argument) or
 * nothing between its parentheses. Arguments and the result are Yog Data:
 * numbers, strings, booleans, null, and entities as {@code @entity:<uuid>}
 * references (the host says how to find one and what an entity's uuid is).
 *
 * <p>For other mods' APIs: their names are what they're compiled with.
 * Minecraft's own classes are renamed at run time on some loaders, so they
 * aren't for this.
 */
public final class YogJava {
    private YogJava() {}

    /** The entity of a uuid, or null — set by the host. */
    public static volatile Function<String, Object> entityByUuid = uuid -> null;
    /** An entity's uuid, or null for anything that isn't one — set by the host. */
    public static volatile Function<Object, String> uuidOf = o -> null;

    /** The call {@code expr} with {@code args}; its result as Yog Data (null for none). */
    public static Object invoke(String expr, List<Object> args) throws Exception {
        Object[] values = new Object[args.size()];
        for (int i = 0; i < values.length; i++) values[i] = toJava(args.get(i));
        return toData(eval(expr.trim(), values));
    }

    private static Object toJava(Object v) {
        if (v instanceof YogData.Ref r && r.kind().equals(dev.yog.YogNames.Kind.ENTITY)) {
            Object e = entityByUuid.apply(r.id());
            if (e == null) throw new IllegalArgumentException("no entity " + r.id());
            return e;
        }
        return v;
    }

    private static Object toData(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean) return v;
        if (v instanceof Float || v instanceof Double) return ((Number) v).doubleValue();
        if (v instanceof Number n) return n.longValue();
        String uuid = uuidOf.apply(v);
        if (uuid != null) return new YogData.Ref(dev.yog.YogNames.Kind.ENTITY, uuid);
        return String.valueOf(v);
    }

    /** One step of a chain: a name, and the indices of its arguments if it's a call. */
    private record Step(String name, int[] args) {}

    static Object eval(String expr, Object[] values) throws Exception {
        List<String> parts = split(expr);
        // The class: the longest dotted prefix that loads.
        Class<?> cls = null;
        int at = 0;
        StringBuilder name = new StringBuilder();
        for (int i = 0; i < parts.size(); i++) {
            String p = parts.get(i);
            if (p.contains("(")) break;
            if (i > 0) name.append('.');
            name.append(p);
            try {
                cls = Class.forName(name.toString());
                at = i + 1;
            } catch (ClassNotFoundException ignored) {
                // Maybe a nested class: Outer$Inner.
                if (cls != null) {
                    try {
                        cls = Class.forName(cls.getName() + "$" + p);
                        at = i + 1;
                    } catch (ClassNotFoundException ignored2) {
                        // not a class
                    }
                }
            }
        }
        if (cls == null) throw new ClassNotFoundException("no class at the start of " + expr);
        Object target = null;
        Class<?> type = cls;
        for (int i = at; i < parts.size(); i++) {
            Step step = step(parts.get(i));
            if (step.args() == null) {
                Field f = type.getField(step.name());
                target = f.get(Modifier.isStatic(f.getModifiers()) ? null : target);
            } else {
                Object[] a = new Object[step.args().length];
                for (int k = 0; k < a.length; k++) a[k] = values[step.args()[k]];
                Method m = method(type, step.name(), a, target == null);
                target = m.invoke(Modifier.isStatic(m.getModifiers()) ? null : target, coerce(m.getParameterTypes(), a));
            }
            if (target == null && i < parts.size() - 1) throw new NullPointerException(step.name() + " gave null");
            type = target == null ? Object.class : target.getClass();
        }
        return target;
    }

    /** {@code a.b(c).d} at top-level dots. */
    private static List<String> split(String expr) {
        List<String> out = new ArrayList<>();
        int depth = 0, start = 0;
        for (int i = 0; i < expr.length(); i++) {
            char c = expr.charAt(i);
            if (c == '(') depth++;
            else if (c == ')') depth--;
            else if (c == '.' && depth == 0) {
                out.add(expr.substring(start, i).trim());
                start = i + 1;
            }
        }
        out.add(expr.substring(start).trim());
        return out;
    }

    private static Step step(String part) {
        int open = part.indexOf('(');
        if (open < 0) return new Step(part, null);
        String inside = part.substring(open + 1, part.lastIndexOf(')')).trim();
        if (inside.isEmpty()) return new Step(part.substring(0, open), new int[0]);
        String[] items = inside.split(",");
        int[] idx = new int[items.length];
        for (int i = 0; i < items.length; i++) {
            String s = items[i].trim();
            if (!s.startsWith("$")) throw new IllegalArgumentException("argument `" + s + "`: write $0, $1, …");
            idx[i] = Integer.parseInt(s.substring(1));
        }
        return new Step(part.substring(0, open), idx);
    }

    /** A public method of {@code type} named {@code name} that takes {@code args}. */
    private static Method method(Class<?> type, String name, Object[] args, boolean wantStatic) throws NoSuchMethodException {
        Method best = null;
        int bestScore = -1;
        for (Method m : type.getMethods()) {
            if (!m.getName().equals(name) || m.getParameterCount() != args.length) continue;
            if (wantStatic && !Modifier.isStatic(m.getModifiers())) continue;
            Class<?>[] ps = m.getParameterTypes();
            int score = 0;
            boolean fits = true;
            for (int i = 0; i < ps.length && fits; i++) {
                fits = fits(ps[i], args[i]);
                score += closeness(ps[i], args[i]);
            }
            if (fits && score > bestScore) {
                best = m;
                bestScore = score;
            }
        }
        if (best == null) throw new NoSuchMethodException(type.getName() + "." + name + " taking " + args.length + " argument(s)");
        return best;
    }

    /** How well a parameter type takes an argument: whole numbers best as long/int, fractions as double/float. */
    private static int closeness(Class<?> p, Object a) {
        boolean whole = a instanceof Long || a instanceof Integer;
        boolean fraction = a instanceof Double || a instanceof Float;
        if (whole) return p == long.class || p == Long.class ? 3 : p == int.class || p == Integer.class ? 2 : 1;
        if (fraction) return p == double.class || p == Double.class ? 3 : p == float.class || p == Float.class ? 2 : 1;
        return 1;
    }

    private static boolean fits(Class<?> p, Object a) {
        if (a == null) return !p.isPrimitive();
        if (a instanceof Number) {
            return p == int.class || p == long.class || p == float.class || p == double.class || p == short.class
                    || p == byte.class || Number.class.isAssignableFrom(p) || p == Object.class;
        }
        if (a instanceof Boolean) return p == boolean.class || p == Boolean.class || p == Object.class;
        return p.isInstance(a);
    }

    private static Object[] coerce(Class<?>[] ps, Object[] args) {
        Object[] out = new Object[args.length];
        for (int i = 0; i < args.length; i++) {
            Object a = args[i];
            Class<?> p = ps[i];
            if (a instanceof Number n) {
                if (p == int.class || p == Integer.class) a = n.intValue();
                else if (p == long.class || p == Long.class) a = n.longValue();
                else if (p == float.class || p == Float.class) a = n.floatValue();
                else if (p == double.class || p == Double.class) a = n.doubleValue();
                else if (p == short.class || p == Short.class) a = n.shortValue();
                else if (p == byte.class || p == Byte.class) a = n.byteValue();
            }
            out[i] = a;
        }
        return out;
    }
}
