package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.entity.LivingEntity;
import net.minecraft.util.math.Vec3d;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A mod may move the local player instead of its travel step (yog-body). Client only. */
@Mixin(LivingEntity.class)
public abstract class TravelMixin {
    @Inject(method = "travel", at = @At("HEAD"), cancellable = true)
    private void yog$travel(Vec3d input, CallbackInfo ci) {
        if (YogBody.travel((LivingEntity) (Object) this)) ci.cancel();
    }
}
