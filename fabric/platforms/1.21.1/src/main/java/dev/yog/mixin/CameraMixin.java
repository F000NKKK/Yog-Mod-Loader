package dev.yog.mixin;

import dev.yog.YogBody;
import net.minecraft.client.render.Camera;
import net.minecraft.entity.Entity;
import net.minecraft.util.math.Vec3d;
import net.minecraft.world.BlockView;
import org.joml.Quaternionf;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The mods move and turn the camera after vanilla placed it (yog-body). */
@Mixin(Camera.class)
public abstract class CameraMixin {
    @Shadow @Final private Quaternionf rotation;
    @Shadow protected abstract void setRotation(float yaw, float pitch);
    @Shadow protected abstract void moveBy(float forward, float up, float left);
    @Shadow protected abstract void setPos(Vec3d pos);
    @Shadow public abstract Vec3d getPos();
    @Shadow public abstract float getYaw();
    @Shadow public abstract float getPitch();

    @Inject(method = "update", at = @At("TAIL"))
    private void yog$camera(BlockView area, Entity entity, boolean detached, boolean inverse, float partial, CallbackInfo ci) {
        float[] o = YogBody.camera(entity, partial, getPos(), getYaw(), getPitch(), detached);
        if (o == null) return;
        if (o[6] != 0 || o[7] != 0) setRotation(getYaw() + o[6], getPitch() + o[7]);
        if (o[0] != 0 || o[1] != 0 || o[2] != 0) moveBy(o[2], o[1], -o[0]);
        if (o[3] != 0 || o[4] != 0 || o[5] != 0) setPos(getPos().add(o[3], o[4], o[5]));
        // The view is built from this rotation (GameRenderer.renderWorld): the roll goes in it.
        if (o[8] != 0) rotation.rotateZ((float) Math.toRadians(-o[8]));
    }
}
