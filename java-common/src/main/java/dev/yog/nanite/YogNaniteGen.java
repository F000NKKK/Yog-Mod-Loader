package dev.yog.nanite;

import dev.yog.YogNames;
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
 * A mod's nanite as a Mixin class ({@code docs/design/nanites.md}): one
 * {@code @Mixin} on the target class (its run-time name) with one handler of
 * the nanite's kind ({@code @Inject}, {@code @Redirect}, {@code @ModifyArg},
 * {@code @ModifyVariable}, {@code @ModifyConstant}) whose body only packs
 * what's at stake and hands it to {@link YogInjections} — the mods' code
 * runs on the bus. Made with ASM before the game's classes load; no game
 * class is loaded (bytes are read to tell a static method, an interface).
 */
final class YogNaniteGen {
    private YogNaniteGen() {}

    /** The generated mixins' package (the config's), internal form. */
    static final String PACKAGE = "dev/yog/nanite/gen";
    /** A generated mixin's simple name before its number. */
    static final String CLASS_PREFIX = "Nanite";
    private static final String MIXIN = "Lorg/spongepowered/asm/mixin/Mixin;";
    private static final String OBJECT = "java/lang/Object";
    /** Where a method's descriptor starts in a member reference. */
    private static final char DESC_START = '(';
    /** What separates a field reference's name from its type. */
    private static final char FIELD_TYPE = ':';
    /** What closes a class in a member reference ({@code Lowner;name…}). */
    private static final char CLASS_END = ';';

    /** The method a nanite goes into: its selector ({@code name desc}), type, whether it's static. */
    record Target(String selector, Type method, boolean isStatic) {}

    /** A call a redirect stands in for (run-time names). */
    record Call(String owner, String name, String desc, boolean isStatic, boolean isInterface) {}

    /** A made mixin: its bytes, the type of the value at stake, the call a redirect stands in for. */
    record Made(byte[] bytes, Type value, Call call) {}

    /** Nanite {@code inject} (its number {@code n}) as a mixin class. */
    static Made mixin(int n, YogTypes.Inject inject) {
        String owner = inject.class_().replace('.', '/');
        String runtimeOwner = YogMappings.className(owner);
        String desc = YogMappings.desc(inject.desc());
        String name = YogMappings.declaredMethod(owner, inject.method(), inject.desc());
        Target target = new Target(name + desc, Type.getMethodType(desc), flags(runtimeOwner, name, desc)[0]);
        ClassWriter cw = writer();
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC | Opcodes.ACC_SUPER, className(n), null, OBJECT, null);
        AnnotationVisitor mixin = cw.visitAnnotation(MIXIN, false);
        AnnotationVisitor targets = mixin.visitArray("targets");
        targets.visit(null, runtimeOwner.replace('/', '.'));
        targets.visitEnd();
        mixin.visit("remap", false);
        mixin.visitEnd();
        Made made = handler(cw, n, inject, target);
        cw.visitEnd();
        return new Made(cw.toByteArray(), made.value(), made.call());
    }

    /** The handler of the nanite's kind; its value's type and call (bytes left to {@link #mixin}). */
    private static Made handler(ClassWriter cw, int n, YogTypes.Inject inject, Target target) {
        return switch (inject.kind()) {
            case YogNames.NaniteKind.REDIRECT -> {
                Call call = call(inject.target());
                YogNaniteBodies.redirect(cw, n, target, call, mv -> YogNaniteAnnotations.redirect(mv, inject, target));
                yield new Made(null, Type.getReturnType(call.desc()), call);
            }
            case YogNames.NaniteKind.MODIFY_ARG -> {
                Type value = Type.getArgumentTypes(call(inject.target()).desc())[inject.index()];
                YogNaniteBodies.modify(cw, n, target, value, mv -> YogNaniteAnnotations.modifyArg(mv, inject, target));
                yield new Made(null, value, null);
            }
            case YogNames.NaniteKind.MODIFY_VARIABLE -> {
                Type value = Type.getType(YogMappings.desc(inject.valueDesc()));
                YogNaniteBodies.modify(cw, n, target, value, mv -> YogNaniteAnnotations.modifyVariable(mv, inject, target));
                yield new Made(null, value, null);
            }
            case YogNames.NaniteKind.MODIFY_CONSTANT -> {
                Type value = YogNaniteAnnotations.constantType(inject.constant());
                YogNaniteBodies.modify(cw, n, target, value, mv -> YogNaniteAnnotations.modifyConstant(mv, inject, target));
                yield new Made(null, value, null);
            }
            default -> {
                YogNaniteBodies.inject(cw, n, target, mv -> YogNaniteAnnotations.inject(mv, inject, target));
                yield new Made(null, target.method().getReturnType(), null);
            }
        };
    }

    /** Mixin {@code n}'s internal name. */
    static String className(int n) {
        return PACKAGE + "/" + CLASS_PREFIX + n;
    }

    /** A writer whose frames never load a class (a game class loaded now can't be mixed into). */
    private static ClassWriter writer() {
        return new ClassWriter(ClassWriter.COMPUTE_FRAMES | ClassWriter.COMPUTE_MAXS) {
            @Override
            protected String getCommonSuperClass(String a, String b) {
                return OBJECT;
            }
        };
    }

    /** The call {@code named} ({@code Lowner;name(desc)}, Mojang's) names. */
    static Call call(String named) {
        int end = named.indexOf(CLASS_END);
        String owner = named.substring(1, end);
        String member = named.substring(end + 1);
        int paren = member.indexOf(DESC_START);
        String name = member.substring(0, paren), desc = member.substring(paren);
        String runtimeOwner = YogMappings.className(owner);
        String runtimeName = YogMappings.declaredMethod(owner, name, desc);
        String runtimeDesc = YogMappings.desc(desc);
        boolean[] f = flags(runtimeOwner, runtimeName, runtimeDesc);
        return new Call(runtimeOwner, runtimeName, runtimeDesc, f[0], f[1]);
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

    /**
     * Whether method {@code name desc} of {@code owner} is static, and whether
     * {@code owner} is an interface — from its bytes, not loading it.
     */
    private static boolean[] flags(String owner, String name, String desc) {
        boolean[] found = {false, false};
        ClassLoader loader = Thread.currentThread().getContextClassLoader();
        if (loader == null) loader = YogNaniteGen.class.getClassLoader();
        try (InputStream in = loader.getResourceAsStream(owner + ".class")) {
            if (in == null) return found;
            new ClassReader(in).accept(new ClassVisitor(Opcodes.ASM9) {
                @Override
                public void visit(int v, int access, String n, String sig, String sup, String[] ifaces) {
                    found[1] = (access & Opcodes.ACC_INTERFACE) != 0;
                }

                @Override
                public MethodVisitor visitMethod(int access, String n, String d, String sig, String[] ex) {
                    if (n.equals(name) && d.equals(desc)) found[0] = (access & Opcodes.ACC_STATIC) != 0;
                    return null;
                }
            }, ClassReader.SKIP_CODE);
        } catch (IOException e) {
            dev.yog.YogLog.warn("nanite: " + owner + " unreadable (" + e + "): taken as an instance method of a class");
        }
        return found;
    }
}
