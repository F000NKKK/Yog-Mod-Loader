package dev.yog.nanite;

import dev.yog.YogNames;
import java.util.ArrayList;
import java.util.List;
import org.objectweb.asm.AnnotationVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;

/**
 * The locals an inject asks for (MixinExtras' {@code @Local}): a handler
 * parameter each, the local itself — or, to write it, its
 * {@code Local…Ref} — read into what the handlers see and written back
 * from what they made of it.
 */
final class YogNaniteLocals {
    private YogNaniteLocals() {}

    private static final String LOCAL = "Lcom/llamalad7/mixinextras/sugar/Local;";
    private static final String REFS = "com/llamalad7/mixinextras/sugar/ref/";

    /** A local asked for: its type (run-time names), whether it's written, its ordinal (-1: the only one of its type). */
    record Local(Type type, boolean ref, int ordinal) {
        /** The handler parameter it is: the local, or its ref. */
        Type param() {
            return ref ? Type.getObjectType(refClass(type)) : type;
        }
    }

    /** The locals {@code specs} name ({@code [&]desc[@ordinal]}, Mojang's names). */
    static List<Local> of(List<String> specs) {
        List<Local> out = new ArrayList<>();
        for (String spec : specs) {
            boolean ref = spec.startsWith(YogNames.NaniteLocal.REF);
            String rest = ref ? spec.substring(YogNames.NaniteLocal.REF.length()) : spec;
            int at = rest.indexOf(YogNames.NaniteLocal.ORDINAL);
            String desc = at < 0 ? rest : rest.substring(0, at);
            int ordinal = at < 0 ? -1 : Integer.parseInt(rest.substring(at + 1));
            out.add(new Local(Type.getType(YogMappings.desc(desc)), ref, ordinal));
        }
        return out;
    }

    /** The {@code Local…Ref} a local of {@code t} is written through. */
    private static String refClass(Type t) {
        return REFS + switch (t.getSort()) {
            case Type.BOOLEAN -> "LocalBooleanRef";
            case Type.BYTE -> "LocalByteRef";
            case Type.CHAR -> "LocalCharRef";
            case Type.SHORT -> "LocalShortRef";
            case Type.INT -> "LocalIntRef";
            case Type.LONG -> "LocalLongRef";
            case Type.FLOAT -> "LocalFloatRef";
            case Type.DOUBLE -> "LocalDoubleRef";
            default -> "LocalRef";
        };
    }

    /** What a ref's get / set read and take: its local's type, an {@code Object} for a reference. */
    private static Type refValue(Type t) {
        return t.getSort() == Type.OBJECT || t.getSort() == Type.ARRAY ? Type.getType(Object.class) : t;
    }

    /** {@code @Local(ordinal)} on handler parameter {@code index}. */
    static void annotate(MethodVisitor mv, int index, Local l) {
        AnnotationVisitor a = mv.visitParameterAnnotation(index, LOCAL, false);
        if (l.ordinal() >= 0) a.visit("ordinal", l.ordinal());
        a.visitEnd();
    }

    /** Pushes the local at {@code slot}'s value (a ref's current one), boxed. */
    static void push(MethodVisitor mv, Local l, int slot) {
        mv.visitVarInsn(l.param().getOpcode(Opcodes.ILOAD), slot);
        if (l.ref()) {
            Type v = refValue(l.type());
            mv.visitMethodInsn(Opcodes.INVOKEINTERFACE, refClass(l.type()), "get", "()" + v.getDescriptor(), true);
            YogNaniteBodies.box(mv, v);
        } else {
            YogNaniteBodies.box(mv, l.type());
        }
    }

    /** Sets the ref at {@code slot} to element {@code i} of the {@code Object[]} at {@code array} (its local's type). */
    static void writeBack(MethodVisitor mv, Local l, int slot, int array, int i) {
        Type v = refValue(l.type());
        mv.visitVarInsn(Opcodes.ALOAD, slot);
        mv.visitVarInsn(Opcodes.ALOAD, array);
        mv.visitLdcInsn(i);
        mv.visitInsn(Opcodes.AALOAD);
        YogNaniteBodies.unbox(mv, v);
        mv.visitMethodInsn(Opcodes.INVOKEINTERFACE, refClass(l.type()), "set", "(" + v.getDescriptor() + ")V", true);
    }
}
