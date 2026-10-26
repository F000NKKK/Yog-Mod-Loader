package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.Function;

/** A constructor: Yog Script's {@code System.Reflection.ConstructorInfo}. */
public class ConstructorInfo extends MethodBase {
    /** A constructor's name, as the CLR's. */
    public static final String NAME = ".ctor";

    private final Function<Object[], Object> make;

    ConstructorInfo(Member.Parts parts, Function<Object[], Object> make) {
        super(parts);
        this.make = make;
    }

    @Override
    public MemberTypes MemberType() {
        return MemberTypes.Constructor;
    }

    /** A new object made with {@code parameters}. */
    public Object Invoke(Object[] parameters) {
        return make.apply(counted(parameters));
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        ConstructorInfo made = new ConstructorInfo(partsOf(declaring), make);
        parametersInto(made, arguments);
        return made;
    }
}
