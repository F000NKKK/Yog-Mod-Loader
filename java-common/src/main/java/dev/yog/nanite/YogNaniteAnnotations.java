package dev.yog.nanite;

import dev.yog.YogNames;
import dev.yog.bus.YogTypes;
import org.objectweb.asm.AnnotationVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Type;

/**
 * The Mixin annotations of a generated handler, one a nanite kind: the
 * method it goes into, its {@code @At} (run-time names), {@code remap = false}
 * (the names are run-time ones already, no refmap).
 */
final class YogNaniteAnnotations {
    private YogNaniteAnnotations() {}

    private static final String INJECTION = "Lorg/spongepowered/asm/mixin/injection/";
    private static final String INJECT = INJECTION + "Inject;";
    private static final String REDIRECT = INJECTION + "Redirect;";
    private static final String MODIFY_ARG = INJECTION + "ModifyArg;";
    private static final String MODIFY_VARIABLE = INJECTION + "ModifyVariable;";
    private static final String MODIFY_CONSTANT = INJECTION + "ModifyConstant;";
    private static final String CONSTANT = INJECTION + "Constant;";
    private static final String AT = INJECTION + "At;";
    private static final String SHIFT = INJECTION + "At$Shift;";
    /** What separates a constant's type from its value ({@code float:0.5}). */
    private static final char CONSTANT_SEP = ':';
    private static final String NO_TARGET = "";
    private static final int ANY = -1;

    static void inject(MethodVisitor mv, YogTypes.Inject inject, YogNaniteGen.Target target) {
        AnnotationVisitor a = start(mv, INJECT, target);
        at(a, inject);
        a.visit("cancellable", inject.cancellable());
        a.visitEnd();
    }

    static void redirect(MethodVisitor mv, YogTypes.Inject inject, YogNaniteGen.Target target) {
        AnnotationVisitor a = start(mv, REDIRECT, target);
        atOne(a, inject);
        a.visitEnd();
    }

    static void modifyArg(MethodVisitor mv, YogTypes.Inject inject, YogNaniteGen.Target target) {
        AnnotationVisitor a = start(mv, MODIFY_ARG, target);
        atOne(a, inject);
        a.visit("index", inject.index());
        a.visitEnd();
    }

    static void modifyVariable(MethodVisitor mv, YogTypes.Inject inject, YogNaniteGen.Target target) {
        AnnotationVisitor a = start(mv, MODIFY_VARIABLE, target);
        atOne(a, inject);
        if (inject.index() != ANY) a.visit("index", inject.index());
        a.visitEnd();
    }

    static void modifyConstant(MethodVisitor mv, YogTypes.Inject inject, YogNaniteGen.Target target) {
        AnnotationVisitor a = start(mv, MODIFY_CONSTANT, target);
        AnnotationVisitor constants = a.visitArray("constant");
        AnnotationVisitor c = constants.visitAnnotation(null, CONSTANT);
        constant(c, inject.constant());
        if (inject.ordinal() != ANY) c.visit("ordinal", inject.ordinal());
        c.visitEnd();
        constants.visitEnd();
        a.visitEnd();
    }

    /** The annotation {@code desc} on {@code mv}, its method {@code target}'s, not remapped. */
    private static AnnotationVisitor start(MethodVisitor mv, String desc, YogNaniteGen.Target target) {
        AnnotationVisitor a = mv.visitAnnotation(desc, true);
        AnnotationVisitor methods = a.visitArray("method");
        methods.visit(null, target.selector());
        methods.visitEnd();
        a.visit("remap", false);
        return a;
    }

    /** {@code at = {@At(…)}}: an array. */
    private static void at(AnnotationVisitor a, YogTypes.Inject inject) {
        AnnotationVisitor ats = a.visitArray("at");
        point(ats.visitAnnotation(null, AT), inject);
        ats.visitEnd();
    }

    /** {@code at = @At(…)}: one. */
    private static void atOne(AnnotationVisitor a, YogTypes.Inject inject) {
        point(a.visitAnnotation("at", AT), inject);
    }

    private static void point(AnnotationVisitor at, YogTypes.Inject inject) {
        at.visit("value", inject.at());
        if (!inject.target().equals(NO_TARGET)) at.visit("target", YogNaniteGen.target(inject.target()));
        at.visit("ordinal", inject.ordinal());
        at.visitEnum("shift", SHIFT, inject.shift());
        at.visit("remap", false);
        at.visitEnd();
    }

    /** {@code @Constant}'s value member for {@code spec} ({@code <type>:<value>}). */
    private static void constant(AnnotationVisitor c, String spec) {
        int sep = spec.indexOf(CONSTANT_SEP);
        String kind = sep < 0 ? spec : spec.substring(0, sep);
        String value = sep < 0 ? "" : spec.substring(sep + 1);
        switch (kind) {
            case YogNames.NaniteConstant.INT -> c.visit("intValue", Integer.parseInt(value));
            case YogNames.NaniteConstant.FLOAT -> c.visit("floatValue", Float.parseFloat(value));
            case YogNames.NaniteConstant.LONG -> c.visit("longValue", Long.parseLong(value));
            case YogNames.NaniteConstant.DOUBLE -> c.visit("doubleValue", Double.parseDouble(value));
            case YogNames.NaniteConstant.STRING -> c.visit("stringValue", value);
            default -> c.visit("nullValue", true);
        }
    }

    /** The type of the constant {@code spec} names. */
    static Type constantType(String spec) {
        int sep = spec.indexOf(CONSTANT_SEP);
        return switch (sep < 0 ? spec : spec.substring(0, sep)) {
            case YogNames.NaniteConstant.INT -> Type.INT_TYPE;
            case YogNames.NaniteConstant.FLOAT -> Type.FLOAT_TYPE;
            case YogNames.NaniteConstant.LONG -> Type.LONG_TYPE;
            case YogNames.NaniteConstant.DOUBLE -> Type.DOUBLE_TYPE;
            case YogNames.NaniteConstant.STRING -> Type.getType(String.class);
            default -> Type.getType(Object.class);
        };
    }
}
