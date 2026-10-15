package dev.yog.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.yog.YogBody;
import net.minecraft.client.model.EntityModel;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** yog-body's hooks in drawing a living entity (see {@link YogBody}). */
@Mixin(LivingEntityRenderer.class)
public abstract class BodyRenderMixin<T extends LivingEntity, M extends EntityModel<T>> {
    @Shadow protected M model;

    private static final String RENDER =
            "render(Lnet/minecraft/world/entity/LivingEntity;FFLcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;I)V";

    @Inject(method = RENDER, at = @At("HEAD"), cancellable = true)
    private void yog$begin(LivingEntity e, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
        if (YogBody.begin((LivingEntityRenderer<?, ?>) (Object) this, e, yaw, partial, pose, buffers, light)) ci.cancel();
    }

    /** The second scale (after the entity's own) comes right after {@code setupRotations}. */
    @Inject(method = RENDER, at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;scale(FFF)V", ordinal = 1))
    private void yog$rotated(LivingEntity e, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
        YogBody.rotated(pose, buffers, light);
    }

    @Inject(method = RENDER, at = @At(value = "INVOKE",
            target = "Lnet/minecraft/client/model/EntityModel;setupAnim(Lnet/minecraft/world/entity/Entity;FFFFF)V", shift = At.Shift.AFTER))
    private void yog$posed(LivingEntity e, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
        YogBody.posed(model);
    }

    @Inject(method = RENDER, at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;popPose()V"))
    private void yog$drawn(LivingEntity e, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
        YogBody.drawn(model, pose, buffers, light);
    }

    @Inject(method = RENDER, at = @At("RETURN"))
    private void yog$end(LivingEntity e, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
        YogBody.end();
    }
}
