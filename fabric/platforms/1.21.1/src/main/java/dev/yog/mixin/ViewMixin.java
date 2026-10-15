package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The mods' field of view (yog-body); the roll is in the camera's rotation. */
@Mixin(GameRenderer.class)
public abstract class ViewMixin {
    @Inject(method = "getFov", at = @At("RETURN"), cancellable = true)
    private void yog$fov(Camera camera, float partial, boolean changingFov, CallbackInfoReturnable<Double> cir) {
        if (YogBody.fov != 1) cir.setReturnValue(cir.getReturnValue() * YogBody.fov);
    }
}
