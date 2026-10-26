package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.BiConsumer;
import java.util.function.Function;

/** A property: Yog Script's {@code System.Reflection.PropertyInfo}, read and written through its accessors. */
public class PropertyInfo extends Member {
    private final Type type;
    private final Function<Object, Object> get;
    private final BiConsumer<Object, Object> set;

    PropertyInfo(Member.Parts parts, Type type, Function<Object, Object> get, BiConsumer<Object, Object> set) {
        super(parts);
        this.type = type;
        this.get = get;
        this.set = set;
    }

    @Override
    public MemberTypes MemberType() {
        return MemberTypes.Property;
    }

    public Type PropertyType() {
        return type;
    }

    public boolean CanRead() {
        return get != null;
    }

    public boolean CanWrite() {
        return set != null;
    }

    public Object GetValue(Object obj) {
        if (get == null) {
            throw new IllegalStateException("the property `" + Name() + "` has no getter");
        }
        return get.apply(obj);
    }

    public void SetValue(Object obj, Object value) {
        if (set == null) {
            throw new IllegalStateException("the property `" + Name() + "` has no setter");
        }
        set.accept(obj, value);
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        return new PropertyInfo(partsOf(declaring), type.substitute(arguments), get, set);
    }
}
