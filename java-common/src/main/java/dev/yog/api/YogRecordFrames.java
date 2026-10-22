package dev.yog.api;

import dev.yog.data.YogFrame;
import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.util.List;
import java.util.Optional;

/**
 * A Java record as a bus frame — the same bytes a Rust struct with the same
 * fields in the same order packs to ({@code yog-event}'s {@code java/frames.rs}
 * writes the built-in messages' code; this reads any record by reflection):
 * numbers, {@code boolean}, {@code String}, {@code byte[]}, number arrays,
 * {@code List}, {@code Optional}, enums (by their order), nested records;
 * anything else as a tagged Yog Data value.
 */
final class YogRecordFrames {
    private YogRecordFrames() {}

    /** A record type's components, how to read each and how to make one. */
    private record Layout(Method[] getters, Type[] types, Constructor<?> make) {}

    private static final ClassValue<Layout> LAYOUTS = new ClassValue<>() {
        @Override
        protected Layout computeValue(Class<?> type) {
            RecordComponent[] parts = type.getRecordComponents();
            Method[] getters = new Method[parts.length];
            Type[] types = new Type[parts.length];
            Class<?>[] raw = new Class<?>[parts.length];
            for (int i = 0; i < parts.length; i++) {
                getters[i] = parts[i].getAccessor();
                getters[i].setAccessible(true);
                types[i] = parts[i].getGenericType();
                raw[i] = parts[i].getType();
            }
            try {
                Constructor<?> make = type.getDeclaredConstructor(raw);
                make.setAccessible(true);
                return new Layout(getters, types, make);
            } catch (NoSuchMethodException e) {
                throw new IllegalStateException(type.getName() + " has no canonical constructor", e);
            }
        }
    };

    static void pack(YogFrame.Packer p, Object value, Type type) {
        Class<?> raw = raw(type);
        if (raw == boolean.class || raw == Boolean.class) p.putBool((Boolean) value);
        else if (integral(raw)) p.putI(((Number) value).longValue());
        else if (raw == float.class || raw == Float.class) p.putF32((Float) value);
        else if (raw == double.class || raw == Double.class) p.putF64((Double) value);
        else if (raw == String.class) p.putStr((String) value);
        else if (raw == byte[].class) p.putBytes((byte[]) value);
        else if (raw == int[].class) p.putInts((int[]) value, true);
        else if (raw == long[].class) p.putLongs((long[]) value, true);
        else if (raw == float[].class) p.putF32s((float[]) value);
        else if (raw == double[].class) p.putF64s((double[]) value);
        else if (raw == Optional.class) packOptional(p, (Optional<?>) value, argument(type));
        else if (List.class.isAssignableFrom(raw)) packList(p, (List<?>) value, argument(type));
        else if (raw.isRecord()) packRecord(p, value);
        else if (raw.isEnum()) p.putU(((Enum<?>) value).ordinal());
        else p.putTagged(value);
    }

    private static void packOptional(YogFrame.Packer p, Optional<?> value, Type item) {
        boolean present = value != null && value.isPresent();
        p.putBool(present);
        if (present) pack(p, value.get(), item);
    }

    private static void packList(YogFrame.Packer p, List<?> list, Type item) {
        int at = p.beginList(list.size());
        for (Object e : list) pack(p, e, item);
        p.end(at);
    }

    static void packRecord(YogFrame.Packer p, Object record) {
        Layout l = LAYOUTS.get(record.getClass());
        int at = p.begin();
        for (int i = 0; i < l.getters.length; i++) pack(p, get(l.getters[i], record), l.types[i]);
        p.end(at);
    }

    static Object unpack(YogFrame.Unpacker u, Type type) {
        Class<?> raw = raw(type);
        if (raw == boolean.class || raw == Boolean.class) return u.getBool();
        if (raw == int.class || raw == Integer.class) return (int) u.getI();
        if (raw == long.class || raw == Long.class) return u.getI();
        if (raw == short.class || raw == Short.class) return (short) u.getI();
        if (raw == byte.class || raw == Byte.class) return (byte) u.getI();
        if (raw == float.class || raw == Float.class) return u.getF32();
        if (raw == double.class || raw == Double.class) return u.getF64();
        if (raw == String.class) return u.getStr();
        if (raw == byte[].class) return u.getBytes();
        if (raw == int[].class) return u.getInts(true);
        if (raw == long[].class) return u.getLongs(true);
        if (raw == float[].class) return u.getF32s();
        if (raw == double[].class) return u.getF64s();
        if (raw == Optional.class) return u.getBool() ? Optional.of(unpack(u, argument(type))) : Optional.empty();
        if (List.class.isAssignableFrom(raw)) return YogFrame.list(u, () -> unpack(u, argument(type)));
        if (raw.isRecord()) return unpackRecord(u, raw);
        if (raw.isEnum()) return raw.getEnumConstants()[(int) u.getU()];
        return u.getTagged();
    }

    static Object unpackRecord(YogFrame.Unpacker u, Class<?> type) {
        Layout l = LAYOUTS.get(type);
        Object[] values = new Object[l.types.length];
        int end = u.begin();
        for (int i = 0; i < values.length; i++) values[i] = u.more(end) ? unpack(u, l.types[i]) : missing(l.types[i]);
        u.leave(end);
        try {
            return l.make.newInstance(values);
        } catch (InstantiationException | IllegalAccessException | InvocationTargetException e) {
            throw new IllegalStateException("can't make a " + type.getName(), e);
        }
    }

    /** A field a shorter frame (an older sender) leaves out: its type's empty value. */
    private static Object missing(Type type) {
        Class<?> raw = raw(type);
        if (raw == boolean.class) return false;
        if (raw == float.class) return 0f;
        if (raw == double.class) return 0d;
        if (raw == long.class) return 0L;
        if (raw == short.class) return (short) 0;
        if (raw == byte.class) return (byte) 0;
        if (raw.isPrimitive()) return 0;
        if (raw == String.class) return "";
        if (raw == Optional.class) return Optional.empty();
        if (List.class.isAssignableFrom(raw)) return List.of();
        if (raw.isArray()) return Array.newInstance(raw.getComponentType(), 0);
        return null;
    }

    private static Object get(Method getter, Object record) {
        try {
            return getter.invoke(record);
        } catch (IllegalAccessException | InvocationTargetException e) {
            throw new IllegalStateException("can't read " + getter, e);
        }
    }

    private static boolean integral(Class<?> raw) {
        return raw == int.class || raw == Integer.class || raw == long.class || raw == Long.class
                || raw == short.class || raw == Short.class || raw == byte.class || raw == Byte.class;
    }

    private static Class<?> raw(Type type) {
        if (type instanceof Class<?> c) return c;
        if (type instanceof ParameterizedType p) return (Class<?>) p.getRawType();
        return Object.class;
    }

    /** {@code List<T>}'s / {@code Optional<T>}'s {@code T}. */
    private static Type argument(Type type) {
        return type instanceof ParameterizedType p ? p.getActualTypeArguments()[0] : Object.class;
    }
}
