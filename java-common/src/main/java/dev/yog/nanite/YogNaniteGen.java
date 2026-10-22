package dev.yog.nanite;

import dev.yog.bus.YogTypes;
import java.io.IOException;
import java.io.InputStream;
import org.objectweb.asm.AnnotationVisitor;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;

/**
 * A mod's injection as a Mixin class ({@code docs/design/nanites.md}): one
 * {@code @Mixin} on the target class (its run-time name) with one
 * {@code @Inject} handler whose body only packs the arguments and hands
 * them to {@link YogInjections#fire} — the mods' code runs on the bus. Made
 * with ASM before the game's classes load; no target class is loaded (its
 * bytes are read to tell a static method).
 */
final class YogNaniteGen {
    private YogNaniteGen() {}

    /** The generated mixins' package (the config's), internal form. */
    static final String PACKAGE = "dev/yog/nanite/gen";
    /** A generated mixin's simple name before its number. */
    static final String CLASS_PREFIX = "Nanite";
    private static final String HANDLER_PREFIX = "yog$inject$";

    private static final String MIXIN = "Lorg/spongepowered/asm/mixin/Mixin;";
    private static final String INJECT = "Lorg/spongepowered/asm/mixin/injection/Inject;";
    private static final String AT = "Lorg/spongepowered/asm/mixin/injection/At;";
    private static final String SHIFT = "Lorg/spongepowered/asm/mixin/injection/At$Shift;";
    private static final String CALLBACK = "org/spongepowered/asm/mixin/injection/callback/CallbackInfo";
    private static final String CALLBACK_RETURNABLE = "org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable";
    private static final String FIRE_OWNER = Type.getInternalName(YogInjections.class);
    private static final String FIRE_DESC = "(ILjava/lang/Object;[Ljava/lang/Object;L" + CALLBACK + ";)V";
    /** Where a method's descriptor starts in a member reference. */
    private static final char DESC_START = '(';
    /** What separates a field reference's name from its type. */
    private static final char FIELD_TYPE = ':';
    /** What closes a class in a member reference ({@code Lowner;name…}). */
    private static final char CLASS_END = ';';

