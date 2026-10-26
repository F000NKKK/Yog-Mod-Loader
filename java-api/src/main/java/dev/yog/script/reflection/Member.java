package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.List;
import java.util.function.Supplier;

/** What every member but a type has: its name, whose it is, what it is. */
abstract class Member extends MemberInfo {
    /** A member's name, traits ({@link #STATIC}…), declaring type and attributes' makers. */
    record Parts(String name, int traits, Type declaring, List<Supplier<Object>> attributes) {}

    private final String name;
    private final int traits;
    private final Type declaring;
    /** Its attributes' makers; a constructed type's member shares its definition's. */
    final List<Supplier<Object>> attributes;

    Member(Parts parts) {
        this.attributes = parts.attributes();
        this.name = parts.name();
        this.traits = parts.traits();
        this.declaring = parts.declaring();
    }

    /** Its parts, of {@code declaring}. */
    Parts partsOf(Type declaring) {
        return new Parts(name, traits, declaring, attributes);
    }

    @Override
    public String Name() {
        return name;
    }

    @Override
    public Type DeclaringType() {
        return declaring;
    }

    @Override
    protected List<Supplier<Object>> attributeMakers() {
        return attributes;
    }

    @Override
    protected int memberTraits() {
        return traits;
    }

    /** The same member of {@code declaring}, its types as {@code arguments} say (a constructed type's). */
    abstract Member of(Type declaring, Type[] arguments);

    public boolean IsStatic() {
        return (traits & STATIC) != 0;
    }

    public boolean IsPublic() {
        return isPublic();
    }

    public boolean IsPrivate() {
        return (traits & PRIVATE) != 0;
    }

    @Override
    public String toString() {
        return Name();
    }
}
