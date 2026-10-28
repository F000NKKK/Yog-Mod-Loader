package dev.yog.script.yog;

import dev.yog.YogLog;
import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.util.Collection;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.function.Function;

/**
 * Values of a mod's old generation of classes carried over into its new
 * one (a hot reload, docs/design/storage.md S8/S9), in the same process:
 * what isn't the old classes' (the JDK's, the runtime's) stays as it is,
 * the containers holding the old classes' values filled with theirs; an old
 * class's object becomes the new same-named class's, field by field; an
 * enum's case or a class's singleton the new class's same-named one; a
 * record made by its constructor. What can't be (an old lambda, a
 * container holding one) is {@link #MISSING}: the new object keeps its own.
 */
public final class Rebind {
    /** What can't be carried (an old lambda, what holds one): the new object's own value stays. */
    public static final Object MISSING = new Object();

    /** Where the old classes came from, and where their new ones are. */
    private final Set<ClassLoader> old;
    private final List<ClassLoader> next;
    /** What an old object already became (shared, cyclic). */
    private final Map<Object, Object> made = new IdentityHashMap<>();
    /** Containers already filled. */
    private final Set<Object> filled = java.util.Collections.newSetFromMap(new IdentityHashMap<>());
    /** What an old object some owner keeps becomes instead (a live object's new one). */
    private final Function<Object, Object> owned;

    public Rebind(Set<ClassLoader> old, List<ClassLoader> next, Function<Object, Object> owned) {
        this.old = old;
        this.next = next;
        this.owned = owned;
    }

    /** {@code v} as the new classes have it; {@link #MISSING} for what can't be. */
    public Object value(Object v) {
        if (v == null) {
            return null;
        }
        Object done = made.get(v);
        if (done != null) {
            return done;
        }
        if (!isOld(v.getClass())) {
            return shared(v);
        }
        Object own = owned.apply(v);
        if (own == MISSING) {
            return MISSING;
        }
        if (own != null) {
            made.put(v, own);
            return own;
        }
        Object r = remade(v);
        return r == null ? MISSING : r;
    }

    /** {@code into} (a new class's object) given {@code from}'s fields, by name. */
    public void fields(Object from, Object into) {
        made.put(from, into);
        for (Class<?> c = into.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            Class<?> was = sameIn(from.getClass(), c.getName());
            if (was == null) {
                continue;
            }
            for (Field f : c.getDeclaredFields()) {
                if (!Modifier.isStatic(f.getModifiers())) {
                    copy(f, field(was, f.getName()), from, into);
                }
            }
        }
    }

    /** The new class {@code fresh}'s static fields given the old class {@code was}'s that aren't constants. */
    public void statics(Class<?> was, Class<?> fresh) {
        for (Field f : fresh.getDeclaredFields()) {
            int m = f.getModifiers();
            if (Modifier.isStatic(m) && !Modifier.isFinal(m)) {
                copy(f, field(was, f.getName()), null, null);
            }
        }
    }

    /** Old class {@code name} as the new classes have it; {@code null} when it's gone. */
    public Class<?> newClass(String name) {
        for (ClassLoader l : next) {
            try {
                return Class.forName(name, false, l);
            } catch (ClassNotFoundException e) {
                // Not this jar's.
            }
        }
        return null;
    }

    private boolean isOld(Class<?> c) {
        ClassLoader l = c.getClassLoader();
        return l != null && old.contains(l); // the JDK's own (no loader) never are
    }

    private void copy(Field to, Field from, Object fromObj, Object toObj) {
        if (from == null || !trySetAccessible(from) || !trySetAccessible(to)) {
            return;
        }
        try {
            Object v = value(from.get(fromObj));
            if (v == MISSING) {
                return; // what couldn't be carried: the new one's own value stays
            }
            if (to.getType().isPrimitive() ? v != null : v == null || to.getType().isInstance(v)) {
                to.set(toObj, v);
            }
        } catch (IllegalAccessException | RuntimeException e) {
            YogLog.warn("Yog Script reload: " + to.getDeclaringClass().getName() + "." + to.getName() + " not carried: " + e.getMessage());
        }
    }

    /** An old class's object as the new same-named class's. */
    private Object remade(Object v) {
        Class<?> was = v.getClass();
        Class<?> fresh = was.isHidden() ? null : newClass(was.getName());
        if (fresh == null) {
            return null;
        }
        Object single = singleton(v, was, fresh);
        if (single != null) {
            made.put(v, single);
            return single;
        }
        if (was.isRecord()) {
            return record(v, was, fresh);
        }
        if (fresh.isEnum()) {
            return enumCase(v, fresh);
        }
        Object into = allocate(fresh);
        if (into == null) {
            return null;
        }
        fields(v, into);
        return into;
    }

