package dev.yog.nanite;

import dev.yog.bus.YogTypes;
import java.util.List;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.AnnotationNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A generated injection, and one fired, for the runtime's JVM test. */
public final class InjectProbe {
    private InjectProbe() {}

    static final String POINT = "test:point";

    private int counted(String s, int n) {
        return n;
    }

    static YogTypes.Inject inject() {
        return new YogTypes.Inject(POINT, "inject", InjectProbe.class.getName(), "counted", "(Ljava/lang/String;I)I",
                "HEAD", "", -1, "NONE", true, -1, "", "");
    }

    /** The mixin made of {@link #inject}: its parts, joined. */
    public static String generate() {
        ClassNode c = new ClassNode();
        new ClassReader(YogNaniteGen.mixin(0, inject()).bytes()).accept(c, 0);
        AnnotationNode mixin = c.invisibleAnnotations.get(0);
        MethodNode m = c.methods.get(0);
        AnnotationNode in = m.visibleAnnotations.get(0);
        return String.join("|", c.name, mixin.desc, String.valueOf(mixin.values), m.name, m.desc,
                in.desc, String.valueOf(in.values.get(1)));
    }

    /** {@link #inject} fired with ("x", 3): whether it was cancelled, and with what. */
    public static String fire() {
        YogInjections.points(List.of(POINT), List.of(new YogNaniteGen.Made(null, Type.INT_TYPE, null)));
        CallbackInfoReturnable<Integer> cir = new CallbackInfoReturnable<>("counted", true);
        YogInjections.fire(0, null, new Object[] {"x", 3}, cir);
        return cir.isCancelled() + "," + cir.getReturnValue();
    }

    static final String MODIFY = "test:modify";
    static final String REDIRECT = "test:redirect";
    private static final String CALLEE = "Ldev/yog/nanite/InjectProbe;callee(I)I";

    static int base(int x) {
        return callee(x + 1);
    }

    public static int callee(int x) {
        return x * 10;
    }

    private static YogTypes.Inject at(String point, String kind, int index) {
        return new YogTypes.Inject(point, kind, InjectProbe.class.getName(), "base", "(I)I",
                "INVOKE", CALLEE, -1, "NONE", false, index, "", "");
    }

    /** A made mixin's handler {@code n} run on {@code x} (the class defined as a plain one). */
    private static Object run(YogNaniteGen.Made made, int n, int x) throws Exception {
        Class<?> c = new ClassLoader(InjectProbe.class.getClassLoader()) {
            Class<?> define(byte[] b) {
                return defineClass(null, b, 0, b.length);
            }
        }.define(made.bytes());
        java.lang.reflect.Method h = c.getDeclaredMethod("yog$nanite$" + n, int.class);
        h.setAccessible(true);
        return h.invoke(null, x);
    }

    /** A modify_arg and a redirect at {@link #base}'s call, made and run: their results, joined. */
    public static String modifyAndRedirect() throws Exception {
        YogNaniteGen.Made modify = YogNaniteGen.mixin(1, at(MODIFY, "modify_arg", 0));
        YogNaniteGen.Made redirect = YogNaniteGen.mixin(2, at(REDIRECT, "redirect", -1));
        YogInjections.points(List.of(POINT, MODIFY, REDIRECT),
                List.of(new YogNaniteGen.Made(null, Type.INT_TYPE, null), modify, redirect));
        return run(modify, 1, 5) + "," + run(redirect, 2, 5);
    }

    /** The redirect run again, its handlers gone: the call made as it was. */
    public static String redirectUnanswered() throws Exception {
        YogNaniteGen.Made redirect = YogNaniteGen.mixin(2, at("test:nobody", "redirect", -1));
        YogInjections.points(List.of(POINT, MODIFY, "test:nobody"),
                List.of(new YogNaniteGen.Made(null, Type.INT_TYPE, null), redirect, redirect));
        return String.valueOf(run(redirect, 2, 5));
    }

    static final String ADDED = "test:added";

    private static YogTypes.Inject adding(String point, String kind, String name, String desc, String target) {
        return new YogTypes.Inject(point, kind, InjectProbe.class.getName(), name, desc, "", target, -1, "", false, -1, "", "");
    }

    /** An added method called, an added field and an interface seen: joined. */
    public static String added() throws Exception {
        YogNaniteGen.Made method = YogNaniteGen.mixin(3, adding(ADDED, "add_method", "yogCharge", "()I", ""));
        YogNaniteGen.Made field = YogNaniteGen.mixin(4, adding("test:field", "add_field", "yog$probe$charge", "I", ""));
        YogNaniteGen.Made iface = YogNaniteGen.mixin(5, adding("test:iface", "implement", "", "", "java.lang.Runnable"));
        YogInjections.points(List.of(POINT, MODIFY, REDIRECT, ADDED, "test:field", "test:iface"),
                List.of(method, method, method, method, field, iface));
        Class<?> c = new ClassLoader(InjectProbe.class.getClassLoader()) {
            Class<?> define(byte[] b) {
                return defineClass(null, b, 0, b.length);
            }
        }.define(method.bytes());
        java.lang.reflect.Field unsafe = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafe.setAccessible(true);
        Object u = unsafe.get(null);
        Object instance = u.getClass().getMethod("allocateInstance", Class.class).invoke(u, c);
        Object charge = c.getMethod("yogCharge").invoke(instance);
        ClassNode f = new ClassNode();
        new ClassReader(field.bytes()).accept(f, 0);
        ClassNode i = new ClassNode();
        new ClassReader(iface.bytes()).accept(i, 0);
        return charge + "," + f.fields.get(0).name + f.fields.get(0).desc + "," + i.interfaces;
    }
}
