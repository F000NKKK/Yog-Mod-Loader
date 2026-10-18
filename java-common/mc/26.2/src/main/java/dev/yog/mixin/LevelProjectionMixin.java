package dev.yog.mixin;

import dev.yog.YogClientEvents;
import net.minecraft.client.renderer.GameRenderer;
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * The projection the level is really drawn with — the camera's, with the
 * view's bobbing and nausea's skew on it — for the mods' world drawing,
 * so what they draw moves with the world (the camera's own has neither).
 */
@Mixin(GameRenderer.class)
public abstract class LevelProjectionMixin {
    @ModifyArg(method = "renderLevel", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/client/renderer/ProjectionMatrixBuffer;getBuffer(Lorg/joml/Matrix4f;)Lcom/mojang/blaze3d/buffers/GpuBufferSlice;"))
    private Matrix4f yog$projection(Matrix4f projection) {
        YogClientEvents.levelProjection.set(projection);
        return projection;
    }
}
