package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.RotationAxis;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The mods' roll and field of view (yog-body). */
@Mixin(GameRenderer.class)
public abstract class ViewMixin {
    @Inject(method = "getFov", at = @At("RETURN"), cancellable = true)
    private void yog$fov(Camera camera, float partial, boolean changingFov, CallbackInfoReturnable<Double> cir) {
        if (YogBody.fov != 1) cir.setReturnValue(cir.getReturnValue() * YogBody.fov);
    }

    /** Before the view turns by the camera's pitch and yaw. */
    @Inject(method = "renderWorld", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/render/Camera;getPitch()F", ordinal = 0))
    private void yog$roll(float partial, long limitTime, MatrixStack matrices, CallbackInfo ci) {
        if (YogBody.roll != 0) matrices.multiply(RotationAxis.POSITIVE_Z.rotationDegrees(YogBody.roll));
    }
}
