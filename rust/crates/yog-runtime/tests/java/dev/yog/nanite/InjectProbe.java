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
        return new YogTypes.Inject(POINT, InjectProbe.class.getName(), "counted", "(Ljava/lang/String;I)I",
                "HEAD", "", -1, "NONE", true);
    }

    /** The mixin made of {@link #inject}: its parts, joined. */
    public static String generate() {
        ClassNode c = new ClassNode();
        new ClassReader(YogNaniteGen.mixin(0, inject())).accept(c, 0);
        AnnotationNode mixin = c.invisibleAnnotations.get(0);
        MethodNode m = c.methods.get(0);
        AnnotationNode in = m.visibleAnnotations.get(0);
        return String.join("|", c.name, mixin.desc, String.valueOf(mixin.values), m.name, m.desc,
                in.desc, String.valueOf(in.values.get(1)));
    }

    /** {@link #inject} fired with ("x", 3): whether it was cancelled, and with what. */
    public static String fire() {
        YogInjections.points(List.of(POINT), List.of(Type.INT_TYPE));
        CallbackInfoReturnable<Integer> cir = new CallbackInfoReturnable<>("counted", true);
        YogInjections.fire(0, null, new Object[] {"x", 3}, cir);
        return cir.isCancelled() + "," + cir.getReturnValue();
    }
}