    /** Injection {@code point} (its index {@code n}) as a mixin class: its internal name and bytes. */
    static byte[] mixin(int n, YogTypes.Inject inject) {
        String owner = inject.class_().replace('.', '/');
        String runtimeOwner = YogMappings.className(owner);
        String desc = YogMappings.desc(inject.desc());
        String name = YogMappings.declaredMethod(owner, inject.method(), inject.desc());
        boolean isStatic = isStatic(runtimeOwner, name, desc);
        ClassWriter cw = new ClassWriter(ClassWriter.COMPUTE_FRAMES | ClassWriter.COMPUTE_MAXS);
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC | Opcodes.ACC_SUPER, className(n), null, "java/lang/Object", null);
        AnnotationVisitor mixin = cw.visitAnnotation(MIXIN, false);
        AnnotationVisitor targets = mixin.visitArray("targets");
        targets.visit(null, runtimeOwner.replace('/', '.'));
        targets.visitEnd();
        mixin.visit("remap", false);
        mixin.visitEnd();
        handler(cw, n, inject, new Target(name + desc, desc, isStatic));
        cw.visitEnd();
        return cw.toByteArray();
    }

    /** Mixin {@code n}'s internal name. */
    static String className(int n) {
        return PACKAGE + "/" + CLASS_PREFIX + n;
    }

    /** The method injected into: its selector ({@code name desc}), descriptor, whether it's static. */
    private record Target(String selector, String desc, boolean isStatic) {}

    private static void handler(ClassWriter cw, int n, YogTypes.Inject inject, Target target) {
        boolean isStatic = target.isStatic();
        Type method = Type.getMethodType(target.desc());
        boolean returns = method.getReturnType().getSort() != Type.VOID;
        String callback = returns ? CALLBACK_RETURNABLE : CALLBACK;
        Type[] params = method.getArgumentTypes();
        Type[] handlerParams = new Type[params.length + 1];
        System.arraycopy(params, 0, handlerParams, 0, params.length);
        handlerParams[params.length] = Type.getObjectType(callback);
        int access = Opcodes.ACC_PRIVATE | (isStatic ? Opcodes.ACC_STATIC : 0);
        MethodVisitor mv = cw.visitMethod(access, HANDLER_PREFIX + n, Type.getMethodDescriptor(Type.VOID_TYPE, handlerParams), null, null);
        annotate(mv, inject, target.selector());
        mv.visitCode();
        mv.visitLdcInsn(n);
        if (isStatic) mv.visitInsn(Opcodes.ACONST_NULL);
        else mv.visitVarInsn(Opcodes.ALOAD, 0);
        int slot = isStatic ? 0 : 1;
        mv.visitLdcInsn(params.length);
        mv.visitTypeInsn(Opcodes.ANEWARRAY, "java/lang/Object");
        for (int i = 0; i < params.length; i++) {
            mv.visitInsn(Opcodes.DUP);
            mv.visitLdcInsn(i);
            mv.visitVarInsn(params[i].getOpcode(Opcodes.ILOAD), slot);
            box(mv, params[i]);
            mv.visitInsn(Opcodes.AASTORE);
            slot += params[i].getSize();
        }
        mv.visitVarInsn(Opcodes.ALOAD, slot);
        mv.visitMethodInsn(Opcodes.INVOKESTATIC, FIRE_OWNER, "fire", FIRE_DESC, false);
        mv.visitInsn(Opcodes.RETURN);
        mv.visitMaxs(0, 0);
        mv.visitEnd();
    }

    /** {@code @Inject(method, at = @At(…), cancellable, remap = false)}. */
    private static void annotate(MethodVisitor mv, YogTypes.Inject inject, String selector) {
        AnnotationVisitor in = mv.visitAnnotation(INJECT, true);
        AnnotationVisitor methods = in.visitArray("method");
        methods.visit(null, selector);
        methods.visitEnd();
        AnnotationVisitor ats = in.visitArray("at");
        AnnotationVisitor at = ats.visitAnnotation(null, AT);
        at.visit("value", inject.at());
        if (!inject.target().isEmpty()) at.visit("target", target(inject.target()));
        at.visit("ordinal", inject.ordinal());
        at.visitEnum("shift", SHIFT, inject.shift());
        at.visit("remap", false);
        at.visitEnd();
        ats.visitEnd();
        in.visit("cancellable", inject.cancellable());
        in.visit("remap", false);
        in.visitEnd();
    }

    /**
     * An {@code @At} target in run-time names: a method
     * ({@code Lowner;name(desc)}), a field ({@code Lowner;name:type}) or a
     * class ({@code owner}, a {@code NEW}).
     */
    static String target(String named) {
        int end = named.indexOf(CLASS_END);
        if (!named.startsWith("L") || end < 0) return YogMappings.className(named.replace('.', '/'));
        String owner = named.substring(1, end);
        String member = named.substring(end + 1);
        String runtimeOwner = "L" + YogMappings.className(owner) + CLASS_END;
        int paren = member.indexOf(DESC_START);
        if (paren >= 0) {
            String name = member.substring(0, paren), desc = member.substring(paren);
            return runtimeOwner + YogMappings.declaredMethod(owner, name, desc) + YogMappings.desc(desc);
        }
        int colon = member.indexOf(FIELD_TYPE);
        String name = colon < 0 ? member : member.substring(0, colon);
        String type = colon < 0 ? "" : FIELD_TYPE + YogMappings.desc(member.substring(colon + 1));
        return runtimeOwner + YogMappings.declaredField(owner, name) + type;
    }

    private static void box(MethodVisitor mv, Type t) {
        String boxed = switch (t.getSort()) {
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
        if (boxed != null) {
            mv.visitMethodInsn(Opcodes.INVOKESTATIC, boxed, "valueOf", "(" + t.getDescriptor() + ")L" + boxed + ";", false);
        }
    }

    /** Whether method {@code name desc} of {@code owner} is static, from its bytes (not loading it). */
    private static boolean isStatic(String owner, String name, String desc) {
        ClassLoader loader = Thread.currentThread().getContextClassLoader();
        if (loader == null) loader = YogNaniteGen.class.getClassLoader();
        try (InputStream in = loader.getResourceAsStream(owner + ".class")) {
            if (in == null) return false;
            boolean[] found = {false};
            new ClassReader(in).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public MethodVisitor visitMethod(int access, String n, String d, String sig, String[] ex) {
                    if (n.equals(name) && d.equals(desc)) found[0] = (access & Opcodes.ACC_STATIC) != 0;
                    return null;
                }
            }, ClassReader.SKIP_CODE);
            return found[0];
        } catch (IOException e) {
            dev.yog.YogLog.warn("nanite: " + owner + " unreadable (" + e + "): taken as an instance method");
            return false;
        }
    }
}
