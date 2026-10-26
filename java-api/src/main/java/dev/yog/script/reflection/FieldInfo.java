package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.BiConsumer;
import java.util.function.Function;

/** A field: Yog Script's {@code System.Reflection.FieldInfo}, read and written by what the compiler wrote beside it. */
public class FieldInfo extends Member {
    private final Type type;
    private final Function<Object, Object> get;
    private final BiConsumer<Object, Object> set;

    FieldInfo(Member.Parts parts, Type type, Function<Object, Object> get, BiConsumer<Object, Object> set) {
        super(parts);
        this.type = type;
        this.get = get;
        this.set = set;
    }

    @Override
    public MemberTypes MemberType() {
        return MemberTypes.Field;
    }

    public Type FieldType() {
        return type;
    }

    public boolean IsInitOnly() {
        return (memberTraits() & INIT_ONLY) != 0;
    }

    public boolean IsLiteral() {
        return (memberTraits() & LITERAL) != 0;
    }

    /** Its value in {@code obj} ({@code null} for a static one). */
    public Object GetValue(Object obj) {
        return get.apply(obj);
    }

    public void SetValue(Object obj, Object value) {
        if (set == null) {
            throw new IllegalStateException("the field `" + Name() + "` is set only where it's made");
        }
        set.accept(obj, value);
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        return new FieldInfo(partsOf(declaring), type.substitute(arguments), get, set);
    }
}
