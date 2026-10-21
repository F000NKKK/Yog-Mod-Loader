package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.Camera;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The mods move and turn the camera after vanilla placed it (yog-body). */
@Mixin(Camera.class)
public abstract class CameraMixin {
    @Shadow @org.spongepowered.asm.mixin.Final private org.joml.Quaternionf rotation;
    @Shadow protected abstract void setRotation(float yRot, float xRot);
    @Shadow protected abstract void move(float forward, float up, float left);
    @Shadow protected abstract void setPosition(Vec3 pos);
    @Shadow public abstract Vec3 getPosition();
    @Shadow public abstract float getYRot();
    @Shadow public abstract float getXRot();

    @Inject(method = "setup", at = @At("TAIL"))
    private void yog$camera(BlockGetter level, Entity entity, boolean detached, boolean mirrored, float partial, CallbackInfo ci) {
        float[] o = YogBody.camera(entity, partial, getPosition(), new YogBody.Look(getYRot(), getXRot()), detached);
        if (o == null) return;
        if (o[6] != 0 || o[7] != 0) setRotation(getYRot() + o[6], getXRot() + o[7]);
        if (o[0] != 0 || o[1] != 0 || o[2] != 0) move(o[2], o[1], -o[0]);
        if (o[3] != 0 || o[4] != 0 || o[5] != 0) setPosition(getPosition().add(o[3], o[4], o[5]));
        // The view is built from this rotation: the roll goes in it.
        if (o[8] != 0) rotation.rotateZ((float) Math.toRadians(-o[8]));
    }
}
