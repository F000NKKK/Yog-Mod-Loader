package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.Vec3;
import org.joml.Quaternionf;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The mods move and turn the camera after vanilla placed it, and scale its
 * field of view (yog-body); the roll goes in its rotation.
 */
@Mixin(Camera.class)
public abstract class CameraMixin {
    @Shadow @Final private Quaternionf rotation;
    @Shadow protected abstract void setRotation(float yaw, float pitch);
    @Shadow protected abstract void move(float forward, float up, float right);
    @Shadow protected abstract void setPosition(Vec3 pos);
    @Shadow public abstract Vec3 position();
    @Shadow public abstract float yRot();
    @Shadow public abstract float xRot();
    @Shadow public abstract Entity entity();
    @Shadow public abstract boolean isDetached();

    @Inject(method = "alignWithEntity", at = @At("TAIL"))
    private void yog$camera(float partial, CallbackInfo ci) {
        float[] o = YogBody.camera(entity(), partial, position(), yRot(), xRot(), isDetached());
        if (o == null) return;
        if (o[6] != 0 || o[7] != 0) setRotation(yRot() + o[6], xRot() + o[7]);
        if (o[0] != 0 || o[1] != 0 || o[2] != 0) move(o[2], o[1], -o[0]);
        if (o[3] != 0 || o[4] != 0 || o[5] != 0) setPosition(position().add(o[3], o[4], o[5]));
        // The view is built from this rotation: the roll goes in it.
        if (o[8] != 0) rotation.rotateZ((float) Math.toRadians(-o[8]));
    }

    @Inject(method = "calculateFov", at = @At("RETURN"), cancellable = true)
    private void yog$fov(float partial, CallbackInfoReturnable<Float> cir) {
        if (YogBody.fov != 1) cir.setReturnValue(cir.getReturnValue() * YogBody.fov);
    }
}
