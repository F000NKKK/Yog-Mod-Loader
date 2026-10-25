package dev.yog.nanite;

import dev.yog.YogNames;
import dev.yog.data.YogData;
import dev.yog.data.YogJava;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Reflection for Yog mods ({@code yog:nanite/…}, {@code docs/design/nanites.md}):
 * a member named by Mojang's names resolved once to a handle (the run-time
 * name looked up in {@link YogMappings}, a {@link MethodHandle} or
 * {@link Field} made accessible), then called, read or written by it.
 * Values cross as Yog Data: numbers, text, booleans, lists, entities
 * ({@code @entity:<uuid>}) and held objects ({@link YogObjects}).
 */
public final class YogReflect {
    private YogReflect() {}

    /**
     * What a resolved member is: a method (its handle, and the same taking
     * its arguments as one {@code Object[]} and giving an {@code Object},
     * called exactly — no array of arguments made by the JVM a call) or a field.
     */
    private record Member(String key, MethodHandle method, MethodHandle spread, Field field) {
        static Member of(String key, MethodHandle mh) {
            int n = mh.type().parameterCount();
            MethodHandle spread = mh.asType(MethodType.genericMethodType(n)).asSpreader(Object[].class, n);
            return new Member(key, mh, spread, null);
        }
    }

    private static final List<Member> MEMBERS = new ArrayList<>();
    private static final Map<String, Integer> RESOLVED = new ConcurrentHashMap<>();
    /** What a member's key joins its parts with. */
    private static final char SEP = ' ';
    /** A constructor's name and what starts a method's descriptor. */
    private static final String CONSTRUCTOR = "<init>";
    private static final char METHOD = '(';
    /** What a handle answers when there's no such member. */
    private static final int NONE = Integer.MIN_VALUE;

    /** Member {@code name desc} of class {@code cls} → its handle; {@link #NONE} if there's none. */
    public static synchronized int resolve(String cls, String name, String desc) {
        String key = cls + SEP + name + SEP + desc;
        Integer known = RESOLVED.get(key);
        if (known != null) return known;
        try {
            Class<?> c = YogMappings.load(cls.replace('.', '/'));
            Member m = desc.indexOf(METHOD) == 0 ? method(key, c, name, desc) : new Member(key, null, null, field(c, name));
            MEMBERS.add(m);
            int id = MEMBERS.size() - 1;
            RESOLVED.put(key, id);
            return id;
        } catch (ReflectiveOperationException | RuntimeException e) {
            dev.yog.YogLog.warn("nanite: no " + key + " (" + e + ")");
            return NONE;
        }
    }

    private static Member method(String key, Class<?> c, String name, String desc) throws ReflectiveOperationException {
        MethodType type = MethodType.fromMethodDescriptorString(YogMappings.desc(desc), c.getClassLoader());
        MethodHandles.Lookup lookup = MethodHandles.lookup();
        if (name.equals(CONSTRUCTOR)) {
            Constructor<?> k = c.getDeclaredConstructor(type.parameterArray());
            k.setAccessible(true);
            return Member.of(key, lookup.unreflectConstructor(k));
        }
        String runtime = YogMappings.method(c, name, desc);
        for (Class<?> at = c; at != null; at = at.getSuperclass()) {
            for (Method m : at.getDeclaredMethods()) {
                if (m.getName().equals(runtime) && Arrays.equals(m.getParameterTypes(), type.parameterArray())) {
                    m.setAccessible(true);
                    return Member.of(key, lookup.unreflect(m));
                }
            }
        }
        Method m = c.getMethod(runtime, type.parameterArray());
        return Member.of(key, lookup.unreflect(m));
    }

    private static Field field(Class<?> c, String name) throws NoSuchFieldException {
        String runtime = YogMappings.field(c, name);
        for (Class<?> at = c; at != null; at = at.getSuperclass()) {
            for (Field f : at.getDeclaredFields()) {
                if (f.getName().equals(runtime)) {
                    f.setAccessible(true);
                    return f;
                }
            }
        }
        throw new NoSuchFieldException(c.getName() + "." + runtime);
    }