    /** {@code v} kept in a static field of its class (a case, a singleton): the new class's of that name. */
    private static Object singleton(Object v, Class<?> was, Class<?> fresh) {
        for (Field f : was.getDeclaredFields()) {
            if (Modifier.isStatic(f.getModifiers()) && f.getType().isAssignableFrom(was) && trySetAccessible(f)) {
                try {
                    if (f.get(null) == v) {
                        Field same = field(fresh, f.getName());
                        return same != null && trySetAccessible(same) ? same.get(null) : null;
                    }
                } catch (IllegalAccessException e) {
                    return null;
                }
            }
        }
        return null;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Object enumCase(Object v, Class<?> fresh) {
        try {
            return Enum.valueOf((Class) fresh, ((Enum<?>) v).name());
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    private Object record(Object v, Class<?> was, Class<?> fresh) {
        RecordComponent[] parts = fresh.getRecordComponents();
        Object[] args = new Object[parts.length];
        Class<?>[] types = new Class<?>[parts.length];
        try {
            for (int i = 0; i < parts.length; i++) {
                types[i] = parts[i].getType();
                Method get = was.getMethod(parts[i].getName());
                args[i] = value(get.invoke(v));
                if (args[i] == MISSING) {
                    return null;
                }
            }
            var make = fresh.getDeclaredConstructor(types);
            make.setAccessible(true);
            Object r = make.newInstance(args);
            made.put(v, r);
            return r;
        } catch (ReflectiveOperationException | RuntimeException e) {
            YogLog.warn("Yog Script reload: a " + fresh.getName() + " not carried: " + e.getMessage());
            return null;
        }
    }

    /** Something not of the old classes: itself, what it holds of them
     * theirs now ({@link #MISSING} when some of that can't be carried: it's
     * left as it is, and the new object keeps its own). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private Object shared(Object v) {
        if (v instanceof Optional<?> o) {
            Object x = o.isPresent() ? value(o.get()) : null;
            return x == MISSING ? MISSING : Optional.ofNullable(x);
        }
        if (!filled.add(v)) {
            return v;
        }
        try {
            if (v instanceof List l) {
                List<Object> items = carried(l.toArray());
                if (items == null) return MISSING;
                for (int i = 0; i < items.size(); i++) l.set(i, items.get(i));
            } else if (v instanceof Map m) {
                Object[] keys = m.keySet().toArray();
                List<Object> ks = carried(keys);
                List<Object> vs = carried(java.util.Arrays.stream(keys).map(m::get).toArray());
                if (ks == null || vs == null) return MISSING;
                m.clear();
                for (int i = 0; i < ks.size(); i++) m.put(ks.get(i), vs.get(i));
            } else if (v instanceof Collection c) {
                List<Object> items = carried(c.toArray());
                if (items == null) return MISSING;
                c.clear();
                c.addAll(items);
            } else if (v.getClass().isArray() && !v.getClass().getComponentType().isPrimitive()) {
                Object[] a = new Object[Array.getLength(v)];
                for (int i = 0; i < a.length; i++) a[i] = Array.get(v, i);
                List<Object> items = carried(a);
                if (items == null) return MISSING;
                for (int i = 0; i < a.length; i++) Array.set(v, i, items.get(i));
            } else if (v.getClass().getName().startsWith(RUNTIME)) {
                return fieldsInPlace(v) ? v : MISSING;
            }
        } catch (UnsupportedOperationException | ArrayStoreException e) {
            return MISSING; // an immutable container of the old classes' values
        }
        return v;
    }

    /** {@code items} carried; {@code null} when one of them can't be. */
    private List<Object> carried(Object[] items) {
        List<Object> out = new java.util.ArrayList<>(items.length);
        for (Object x : items) {
            Object y = value(x);
            if (y == MISSING) {
                return null;
            }
            out.add(y);
        }
        return out;
    }

    /** The runtime's own types (a Yog Script collection): their fields walked in place. */
    private static final String RUNTIME = "dev.yog.";

    /** {@code v}'s fields (a runtime type's) carried in place; false when one can't be. */
    private boolean fieldsInPlace(Object v) {
        java.util.List<Object[]> sets = new java.util.ArrayList<>();
        for (Class<?> c = v.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers()) || f.getType().isPrimitive() || !trySetAccessible(f)) {
                    continue;
                }
                try {
                    Object x = f.get(v);
                    Object y = value(x);
                    if (y == MISSING) {
                        return false;
                    }
                    if (y != x) {
                        sets.add(new Object[] {f, y});
                    }
                } catch (IllegalAccessException | RuntimeException e) {
                    return false;
                }
            }
        }
        for (Object[] set : sets) {
            try {
                ((Field) set[0]).set(v, set[1]);
            } catch (IllegalAccessException | RuntimeException e) {
                return false;
            }
        }
        return true;
    }

    /** Class {@code name} among {@code c} and its supers. */
    private static Class<?> sameIn(Class<?> c, String name) {
        for (Class<?> k = c; k != null; k = k.getSuperclass()) {
            if (k.getName().equals(name)) {
                return k;
            }
        }
        return null;
    }

    private static Field field(Class<?> c, String name) {
        try {
            return c.getDeclaredField(name);
        } catch (NoSuchFieldException e) {
            return null;
        }
    }

    private static boolean trySetAccessible(Field f) {
        try {
            return f.trySetAccessible();
        } catch (SecurityException e) {
            return false;
        }
    }

    /** An object of {@code c} made without its constructor: its fields are about to be set. */
    private static Object allocate(Class<?> c) {
        try {
            Class<?> unsafeClass = Class.forName(UNSAFE);
            Field theUnsafe = unsafeClass.getDeclaredField(THE_UNSAFE);
            theUnsafe.setAccessible(true);
            return unsafeClass.getMethod(ALLOCATE, Class.class).invoke(theUnsafe.get(null), c);
        } catch (ReflectiveOperationException | RuntimeException e) {
            YogLog.warn("Yog Script reload: a " + c.getName() + " can't be made: " + e.getMessage());
            return null;
        }
    }

    /** What makes an object without running a constructor ({@code jdk.unsupported}). */
    private static final String UNSAFE = "sun.misc.Unsafe";
    private static final String THE_UNSAFE = "theUnsafe";
    private static final String ALLOCATE = "allocateInstance";
}
