package dev.yog.nanite;

import java.util.List;
import java.util.Set;
import org.objectweb.asm.tree.ClassNode;
import org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;

/**
 * The host's nanite config's plugin ({@code yog-nanites.mixins.json}): its
 * mixins are the ones {@link YogNanites} makes from the mods' injections.
 */
public final class YogNanitePlugin implements IMixinConfigPlugin {
    /** MixinExtras' bootstrap, where a loader doesn't start it itself (Forge's 1.20.1 line: jar-in-jar). */
    private static final String MIXIN_EXTRAS = "com.llamalad7.mixinextras.MixinExtrasBootstrap";

    @Override
    public void onLoad(String mixinPackage) {
        try {
            Class.forName(MIXIN_EXTRAS).getMethod("init").invoke(null);
        } catch (ReflectiveOperationException | LinkageError e) {
            dev.yog.YogLog.warn("nanites: no MixinExtras here (" + e + "): an inject's locals won't be reached");
        }
    }

    @Override
    public String getRefMapperConfig() {
        return null;
    }

    @Override
    public boolean shouldApplyMixin(String targetClassName, String mixinClassName) {
        return true;
    }

    @Override
    public void acceptTargets(Set<String> myTargets, Set<String> otherTargets) {}

    @Override
    public List<String> getMixins() {
        return YogNanites.mixins();
    }

    @Override
    public void preApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {}

    @Override
    public void postApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {
        dev.yog.YogLog.info("nanites: " + mixinClassName + " applied to " + targetClassName);
    }
}