    private static Member member(long id) {
        if (id < 0 || id >= MEMBERS.size()) throw new IllegalArgumentException("no member " + id);
        return MEMBERS.get((int) id);
    }

    /** Method or constructor {@code id} called on {@code target} with {@code args}. */
    public static Object call(long id, Object target, List<Object> args) throws Throwable {
        Member m = member(id);
        if (m.method() == null) throw new IllegalArgumentException("member " + id + " is a field");
        MethodType type = m.method().type();
        Object[] in = new Object[type.parameterCount()];
        int from = in.length > args.size() ? 1 : 0;
        if (from == 1) in[0] = notGone(m, target, toJava(target, type.parameterType(0)));
        for (int i = 0; i < args.size() && from + i < in.length; i++) {
            in[from + i] = toJava(args.get(i), type.parameterType(from + i));
        }
        return toData((Object) m.spread().invokeExact(in));
    }

    /**
     * What a member is reached on, {@code target} resolved: one that's gone (let
     * go, or never given) is said by name rather than failing as a bare null.
     */
    private static Object notGone(Member m, Object target, Object on) {
        if (on == null) throw new IllegalStateException("nanite " + m.key() + ": on " + target + ", which isn't held (let go, or never given)");
        return on;
    }

    /** Field {@code id} of {@code target}. */
    public static Object get(long id, Object target) throws IllegalAccessException {
        Field f = field(id);
        return toData(f.get(Modifier.isStatic(f.getModifiers()) ? null : notGone(member(id), target, toJava(target, f.getDeclaringClass()))));
    }

    /** Field {@code id} of {@code target} set to {@code value}. */
    public static boolean set(long id, Object target, Object value) throws IllegalAccessException {
        Field f = field(id);
        Object on = Modifier.isStatic(f.getModifiers()) ? null : notGone(member(id), target, toJava(target, f.getDeclaringClass()));
        f.set(on, toJava(value, f.getType()));
        return true;
    }

    private static Field field(long id) {
        Field f = member(id).field();
        if (f == null) throw new IllegalArgumentException("member " + id + " is a method");
        return f;
    }

    /** Lets the held objects go. */
    public static boolean release(long[] objects) {
        for (long n : objects) YogObjects.release(n);
        return true;
    }

    /** A value from the bus as type {@code want}. */
    static Object toJava(Object v, Class<?> want) {
        if (v instanceof YogData.Ref r) {
            Object held = YogObjects.of(r);
            if (held != null) return held;
            if (r.kind().equals(YogNames.Kind.ENTITY)) return YogJava.entityByUuid.apply(r.id());
            return null;
        }
        if (v instanceof Number n) return number(n, want);
        return v;
    }

    private static Object number(Number n, Class<?> want) {
        if (want == int.class || want == Integer.class) return n.intValue();
        if (want == long.class || want == Long.class) return n.longValue();
        if (want == float.class || want == Float.class) return n.floatValue();
        if (want == double.class || want == Double.class) return n.doubleValue();
        if (want == short.class || want == Short.class) return n.shortValue();
        if (want == byte.class || want == Byte.class) return n.byteValue();
        if (want == char.class || want == Character.class) return (char) n.intValue();
        return n;
    }

    /** A value as the bus carries it: an object it can't as a held reference. */
    static Object toData(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean) return v;
        if (v instanceof Float || v instanceof Double) return ((Number) v).doubleValue();
        if (v instanceof Number n) return n.longValue();
        if (v instanceof Character c) return String.valueOf(c);
        if (v instanceof List<?> l) {
            List<Object> out = new ArrayList<>(l.size());
            for (Object o : l) out.add(toData(o));
            return out;
        }
        String uuid = YogJava.uuidOf.apply(v);
        if (uuid != null) return new YogData.Ref(YogNames.Kind.ENTITY, uuid);
        return YogObjects.hold(v);
    }
}
