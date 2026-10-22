package dev.yog.nanite;

import java.util.function.Consumer;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Label;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;

/**
 * The bodies of generated handlers: what's at stake packed (boxed) and
 * handed to {@link YogInjections}, what comes back unpacked — an inject's
 * callback, a modified value, a redirected call's result (or the call
 * itself, when no handler answered).
 */
final class YogNaniteBodies {
    private YogNaniteBodies() {}

    private static final String HANDLER_PREFIX = "yog$nanite$";
    private static final String CALLBACK = "org/spongepowered/asm/mixin/injection/callback/CallbackInfo";
    private static final String CALLBACK_RETURNABLE = "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable";
    private static final String OBJECT = "java/lang/Object";
    private static final String OWNER = Type.getInternalName(YogInjections.class);
    private static final String FIRE = "(ILjava/lang/Object;[Ljava/lang/Object;L" + CALLBACK + ";)V";
    private static final String MODIFY = "(ILjava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;";
    private static final String REDIRECT = "(ILjava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;";
    private static final String NOT_ANSWERED = "NOT_ANSWERED";
    private static final String CALL = "(ILjava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;";

    /** A method added to the class: its arguments to the mod's handler, its answer returned. */
    static void added(ClassWriter cw, int n, String name, Type method) {
        MethodVisitor mv = cw.visitMethod(Opcodes.ACC_PUBLIC, name, method.getDescriptor(), null, null);
        mv.visitCode();
        mv.visitLdcInsn(n);
        mv.visitVarInsn(Opcodes.ALOAD, 0);
        array(mv, method.getArgumentTypes(), 1);
        mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, "call", CALL, false);
        Type ret = method.getReturnType();
        if (ret.getSort() == Type.VOID) {
            mv.visitInsn(Opcodes.POP);
            mv.visitInsn(Opcodes.RETURN);
        } else {
            unbox(mv, ret);
            mv.visitInsn(ret.getOpcode(Opcodes.IRETURN));
        }
        end(mv);
    }

    /** An {@code @Inject} handler: the target's arguments and its callback. */
    static void inject(ClassWriter cw, int n, YogNaniteGen.Target target, Consumer<MethodVisitor> annotate) {
        Type[] params = target.method().getArgumentTypes();
        boolean returns = target.method().getReturnType().getSort() != Type.VOID;
        Type callback = Type.getObjectType(returns ? CALLBACK_RETURNABLE : CALLBACK);
        Type[] all = append(params, callback);
        MethodVisitor mv = method(cw, n, target, Type.getMethodDescriptor(Type.VOID_TYPE, all), annotate);
        int first = target.isStatic() ? 0 : 1;
        start(mv, n, target);
        array(mv, params, first);
        mv.visitVarInsn(Opcodes.ALOAD, first + size(params));
        mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, "fire", FIRE, false);
        mv.visitInsn(Opcodes.RETURN);
        end(mv);
    }

    /** A {@code @Modify…} handler: the value in, the value out. */
    static void modify(ClassWriter cw, int n, YogNaniteGen.Target target, Type value, Consumer<MethodVisitor> annotate) {
        MethodVisitor mv = method(cw, n, target, Type.getMethodDescriptor(value, value), annotate);
        start(mv, n, target);
        mv.visitVarInsn(value.getOpcode(Opcodes.ILOAD), target.isStatic() ? 0 : 1);
        box(mv, value);
        mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, "modify", MODIFY, false);
        unbox(mv, value);
        mv.visitInsn(value.getOpcode(Opcodes.IRETURN));
        end(mv);
    }

    /**
     * A {@code @Redirect} handler: the call's receiver (unless static) and
     * arguments in; what a handler answered out, else the call made as it was.
     */
    static void redirect(ClassWriter cw, int n, YogNaniteGen.Target target, YogNaniteGen.Call call, Consumer<MethodVisitor> annotate) {
        Type[] params = redirectParams(call);
        Type ret = redirectReturn(call);
        MethodVisitor mv = method(cw, n, target, Type.getMethodDescriptor(ret, params), annotate);
        int first = target.isStatic() ? 0 : 1;
        start(mv, n, target);
        array(mv, params, first);
        mv.visitMethodInsn(Opcodes.INVOKESTATIC, OWNER, "redirect", REDIRECT, false);
        mv.visitInsn(Opcodes.DUP);
        mv.visitFieldInsn(Opcodes.GETSTATIC, OWNER, NOT_ANSWERED, "L" + OBJECT + ";");
        Label answered = new Label();
        mv.visitJumpInsn(Opcodes.IF_ACMPNE, answered);
        mv.visitInsn(Opcodes.POP);
        load(mv, params, first);
        original(mv, call);
        mv.visitInsn(ret.getOpcode(Opcodes.IRETURN));
        mv.visitLabel(answered);
        if (ret.getSort() == Type.VOID) {
            mv.visitInsn(Opcodes.POP);
            mv.visitInsn(Opcodes.RETURN);
        } else {
            unbox(mv, ret);
            mv.visitInsn(ret.getOpcode(Opcodes.IRETURN));
        }
        end(mv);
    }

    /** What a redirect's handler takes: the receiver (unless static), then the call's arguments or the value written. */
    private static Type[] redirectParams(YogNaniteGen.Call call) {
        Type[] args;
        if (call.isField()) args = call.write() ? new Type[] {Type.getType(call.desc())} : new Type[0];
        else args = Type.getArgumentTypes(call.desc());
        return call.isStatic() ? args : prepend(Type.getObjectType(call.owner()), args);
    }

    /** What a redirect's handler gives: the call's result, the field's value read, nothing for a write. */
    private static Type redirectReturn(YogNaniteGen.Call call) {
        if (!call.isField()) return Type.getReturnType(call.desc());
        return call.write() ? Type.VOID_TYPE : Type.getType(call.desc());
    }

    /** The access the redirect stands in for, made as it was (its operands on the stack). */
    private static void original(MethodVisitor mv, YogNaniteGen.Call call) {
        if (call.isField()) {
            mv.visitFieldInsn(call.fieldOpcode(), call.owner(), call.name(), call.desc());
            return;
        }
        int op = call.isStatic() ? Opcodes.INVOKESTATIC : call.isInterface() ? Opcodes.INVOKEINTERFACE : Opcodes.INVOKEVIRTUAL;
        mv.visitMethodInsn(op, call.owner(), call.name(), call.desc(), call.isInterface());
    }

    private static MethodVisitor method(ClassWriter cw, int n, YogNaniteGen.Target target, String desc, Consumer<MethodVisitor> annotate) {
        int access = Opcodes.ACC_PRIVATE | (target.isStatic() ? Opcodes.ACC_STATIC : 0);
        MethodVisitor mv = cw.visitMethod(access, HANDLER_PREFIX + n, desc, null, null);
        annotate.accept(mv);
        mv.visitCode();
        return mv;
    }

    /** Pushes the point's number and the object (null in a static method). */
    private static void start(MethodVisitor mv, int n, YogNaniteGen.Target target) {
        mv.visitLdcInsn(n);
        if (target.isStatic()) mv.visitInsn(Opcodes.ACONST_NULL);
        else mv.visitVarInsn(Opcodes.ALOAD, 0);
    }

    private static void end(MethodVisitor mv) {
        mv.visitMaxs(0, 0);
        mv.visitEnd();
    }

    /** Pushes an {@code Object[]} of the locals from {@code slot} on, typed {@code types}, boxed. */
    private static void array(MethodVisitor mv, Type[] types, int slot) {
        mv.visitLdcInsn(types.length);
        mv.visitTypeInsn(Opcodes.ANEWARRAY, OBJECT);
        for (int i = 0; i < types.length; i++) {
            mv.visitInsn(Opcodes.DUP);
            mv.visitLdcInsn(i);
            mv.visitVarInsn(types[i].getOpcode(Opcodes.ILOAD), slot);
            box(mv, types[i]);
            mv.visitInsn(Opcodes.AASTORE);
            slot += types[i].getSize();
        }
    }

    /** Pushes the locals from {@code slot} on, typed {@code types}. */
    private static void load(MethodVisitor mv, Type[] types, int slot) {
        for (Type t : types) {
            mv.visitVarInsn(t.getOpcode(Opcodes.ILOAD), slot);
            slot += t.getSize();
        }
    }

    private static int size(Type[] types) {
        int n = 0;
        for (Type t : types) n += t.getSize();
        return n;
    }

    private static Type[] append(Type[] types, Type last) {
        Type[] out = java.util.Arrays.copyOf(types, types.length + 1);
        out[types.length] = last;
        return out;
    }

    private static Type[] prepend(Type first, Type[] types) {
        Type[] out = new Type[types.length + 1];
        out[0] = first;
        System.arraycopy(types, 0, out, 1, types.length);
        return out;
    }

    /** A primitive's wrapper (internal name), or null for a reference. */
    private static String wrapper(Type t) {
        return switch (t.getSort()) {
            case Type.BOOLEAN -> "java/lang/Boolean";
            case Type.BYTE -> "java/lang/Byte";
            case Type.CHAR -> "java/lang/Character";
            case Type.SHORT -> "java/lang/Short";
            case Type.INT -> "java/lang/Integer";
            case Type.LONG -> "java/lang/Long";
            case Type.FLOAT -> "java/lang/Float";
            case Type.DOUBLE -> "java/lang/Double";
            default -> null;
        };
    }

    private static void box(MethodVisitor mv, Type t) {
        String w = wrapper(t);
        if (w != null) mv.visitMethodInsn(Opcodes.INVOKESTATIC, w, "valueOf", "(" + t.getDescriptor() + ")L" + w + ";", false);
    }

    /** The {@code Object} on the stack as {@code t}. */
    private static void unbox(MethodVisitor mv, Type t) {
        String w = wrapper(t);
        if (w == null) {
            mv.visitTypeInsn(Opcodes.CHECKCAST, t.getSort() == Type.ARRAY ? t.getDescriptor() : t.getInternalName());
            return;
        }
        mv.visitTypeInsn(Opcodes.CHECKCAST, w);
        mv.visitMethodInsn(Opcodes.INVOKEVIRTUAL, w, t.getClassName() + "Value", "()" + t.getDescriptor(), false);
    }
}
