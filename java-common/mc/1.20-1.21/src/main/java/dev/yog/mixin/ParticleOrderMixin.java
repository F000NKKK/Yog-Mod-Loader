package dev.yog.mixin;

import dev.yog.YogParticleLayers;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.particle.ParticleEngine;
import net.minecraft.client.particle.ParticleRenderType;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The game draws only the particle sheets it lists: Yog's are drawn after its own. */
@Mixin(ParticleEngine.class)
public abstract class ParticleOrderMixin {
    @Shadow @Final @Mutable private static List<ParticleRenderType> RENDER_ORDER;

    @Inject(method = "<clinit>", at = @At("TAIL"))
    private static void yog$order(CallbackInfo ci) {
        List<ParticleRenderType> order = new ArrayList<>(RENDER_ORDER);
        order.addAll(YogParticleLayers.ALL);
        RENDER_ORDER = List.copyOf(order);
    }
}
